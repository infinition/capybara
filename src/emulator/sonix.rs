//! Lecture des load tables Sonix SNC73xx et dechiffrement du code de boot.
//!
//! Le bootrom parcourt la flash a la recherche d'une load table, qui decrit ou
//! charger chaque region et si le code est chiffre. Le schema de chiffrement est
//! un CBC inverse (tables V3) ou un OFB (tables anterieures), dont la cle et l'IV
//! derivent du champ cle de la table combine a la deviceKey de la puce, gravee
//! dans les fusibles SN_SYS0->FEUSE2/3 et absente du dump.

use crate::emulator::aes::Aes;

/// Base de la fenetre XIP non cachee.
pub const XIP_UNCACHED: u32 = 0x6000_0000;
/// Base de la fenetre XIP cachee, celle que visent les vecteurs d'interruption.
pub const XIP_CACHED: u32 = 0x1000_0000;

const TABLE_STRIDE: usize = 0x200;
const MAGICS: [&[u8]; 6] = [
    b"SNC7320", b"SN323200", b"SNUR00", b"SN98300", b"SONIXDEV", b"SNCSPINF",
];
const VERSION_MASK: u32 = 0x5a5a_0000;
const VERSION_V3: u32 = 0x5a5a_0033;
const ENCRYPTER_PENDING: u32 = 0x5f5f_4e45; // "EN__"

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherMode {
    Ofb,
    Cbc,
}

/// Une region decrite par la load table, adressee dans la fenetre XIP.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    pub addr: u32,
    pub len: u32,
}

impl Region {
    /// Offset correspondant dans le dump brut.
    pub fn flash_offset(&self) -> usize {
        (self.addr & 0x00FF_FFFF) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Debug, Clone)]
pub struct LoadTable {
    pub table_offset: usize,
    pub magic: String,
    pub version: u32,
    pub load_cfg: u32,
    pub encrypted: bool,
    pub mode: CipherMode,
    pub aes_key: [u8; 32],
    pub user_code: Region,
    pub sram_code: Region,
    pub dpd_code: Region,
}

fn rd32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

impl LoadTable {
    pub fn parse(buf: &[u8], base: usize) -> Option<Self> {
        if base + TABLE_STRIDE > buf.len() {
            return None;
        }
        let magic = MAGICS.iter().find(|m| buf[base..].starts_with(*m))?;

        let version = rd32(buf, base + 0x1f8);
        if version & 0xffff_0000 != VERSION_MASK {
            return None;
        }
        let mode = if version < VERSION_V3 { CipherMode::Ofb } else { CipherMode::Cbc };

        let load_cfg = rd32(buf, base + 8);
        let mut encrypted = load_cfg & 1 != 0;
        // Sur les tables V3, la marque ENCRYPTER signale une image deja remise en clair.
        if encrypted && version >= VERSION_V3 && rd32(buf, base + 0x80) == ENCRYPTER_PENDING {
            encrypted = false;
        }

        let mut aes_key = [0u8; 32];
        aes_key.copy_from_slice(&buf[base + 0x28..base + 0x48]);

        let (sram_code, dpd_code) = if version >= VERSION_V3 {
            (
                Region { addr: rd32(buf, base + 0x90), len: rd32(buf, base + 0x94) },
                Region { addr: rd32(buf, base + 0x98), len: rd32(buf, base + 0x9c) },
            )
        } else {
            (Region { addr: 0, len: 0 }, Region { addr: 0, len: 0 })
        };

        Some(Self {
            table_offset: base,
            magic: String::from_utf8_lossy(magic).into_owned(),
            version,
            load_cfg,
            encrypted,
            mode,
            aes_key,
            user_code: Region { addr: rd32(buf, base + 0x10), len: rd32(buf, base + 0x14) },
            sram_code,
            dpd_code,
        })
    }

    /// Regions non vides decrites par la table, dans l'ordre de chargement.
    pub fn regions(&self) -> Vec<Region> {
        [self.user_code, self.sram_code, self.dpd_code]
            .into_iter()
            .filter(|r| !r.is_empty())
            .collect()
    }

    /// IV de depart, derive du champ cle de la table et de la deviceKey.
    fn derive_iv(&self, device_key: u32) -> [u8; 16] {
        let mut key = self.aes_key;
        key.reverse();

        let mut iv = [0u8; 16];
        iv.copy_from_slice(&key[0x10..0x20]);
        // Chaque mot de 32 bits est masque par la deviceKey en gros boutiste.
        let dk = device_key.to_be_bytes();
        for (i, b) in iv.iter_mut().enumerate() {
            *b ^= dk[i % 4];
        }

        let key128: [u8; 16] = key[0x0..0x10].try_into().unwrap();
        Aes::new(&key128).encrypt_block(&iv)
    }

