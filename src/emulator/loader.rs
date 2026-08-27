use std::fs;
use std::path::Path;

use super::mmu::MemoryBus;
use super::sonix::{SonixImage, XIP_CACHED, XIP_UNCACHED};

/// Nature de l'image reconnue dans le fichier charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    /// Dump de flash Sonix porteur d'une load table.
    Sonix,
    /// Binaire ARM brut commencant par une table de vecteurs Cortex-M.
    RawCortexM,
    /// Contenu non reconnu, charge en flash mais non demarrable.
    Unknown,
}

/// Une region installee en memoire par le chargement.
#[derive(Debug, Clone)]
pub struct LoadedRegion {
    pub label: &'static str,
    pub addr: u32,
    pub len: u32,
}

/// Compte rendu factuel du chargement, destine a l'affichage et aux tests.
#[derive(Debug, Clone)]
pub struct LoadReport {
    pub bytes: usize,
    pub kind: ImageKind,
    /// L'image etait chiffree dans le dump.
    pub encrypted: bool,
    /// Le code de boot a pu etre remis en clair, donc la machine peut demarrer.
    pub bootable: bool,
    pub entry_sp: u32,
    pub entry_pc: u32,
    pub regions: Vec<LoadedRegion>,
}

impl LoadReport {
    fn empty(bytes: usize, kind: ImageKind) -> Self {
        Self {
            bytes,
            kind,
            encrypted: false,
            bootable: false,
            entry_sp: 0,
            entry_pc: 0,
            regions: Vec::new(),
        }
    }
}

pub struct FirmwareLoader;

impl FirmwareLoader {
    /// Charge un dump de flash et prepare la machine a executer le vrai firmware.
    ///
    /// `device_key` est la cle de la puce. Sans elle, un dump chiffre reste
    /// consultable (code XIP et assets sont en clair) mais ne peut pas demarrer.
    pub fn load_flash_dump<P: AsRef<Path>>(
        bus: &mut MemoryBus,
        path: P,
        device_key: Option<u32>,
    ) -> Result<LoadReport, String> {
        let path = path.as_ref();
        let buffer = fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let len = buffer.len();

        if !SonixImage::is_sonix(&buffer) {
            return Ok(Self::load_raw(bus, &buffer));
        }

        let key = device_key.or_else(|| resolve_device_key(path));
        let image = SonixImage::load(&buffer, key)
            .map_err(|e| format!("load table Sonix illisible: {:?}", e))?;

        bus.flash.load_binary(0, &image.flash);

        let table = image.primary().clone();
        let mut report = LoadReport::empty(len, ImageKind::Sonix);
        report.encrypted = image.was_encrypted;

        for r in table.regions() {
            let label = if r.addr == table.user_code.addr {
                "user_code"
            } else if r.addr == table.sram_code.addr {
                "sram_code"
            } else {
                "dpd_code"
            };
            report.regions.push(LoadedRegion { label, addr: r.addr, len: r.len });
        }

        if !image.decrypted {
            // Le dump reste inspectable, mais on ne pretend pas pouvoir l'executer.
            bus.pram.clear();
            return Ok(report);
        }

        // Le bootrom recopie le code utilisateur en PRAM, mappee a 0, puis saute
        // sur le vecteur de reset qui s'y trouve.
        let src = table.user_code.flash_offset();
        let end = (src + table.user_code.len as usize).min(image.flash.len());
        if src >= end {
            return Ok(report);
        }
        bus.pram.load(&image.flash[src..end]);
        install_boot_info(bus, &table);

        report.entry_sp = bus.pram.read_u32(0);
        report.entry_pc = bus.pram.read_u32(4);
        report.bootable = report.entry_pc & 1 == 1;

        Ok(report)
    }

    /// Charge un binaire ARM brut, sans load table Sonix.
    fn load_raw(bus: &mut MemoryBus, buffer: &[u8]) -> LoadReport {
        bus.flash.load_binary(0, buffer);
        let mut report = LoadReport::empty(buffer.len(), ImageKind::Unknown);

        if buffer.len() < 8 {
            return report;
        }
        let sp = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let pc = u32::from_le_bytes(buffer[4..8].try_into().unwrap());
        // Une table de vecteurs Cortex-M plausible : pile en RAM, reset en Thumb.
        let sp_in_ram = (0x1800_0000..0x1804_0000).contains(&sp)
            || (0x2000_0000..0x2002_0000).contains(&sp);
        if !sp_in_ram || pc & 1 == 0 {
            return report;
        }

        bus.pram.load(&buffer[..buffer.len().min(super::mmu::PRAM_SIZE)]);
        report.kind = ImageKind::RawCortexM;
        report.entry_sp = sp;
        report.entry_pc = pc;
        report.bootable = true;
        report.regions.push(LoadedRegion {
            label: "raw_image",
            addr: 0,
            len: buffer.len().min(super::mmu::PRAM_SIZE) as u32,
        });
        report
    }