    /// Dechiffre une region en place dans le dump.
    pub fn decrypt_region(&self, flash: &mut [u8], region: Region, device_key: u32) {
        let off = region.flash_offset();
        let len = (region.len as usize) & !0xf;
        if len == 0 || off + len > flash.len() {
            return;
        }

        let iv = self.derive_iv(device_key);
        let mut key = self.aes_key;
        key.reverse();
        let aes = Aes::new(&key);

        match self.mode {
            CipherMode::Cbc => {
                // L'IV est reinitialise a chaque tranche, la puce n'ayant pas assez
                // de RAM pour traiter les 0x10000 octets d'un coup.
                const SLICE: usize = 0x1000;
                for start in (0..len).step_by(SLICE) {
                    let mut prev = iv;
                    let end = (start + SLICE).min(len);
                    for b in (start..end).step_by(16) {
                        let mut ct: [u8; 16] = flash[off + b..off + b + 16].try_into().unwrap();
                        ct.reverse();
                        let mut pt = aes.decrypt_block(&ct);
                        for i in 0..16 {
                            pt[i] ^= prev[i];
                        }
                        prev = ct;
                        pt.reverse();
                        flash[off + b..off + b + 16].copy_from_slice(&pt);
                    }
                }
            }
            CipherMode::Ofb => {
                let mut stream = iv;
                for b in (0..len).step_by(16) {
                    stream = aes.encrypt_block(&stream);
                    // Les mots du flux sont consommes en ordre inverse et permutes.
                    for w in 0..4 {
                        let s = u32::from_le_bytes(
                            stream[(3 - w) * 4..(3 - w) * 4 + 4].try_into().unwrap(),
                        );
                        let p = off + b + w * 4;
                        let v = u32::from_le_bytes(flash[p..p + 4].try_into().unwrap());
                        flash[p..p + 4].copy_from_slice(&(v ^ s.swap_bytes()).to_le_bytes());
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum SonixError {
    NoLoadTable,
}

/// Un dump de flash Sonix analyse, et dechiffre si la deviceKey est connue.
pub struct SonixImage {
    pub flash: Vec<u8>,
    pub tables: Vec<LoadTable>,
    pub was_encrypted: bool,
    pub decrypted: bool,
}

impl SonixImage {
    /// Detecte une load table a l'offset 0 sans lire le reste du dump.
    pub fn is_sonix(buf: &[u8]) -> bool {
        MAGICS.iter().any(|m| buf.starts_with(*m))
    }

    pub fn load(buf: &[u8], device_key: Option<u32>) -> Result<Self, SonixError> {
        let mut flash = buf.to_vec();

        // Le bootrom balaie plusieurs emplacements : une image peut porter une table
        // de bootloader et une table de firmware principal.
        let mut offsets = vec![0usize, 0x1000];
        let mut probe = 0x1000usize;
        while probe <= flash.len() / 2 {
            offsets.push(flash.len() - probe);
            probe *= 2;
        }

        let mut tables: Vec<LoadTable> = Vec::new();
        for off in offsets {
            if let Some(t) = LoadTable::parse(&flash, off) {
                if !tables.iter().any(|e| e.table_offset == t.table_offset) {
                    tables.push(t);
                }
            }
        }
        if tables.is_empty() {
            return Err(SonixError::NoLoadTable);
        }

        let was_encrypted = tables.iter().any(|t| t.encrypted);
        if !was_encrypted {
            return Ok(Self { flash, tables, was_encrypted, decrypted: true });
        }

        let Some(key) = device_key else {
            return Ok(Self { flash, tables, was_encrypted, decrypted: false });
        };

        let plan: Vec<(LoadTable, Vec<Region>)> = tables
            .iter()
            .filter(|t| t.encrypted)
            .map(|t| (t.clone(), t.regions()))
            .collect();
        for (table, regions) in plan {
            for r in regions {
                table.decrypt_region(&mut flash, r, key);
            }
        }

        Ok(Self { flash, tables, was_encrypted, decrypted: true })
    }

    /// Table principale, celle qui porte le point d'entree.
    pub fn primary(&self) -> &LoadTable {
        &self.tables[0]
    }
}