    pub fn load_bootrom_dump<P: AsRef<Path>>(
        bus: &mut MemoryBus,
        path: P,
    ) -> Result<usize, String> {
        let path = path.as_ref();
        let buffer = fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let len = buffer.len();
        bus.boot_rom.load_binary(&buffer);
        Ok(len)
    }

    /// Etat de depart sans firmware : PRAM vide, CPU a l'arret.
    ///
    /// Aucune image de synthese n'est fabriquee ici. Tant qu'aucun dump n'est
    /// charge, l'ecran de l'emulateur reste vide, ce qui est la verite.
    pub fn install_idle_state(bus: &mut MemoryBus) {
        bus.pram.clear();
        bus.sram.data.fill(0);
    }
}

/// Emplacement du bloc boot-info dans la mailbox, et de son pointeur.
///
/// Le bootrom laisse ces donnees derriere lui avant de sauter dans le firmware.
/// Comme le bootrom n'est pas emule, on reproduit les deux seuls champs que le
/// firmware relit, releves en tracant son execution.
pub const BOOT_INFO_BLOCK: u32 = 0x2000_0000;
const BOOT_INFO_PTR: usize = 0xF60;
const BOOT_INFO_XIP_BASE: usize = 0x818;

/// Base de la region XIP telle que la load table la decrit.
///
/// Le firmware la recopie dans le registre BASE du controleur XIP, ce qui
/// decale la fenetre 0x10000000 sur le debut du code, et non sur celui de la
/// flash. Un saut vers 0x1006D1C4 vise donc l'offset 0x11000 + 0x6D1C4.
pub fn xip_region_base(table: &crate::emulator::sonix::LoadTable) -> u32 {
    if table.sram_code.addr != 0 {
        table.sram_code.addr
    } else {
        table.user_code.addr.wrapping_add(table.user_code.len)
    }
}

fn install_boot_info(bus: &mut MemoryBus, table: &crate::emulator::sonix::LoadTable) {
    let mb = &mut bus.sram.mailbox;
    let ptr_off = (BOOT_INFO_BLOCK & 0xFFF) as usize;
    if mb.len() < BOOT_INFO_PTR + 4 || mb.len() < ptr_off + BOOT_INFO_XIP_BASE + 4 {
        return;
    }
    mb[BOOT_INFO_PTR..BOOT_INFO_PTR + 4].copy_from_slice(&BOOT_INFO_BLOCK.to_le_bytes());
    let field = ptr_off + BOOT_INFO_XIP_BASE;
    mb[field..field + 4].copy_from_slice(&xip_region_base(table).to_le_bytes());
}

/// Cherche la deviceKey a cote du dump, faute de pouvoir la lire dans la puce.
///
/// Deux sources, dans l'ordre : la variable d'environnement `SONIX_DEVICE_KEY`,
/// puis un fichier `<dump>.key` contenant la valeur en hexadecimal.
fn resolve_device_key(path: &Path) -> Option<u32> {
    fn parse(s: &str) -> Option<u32> {
        let s = s.trim();
        let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
        u32::from_str_radix(s, 16).ok()
    }

    if let Ok(v) = std::env::var("SONIX_DEVICE_KEY") {
        if let Some(k) = parse(&v) {
            return Some(k);
        }
    }

    let sidecar = path.with_extension(format!(
        "{}.key",
        path.extension().and_then(|e| e.to_str()).unwrap_or("bin")
    ));
    fs::read_to_string(sidecar).ok().and_then(|s| parse(&s))
}

/// Adresses equivalentes d'un meme offset flash, fenetre cachee et non cachee.
pub fn xip_aliases(flash_offset: u32) -> (u32, u32) {
    (XIP_CACHED + flash_offset, XIP_UNCACHED + flash_offset)
}
