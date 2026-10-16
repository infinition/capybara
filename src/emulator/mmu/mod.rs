pub mod flash;
pub mod pram;
pub mod rom;
pub mod sram;

pub use flash::SpiFlash;
pub use pram::{Pram, PRAM_SIZE};
pub use rom::BootRom;
pub use sram::InternalSram;

use crate::emulator::cpu::nvic::Nvic;
use crate::emulator::peripherals::Peripherals;
use std::collections::BTreeMap;

/// Carte memoire du SNC7340, datasheet V1.7 section 4.
pub mod map {
    /// Program RAM, 64 Ko. Le bootrom y recopie le code utilisateur dechiffre.
    pub const PRAM_BASE: u32 = 0x0000_0000;
    pub const PRAM_END: u32 = 0x0000_FFFF;
    /// ROM du coeur 0, 64 Ko.
    pub const ROM_BASE: u32 = 0x0800_0000;
    pub const ROM_END: u32 = 0x0800_FFFF;
    /// Fenetre I-cache sur la flash externe, 1 Mo seulement.
    pub const ICACHE_BASE: u32 = 0x1000_0000;
    pub const ICACHE_END: u32 = 0x100F_FFFF;
    /// SRAM AHB, 128 Ko.
    pub const SRAM_BASE: u32 = 0x1800_0000;
    pub const SRAM_END: u32 = 0x1801_FFFF;
    /// Mailbox RAM partagee entre les deux coeurs, 4 Ko.
    pub const MAILBOX_BASE: u32 = 0x2000_0000;
    pub const MAILBOX_END: u32 = 0x2000_0FFF;
    /// Flash SPI NOR externe, fenetre de 256 Mo.
    pub const FLASH_BASE: u32 = 0x6000_0000;
    pub const FLASH_END: u32 = 0x6FFF_FFFF;

    pub const SRAM_SIZE: usize = 128 * 1024;
    pub const MAILBOX_SIZE: usize = 4 * 1024;

    /// Regions bit-band du Cortex-M. Chaque bit d'un octet de la region source
    /// possede son propre mot de 32 bits dans l'alias, ce qui permet de le lire
    /// ou de l'ecrire sans read-modify-write.
    pub const BITBAND_SRAM_SRC: u32 = 0x2000_0000;
    pub const BITBAND_SRAM_ALIAS: u32 = 0x2200_0000;
    pub const BITBAND_SRAM_ALIAS_END: u32 = 0x23FF_FFFF;
    pub const BITBAND_PERIPH_SRC: u32 = 0x4000_0000;
    pub const BITBAND_PERIPH_ALIAS: u32 = 0x4200_0000;
    pub const BITBAND_PERIPH_ALIAS_END: u32 = 0x43FF_FFFF;

    /// Traduit une adresse de l'alias en (adresse de l'octet vise, rang du bit).
    ///
    /// alias = base_alias + 32 * offset_octet + 4 * rang_bit
    pub fn bitband_target(addr: u32) -> Option<(u32, u32)> {
        let (alias_base, src_base) = match addr {
            BITBAND_SRAM_ALIAS..=BITBAND_SRAM_ALIAS_END => (BITBAND_SRAM_ALIAS, BITBAND_SRAM_SRC),
            BITBAND_PERIPH_ALIAS..=BITBAND_PERIPH_ALIAS_END => {
                (BITBAND_PERIPH_ALIAS, BITBAND_PERIPH_SRC)
            }
            _ => return None,
        };
        let delta = addr - alias_base;
        Some((src_base + delta / 32, (delta % 32) / 4))
    }
}

/// Bases des peripheriques, datasheet V1.7 figure 4-1.
pub mod periph {
    pub const PMU: u32 = 0x4000_1000;
    pub const ISO: u32 = 0x4000_2000;
    pub const RTC: u32 = 0x4000_3000;
    pub const SYSCTRL0: u32 = 0x4000_4000;
    pub const SYSCTRL1: u32 = 0x4000_5000;
    pub const USB: u32 = 0x4000_7000;
    pub const WDT0: u32 = 0x4000_8000;
    pub const WDT1: u32 = 0x4000_9000;
    /// Ports serie UART0 et UART1 du microcontroleur Sonix.
    pub const UART0: u32 = 0x4000_A000;
    pub const UART1: u32 = 0x4000_B000;
    /// Ancienne etiquette du datasheet pour UART1.
    pub const SAR_ADC1: u32 = UART1;
    pub const SAR_ADC0: u32 = UART0;
    /// Controleur SPI0 et registre de transmission d'ecran.
    pub const SPI0: u32 = 0x4000_E000;
    pub const I2S4: u32 = SPI0;
    /// Controleur de transferts DMA.
    pub const DMA: u32 = 0x4000_F000;
    pub const I2S2: u32 = 0x4001_2000;
    /// Ports d'entrees-sorties 0 a 2, resolus par la table de broches du
    /// firmware : un identifiant de broche encode `port = id >> 4` et
    /// `pin = id & 15`, et la table en SRAM rend le decalage a ajouter a
    /// 0x40018000.
    pub const GPIO_PORT0: u32 = 0x4001_8000;
    pub const GPIO_PORT1: u32 = 0x4001_9000;
    /// Ses broches 0 et 1 sont lues au demarrage.
    pub const GPIO_PORT2: u32 = 0x4001_A000;
    pub const SPI1: u32 = 0x4002_0000;
    /// Controleur de la flash SPI NOR externe et son DMA.
    pub const FLASH_CTL: u32 = 0x4002_2000;
    pub const IDMA1: u32 = 0x4002_5000;
    pub const IDMA0: u32 = 0x4002_B000;
    /// Controleur de la fenetre XIP cachee.
    pub const XIP_CTRL: u32 = 0x4002_F000;
    pub const GPIO1: u32 = 0x4003_0000;
    pub const GPIO0: u32 = 0x4003_1000;
    pub const I2C1: u32 = 0x4003_3000;
    /// Accelerateur de somme de controle.
    pub const CHECKSUM: u32 = 0x4003_8000;
    pub const WDT: u32 = 0x4003_A000;
    /// Timers CT32B0 a CT32B7, une page de 4 Ko chacun (0x40000000 a 0x40007000).
    pub const TIMERS: u32 = 0x4004_0000;
    pub const TIMERS_LAST: u32 = 0x4004_6000;
    /// Zone systeme SN_SYS0, porteuse des fusibles FEUSE.
    pub const FUSES: u32 = 0x4500_0000;

    /// Nom lisible d'une page de peripherique, pour le journal de trace.
    pub fn name_of(page: u32) -> &'static str {
        match page {
            PMU => "PMU",
            ISO => "ISO",
            RTC => "RTC",
            SYSCTRL0 => "SYSCTRL0",
            SYSCTRL1 => "SYSCTRL1",
            USB => "USB",
            UART0 => "UART0",
            UART1 => "UART1",
            SPI0 => "SPI0",
            DMA => "DMA",
            I2S2 => "I2S2",
            GPIO_PORT0 => "GPIO_P0",
            GPIO_PORT1 => "GPIO_P1",
            GPIO_PORT2 => "GPIO_P2",
            SPI1 => "SPI1",
            FLASH_CTL => "FLASH_CTL",
            IDMA1 => "IDMA1",
            IDMA0 => "IDMA0",
            XIP_CTRL => "XIP_CTRL",
            GPIO1 => "GPIO1",
            GPIO0 => "GPIO0",
            I2C1 => "I2C1",
            CHECKSUM => "CHECKSUM",
            WDT => "WDT",
            FUSES => "SN_SYS0",
            p if (TIMERS..=TIMERS_LAST).contains(&p) => "CT32B",
            _ => "?",
        }
    }
}

/// Compteurs d'acces aux registres non modelises.
///
/// C'est l'outil de reverse : on laisse tourner le vrai firmware et on releve
/// ce qu'il touche, pour savoir quel peripherique implementer ensuite.
#[derive(Debug, Clone, Copy, Default)]
pub struct MmioStat {
    pub reads: u64,
    pub writes: u64,
    pub last_write: u32,
    /// Adresse de l'instruction ayant fait le premier acces, pour retrouver le
    /// code responsable sans avoir a le chercher a la main.
    pub first_pc: u32,
}

#[derive(Default)]
pub struct MmioTrace {
    pub enabled: bool,
    /// Registres touches sans modele derriere.
    pub unknown: BTreeMap<u32, MmioStat>,
    /// Tous les registres peripheriques touches, modelises ou non.
    pub all: BTreeMap<u32, MmioStat>,
    /// Adresses qui ne tombent dans aucune region de la carte memoire.
    pub off_map: BTreeMap<u32, MmioStat>,
    /// Valeurs imposees en lecture sur des registres non modelises, pour tester
    /// une hypothese sans ecrire de peripherique. Alimente par MMIO_FORCE.
    pub forcees: BTreeMap<u32, u32>,
    /// Page dont les acces sont journalises dans l'ordre, pour reconstituer un
    /// protocole. Les compteurs seuls ne disent pas la sequence.
    pub log_page: Option<u32>,
    /// Ne journaliser que les ecritures. Une boucle de scrutation noie sinon la
    /// sequence de configuration sous des millions de lectures identiques.
    pub log_ecritures_seules: bool,
    /// Intervalle de PC dont on journalise les acces, quelle que soit la page.
    ///
    /// Une page ne dit pas d'ou vient un acces : quand on cherche par ou un
    /// module precis parle au materiel, c'est l'appelant qu'il faut filtrer, pas
    /// l'adresse touchee.
    pub log_pc: Option<(u32, u32)>,
    pub log: Vec<LogEntry>,
}

/// Un acces journalise, dans l'ordre d'execution.
#[derive(Debug, Clone, Copy)]
pub struct LogEntry {
    pub pc: u32,
    pub addr: u32,
    pub is_write: bool,
    pub value: u32,
}

impl MmioTrace {
    /// Journalise un acces si sa page est celle observee. Le journal est borne
    /// pour ne pas gonfler indefiniment sur une boucle de scrutation.
    fn journalise(&mut self, addr: u32, is_write: bool, value: u32, pc: u32) {
        if self.log_ecritures_seules && !is_write {
            return;
        }
        let par_page = self.log_page == Some(addr & !0xFFF);
        let par_pc = self
            .log_pc
            .is_some_and(|(bas, haut)| (bas..haut).contains(&pc));
        if (par_page || par_pc) && self.log.len() < 60000 {
            self.log.push(LogEntry {
                pc,
                addr,
                is_write,
                value,
            });
        }
    }

    fn record_read(&mut self, addr: u32, pc: u32) {
        if self.enabled {
            let e = self.unknown.entry(addr).or_default();
            if e.reads == 0 && e.writes == 0 {
                e.first_pc = pc;
            }
            e.reads += 1;
        }
    }

    fn record_write(&mut self, addr: u32, val: u32, pc: u32) {
        if self.enabled {
            let e = self.unknown.entry(addr).or_default();
            if e.reads == 0 && e.writes == 0 {
                e.first_pc = pc;
            }
            e.writes += 1;
            e.last_write = val;
        }
    }

    fn record_any_read(&mut self, addr: u32, pc: u32, valeur: u32) {
        self.journalise(addr, false, valeur, pc);
        if self.enabled {
            let e = self.all.entry(addr).or_default();
            if e.reads == 0 && e.writes == 0 {
                e.first_pc = pc;
            }
            e.reads += 1;
        }
    }

    fn record_any_write(&mut self, addr: u32, val: u32, pc: u32) {
        self.journalise(addr, true, val, pc);
        if self.enabled {
            let e = self.all.entry(addr).or_default();
            if e.reads == 0 && e.writes == 0 {
                e.first_pc = pc;
            }
            e.writes += 1;
            e.last_write = val;
        }
    }

    fn record_off_map_read(&mut self, addr: u32, pc: u32) {
        if self.enabled {
            let e = self.off_map.entry(addr & !3).or_default();
            if e.reads == 0 && e.writes == 0 {
                e.first_pc = pc;
            }
            e.reads += 1;
        }
    }

    fn record_off_map_write(&mut self, addr: u32, val: u32, pc: u32) {
        if self.enabled {
            let e = self.off_map.entry(addr & !3).or_default();
            if e.reads == 0 && e.writes == 0 {
                e.first_pc = pc;
            }
            e.writes += 1;
            e.last_write = val;
        }
    }

    pub fn clear(&mut self) {
        self.unknown.clear();
        self.all.clear();
        self.off_map.clear();
        self.log.clear();
    }

    /// Meme classement que hottest, mais sur l'ensemble des acces peripheriques.
    pub fn hottest_all(&self, count: usize) -> Vec<(u32, &'static str, MmioStat)> {
        let mut v: Vec<_> = self
            .all
            .iter()
            .map(|(a, s)| (*a, periph::name_of(*a & !0xFFF), *s))
            .collect();
        v.sort_by_key(|(_, _, s)| std::cmp::Reverse(s.reads + s.writes));
        v.truncate(count);
        v
    }

    /// Registres les plus sollicites, avec le peripherique auquel ils appartiennent.
    pub fn hottest(&self, count: usize) -> Vec<(u32, &'static str, MmioStat)> {
        let mut v: Vec<_> = self
            .unknown
            .iter()
            .map(|(a, s)| (*a, periph::name_of(*a & !0xFFF), *s))
            .collect();
        v.sort_by_key(|(_, _, s)| std::cmp::Reverse(s.reads + s.writes));
        v.truncate(count);
        v
    }
}

pub struct MemoryBus {
    /// Adresse de l'instruction en cours, renseignee par le coeur avant chaque
    /// execution. Sert uniquement a attribuer les acces dans la trace.
    pub current_pc: u32,
    /// True once a store has happened since the last clear.
    ///
    /// The core uses this to recognise a wait loop: an iteration that writes
    /// nowhere and leaves every register identical can only be broken by a
    /// peripheral. The flag is set by the three write entry points and cleared
    /// by the core on each pass through the loop head.
    pub a_ecrit: bool,
    /// Stack floor of the tracked loop, or zero.
    ///
    /// Anything stored in RAM **below** this floor is dead memory as far as the
    /// ABI is concerned: it is the area functions called from the loop use for
    /// their own register saves, and that the next iteration will rewrite
    /// before reading. A store down there therefore does not count as a write.
    /// Without this exemption every wait written `while (!ready()) ;` — the
    /// vast majority — escaped recognition, `ready`'s PUSH being enough to
    /// disqualify each iteration.
    pub plancher_pile: u32,
    /// Bytes of RAM to present to the core with some bits forced, given as
    /// address, bits to set and bits to clear.
    ///
    /// Writing such a byte from outside once a second is a race the firmware
    /// wins: it sets its flag and reads it back a few instructions later, long
    /// before the next refresh. Forcing the value as it is read settles the
    /// question instead of competing with it.
    ///
    /// Only single-byte reads are masked. Every flag this exists for is read
    /// with `LDRB`, and the half-word and word paths are the two hottest
    /// functions in the emulator: a test on each of them is paid on every
    /// access the firmware makes, for a list that is empty unless someone has
    /// asked for a flag to be held.
    pub masques_lecture: Vec<(u32, u8, u8)>,
    pub flash: SpiFlash,
    pub pram: Pram,
    pub sram: InternalSram,
    pub boot_rom: BootRom,
    pub mmio_trace: MmioTrace,
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self {
            current_pc: 0,
            a_ecrit: false,
            plancher_pile: 0,
            masques_lecture: Vec::new(),
            flash: SpiFlash::default(),
            pram: Pram::default(),
            sram: InternalSram::default(),
            boot_rom: BootRom::default(),
            mmio_trace: MmioTrace::default(),
        }
    }
}

impl MemoryBus {
    pub fn read_u8(&mut self, addr: u32, periph: &mut Peripherals, nvic: &Nvic) -> u8 {
        // Memoire vive et PRAM d'abord : ce sont les deux zones que le code lit
        // sans arret, et aucune des deux ne peut tomber dans un alias bit-band.
        if (map::SRAM_BASE..=map::SRAM_END).contains(&addr) {
            let v = self.sram.read_u8((addr - map::SRAM_BASE) as usize);
            if self.masques_lecture.is_empty() {
                return v;
            }
            return self.masquer(addr, v);
        }
        if addr <= map::PRAM_END {
            return self.pram.read_u8(addr as usize);
        }
        // Alias bit-band : le mot lu vaut 0 ou 1 selon l'etat du bit vise.
        if let Some((target, bit)) = map::bitband_target(addr & !3) {
            if addr & 3 != 0 {
                return 0;
            }
            let byte = self.read_u8(target, periph, nvic);
            return (byte >> bit) & 1;
        }
        match addr {
            map::PRAM_BASE..=map::PRAM_END => self.pram.read_u8(addr as usize),
            map::ROM_BASE..=map::ROM_END => self.boot_rom.read_u8((addr - map::ROM_BASE) as usize),
            map::ICACHE_BASE..=map::ICACHE_END => {
                let off = periph.xip.flash_offset(addr - map::ICACHE_BASE);
                self.flash.read_u8(off)
            }
            map::SRAM_BASE..=map::SRAM_END => self.sram.read_u8((addr - map::SRAM_BASE) as usize),
            map::MAILBOX_BASE..=map::MAILBOX_END => self
                .sram
                .read_mailbox_u8((addr - map::MAILBOX_BASE) as usize),
            0x4000_0000..=0x4FFF_FFFF => {
                let aligned = addr & !3;
                let val = self.read_mmio_u32(aligned, periph);
                ((val >> ((addr & 3) * 8)) & 0xFF) as u8
            }
            map::FLASH_BASE..=map::FLASH_END => {
                self.flash.read_u8((addr - map::FLASH_BASE) as usize)
            }
            0xE000_E000..=0xE000_EFFF => {
                let val = nvic.read_reg(addr & !3);
                ((val >> ((addr & 3) * 8)) & 0xFF) as u8
            }
            _ => {
                let pc = self.current_pc;
                self.mmio_trace.record_off_map_read(addr, pc);
                0
            }
        }
    }

    pub fn write_u8(&mut self, addr: u32, val: u8, periph: &mut Peripherals, nvic: &mut Nvic) {
        self.noter_ecriture(addr);
        // Alias bit-band : seul le bit de poids faible de la valeur compte, et
        // il ne modifie que le bit vise de l'octet source.
        if let Some((target, bit)) = map::bitband_target(addr & !3) {
            if addr & 3 != 0 {
                return;
            }
            let mut byte = self.read_u8(target, periph, nvic);
            if val & 1 != 0 {
                byte |= 1 << bit;
            } else {
                byte &= !(1 << bit);
            }
            self.write_u8(target, byte, periph, nvic);
            return;
        }
        match addr {
            map::PRAM_BASE..=map::PRAM_END => self.pram.write_u8(addr as usize, val),
            map::ICACHE_BASE..=map::ICACHE_END => {
                let off = periph.xip.flash_offset(addr - map::ICACHE_BASE);
                self.flash.write_u8(off, val)
            }
            map::SRAM_BASE..=map::SRAM_END => {
                self.sram.write_u8((addr - map::SRAM_BASE) as usize, val)
            }
            map::MAILBOX_BASE..=map::MAILBOX_END => self
                .sram
                .write_mailbox_u8((addr - map::MAILBOX_BASE) as usize, val),
            0x4000_0000..=0x4FFF_FFFF => {
                let aligned = addr & !3;
                let mut current = self.read_mmio_u32(aligned, periph);
                let shift = (addr & 3) * 8;
                current &= !(0xFF << shift);
                current |= (val as u32) << shift;
                self.write_mmio_u32(aligned, current, periph);
            }
            map::FLASH_BASE..=map::FLASH_END => {
                self.flash.write_u8((addr - map::FLASH_BASE) as usize, val)
            }
            0xE000_E000..=0xE000_EFFF => {
                let aligned = addr & !3;
                let mut current = nvic.read_reg(aligned);
                let shift = (addr & 3) * 8;
                current &= !(0xFF << shift);
                current |= (val as u32) << shift;
                nvic.write_reg(aligned, current);
            }
            _ => {
                let pc = self.current_pc;
                self.mmio_trace.record_off_map_write(addr, val as u32, pc)
            }
        }
    }

    /// Recuperation d'instruction : le demi mot vise et le suivant, en une
    /// seule resolution de region.
    ///
    /// Le coeur lisait un demi mot par appel, deux pour une instruction longue,
    /// et un tiers du code en est fait. Chaque appel refaisait le decodage
    /// d'adresse et, dans la fenetre XIP, le calcul de l'offset flash. Le code
    /// ne vit qu'en PRAM et dans cette fenetre : on y prend quatre octets d'un
    /// coup. Le second demi mot est lu meme quand l'instruction est courte, ce
    /// qui ne coute rien et n'a aucun effet de bord dans ces deux memoires.
    #[inline(always)]
    pub fn fetch_pair(&self, addr: u32, periph: &Peripherals) -> Option<(u16, u16)> {
        let quatre: &[u8] = if addr < map::PRAM_END {
            let i = addr as usize;
            self.pram.data.get(i..i + 4)?
        } else if (map::ICACHE_BASE..map::ICACHE_END).contains(&addr) {
            let o = periph.xip.flash_offset(addr - map::ICACHE_BASE);
            self.flash.data.get(o..o + 4)?
        } else {
            return None;
        };
        Some((
            u16::from_le_bytes([quatre[0], quatre[1]]),
            u16::from_le_bytes([quatre[2], quatre[3]]),
        ))
    }

    /// Applies the read masks to a byte just fetched from RAM.
    ///
    /// The list is almost always empty, and never longer than a handful, so a
    /// linear walk costs less than any structure that would index it.
    #[inline(always)]
    fn masquer(&self, addr: u32, octet: u8) -> u8 {
        let mut v = octet;
        for (a, poser, effacer) in &self.masques_lecture {
            if *a == addr {
                v = (v | poser) & !effacer;
            }
        }
        v
    }

    /// Sets the write flag, except for dead memory below the stack floor.
    /// Program memory and peripherals always count: they sit below RAM in the
    /// address map, so the first condition keeps them out of the exemption.
    #[inline(always)]
    fn noter_ecriture(&mut self, addr: u32) {
        if addr >= map::SRAM_BASE && addr < self.plancher_pile {
            return;
        }
        self.a_ecrit = true;
    }

    /// Index of the first byte of a block of words, if it fits entirely in RAM
    /// and is aligned.
    ///
    /// Multi-register accesses — PUSH, POP, LDM, STM — went through full bus
    /// decoding once per register: a prologue storing five words redid the same
    /// addressing work five times, while the stack always lives in SRAM, which
    /// has neither bit-band aliases nor side effects. One test therefore covers
    /// the whole burst.
    #[inline(always)]
    fn plage_sram(&self, addr: u32, mots: usize) -> Option<usize> {
        if addr < map::SRAM_BASE || (addr & 3) != 0 {
            return None;
        }
        let debut = (addr - map::SRAM_BASE) as usize;
        let long = mots.checked_mul(4)?;
        if debut.checked_add(long)? <= self.sram.data.len() {
            Some(debut)
        } else {
            None
        }
    }

    /// Reads `out.len()` consecutive words in one go.
    ///
    /// Returns false when the range is not entirely in RAM: the caller then
    /// falls back to one word at a time through the general path.
    #[inline]
    pub fn lire_mots(&self, addr: u32, out: &mut [u32]) -> bool {
        let Some(debut) = self.plage_sram(addr, out.len()) else {
            return false;
        };
        // One slice, checked once: the compiler then knows each chunk is
        // exactly four bytes and rechecks nothing. Indexing byte by byte left
        // it four bounds tests per word, which cancelled the gain over the
        // general path.
        let source = &self.sram.data[debut..debut + out.len() * 4];
        for (m, octets) in out.iter_mut().zip(source.chunks_exact(4)) {
            *m = u32::from_le_bytes([octets[0], octets[1], octets[2], octets[3]]);
        }
        true
    }

    /// Mirror of `lire_mots`.
    #[inline]
    pub fn ecrire_mots(&mut self, addr: u32, vals: &[u32]) -> bool {
        self.noter_ecriture(addr);
        let Some(debut) = self.plage_sram(addr, vals.len()) else {
            return false;
        };
        let cible = &mut self.sram.data[debut..debut + vals.len() * 4];
        for (octets, v) in cible.chunks_exact_mut(4).zip(vals) {
            octets.copy_from_slice(&v.to_le_bytes());
        }
        true
    }

    pub fn read_u16(&mut self, addr: u32, periph: &mut Peripherals, nvic: &Nvic) -> u16 {
        // Chemin rapide de la recuperation d'instruction. Le code ne vit qu'en
        // PRAM et dans la fenetre XIP, et le coeur y lit un demi-mot avant
        // chaque instruction, deux pour les longues. Repasser par le decodage
        // complet du bus, avec ses tests de bit-band et ses deux lectures
        // d'octet, coutait plus cher que tout le reste de l'execution.
        if addr < map::PRAM_END {
            let i = addr as usize;
            if i + 1 < self.pram.data.len() {
                return u16::from_le_bytes([self.pram.data[i], self.pram.data[i + 1]]);
            }
        } else if (map::ICACHE_BASE..map::ICACHE_END).contains(&addr) {
            let o = periph.xip.flash_offset(addr - map::ICACHE_BASE);
            if o + 1 < self.flash.data.len() {
                return u16::from_le_bytes([self.flash.data[o], self.flash.data[o + 1]]);
            }
        }
        // La memoire vive est la zone de donnees du jeu : chaque LDRH y passait
        // par deux decodages d'adresse complets.
        if (map::SRAM_BASE..map::SRAM_END).contains(&addr) {
            let i = (addr - map::SRAM_BASE) as usize;
            if i + 1 < self.sram.data.len() {
                return u16::from_le_bytes([self.sram.data[i], self.sram.data[i + 1]]);
            }
        }
        if map::bitband_target(addr).is_some() {
            return self.read_u32(addr & !3, periph, nvic) as u16;
        }
        // Un registre peut avoir un effet de bord a la lecture, typiquement une
        // FIFO. On ne le lit donc qu'une fois, puis on extrait les octets voulus.
        if let 0x4000_0000..=0x4FFF_FFFF = addr {
            let val = self.read_mmio_u32(addr & !3, periph);
            return ((val >> ((addr & 3) * 8)) & 0xFFFF) as u16;
        }
        let b0 = self.read_u8(addr, periph, nvic) as u16;
        let b1 = self.read_u8(addr + 1, periph, nvic) as u16;
        b0 | (b1 << 8)
    }

    pub fn write_u16(&mut self, addr: u32, val: u16, periph: &mut Peripherals, nvic: &mut Nvic) {
        self.noter_ecriture(addr);
        // Meme raison qu'en lecture : la memoire vive est le cas courant, et
        // elle n'a ni alias bit-band ni effet de bord.
        if (map::SRAM_BASE..map::SRAM_END).contains(&addr) {
            let i = (addr - map::SRAM_BASE) as usize;
            if i + 1 < self.sram.data.len() {
                self.sram.data[i] = (val & 0xFF) as u8;
                self.sram.data[i + 1] = (val >> 8) as u8;
                return;
            }
        }
        self.write_u8(addr, (val & 0xFF) as u8, periph, nvic);
        self.write_u8(addr + 1, ((val >> 8) & 0xFF) as u8, periph, nvic);
    }

    pub fn read_u32(&mut self, addr: u32, periph: &mut Peripherals, nvic: &Nvic) -> u32 {
        // Chemin rapide des deux memoires. Le mot est la largeur courante d'un
        // programme ARM, et le chemin general finissait par quatre lectures
        // d'octet, chacune refaisant tout le decodage d'adresse.
        if (map::SRAM_BASE..map::SRAM_END).contains(&addr) {
            let i = (addr - map::SRAM_BASE) as usize;
            if i + 3 < self.sram.data.len() {
                return u32::from_le_bytes([
                    self.sram.data[i],
                    self.sram.data[i + 1],
                    self.sram.data[i + 2],
                    self.sram.data[i + 3],
                ]);
            }
        } else if addr < map::PRAM_END {
            let i = addr as usize;
            if i + 3 < self.pram.data.len() {
                return u32::from_le_bytes([
                    self.pram.data[i],
                    self.pram.data[i + 1],
                    self.pram.data[i + 2],
                    self.pram.data[i + 3],
                ]);
            }
        }
        // L'alias bit-band tombe dans la plage MMIO : il doit etre resolu avant
        // le dispatch vers les peripheriques, sinon il est pris pour un registre.
        if let Some((target, bit)) = map::bitband_target(addr) {
            return ((self.read_u8(target, periph, nvic) >> bit) & 1) as u32;
        }
        // Meme raison que pour read_u16 : un seul acces au registre, symetrique
        // de ce que fait deja write_u32.
        match addr {
            0x4000_0000..=0x4FFF_FFFF => return self.read_mmio_u32(addr & !3, periph),
            0xE000_E000..=0xE000_EFFF => return nvic.read_reg(addr & !3),
            _ => {}
        }
        let b0 = self.read_u8(addr, periph, nvic) as u32;
        let b1 = self.read_u8(addr + 1, periph, nvic) as u32;
        let b2 = self.read_u8(addr + 2, periph, nvic) as u32;
        let b3 = self.read_u8(addr + 3, periph, nvic) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    pub fn write_u32(&mut self, addr: u32, val: u32, periph: &mut Peripherals, nvic: &mut Nvic) {
        self.noter_ecriture(addr);
        // Meme raison qu'en lecture : la memoire vive n'a ni alias bit-band ni
        // effet de bord, et c'est la qu'atterrit la quasi totalite des mots
        // ranges par le jeu.
        if (map::SRAM_BASE..map::SRAM_END).contains(&addr) {
            let i = (addr - map::SRAM_BASE) as usize;
            if i + 3 < self.sram.data.len() {
                self.sram.data[i..i + 4].copy_from_slice(&val.to_le_bytes());
                return;
            }
        }
        if let Some((target, bit)) = map::bitband_target(addr) {
            let mut byte = self.read_u8(target, periph, nvic);
            if val & 1 != 0 {
                byte |= 1 << bit;
            } else {
                byte &= !(1 << bit);
            }
            self.write_u8(target, byte, periph, nvic);
            return;
        }
        match addr {
            0x4000_0000..=0x4FFF_FFFF => self.write_mmio_u32(addr, val, periph),
            0xE000_E000..=0xE000_EFFF => nvic.write_reg(addr, val),
            _ => {
                self.write_u8(addr, (val & 0xFF) as u8, periph, nvic);
                self.write_u8(addr + 1, ((val >> 8) & 0xFF) as u8, periph, nvic);
                self.write_u8(addr + 2, ((val >> 16) & 0xFF) as u8, periph, nvic);
                self.write_u8(addr + 3, ((val >> 24) & 0xFF) as u8, periph, nvic);
            }
        }
    }

    /// Realise la copie demandee par le DMA du controleur de flash.
    ///
    /// Le controleur ne voit pas la memoire ; c'est ici qu'on lit la flash et
    /// qu'on ecrit la destination, en passant par les memes chemins que le
    /// coeur pour que la region visee soit resolue normalement.
    fn executer_transfert(
        &mut self,
        t: crate::emulator::peripherals::Transfer,
        p: &mut Peripherals,
    ) {
        // Une longueur aberrante trahirait un descripteur mal renseigne : on
        // borne pour ne pas parcourir toute la memoire.
        const MAX: u32 = 1 << 20;
        if t.len == 0 || t.len > MAX {
            return;
        }
        let nvic = Nvic::default();
        for i in 0..t.len {
            if t.vers_memoire {
                let octet = self.flash.read_u8((t.flash_offset + i) as usize);
                self.ecrire_octet_brut(t.mem_addr.wrapping_add(i), octet, p, &nvic);
            } else {
                // Sens inverse : la sauvegarde du jeu remonte en flash. Sans ce
                // chemin, le firmware relit l'ancienne page et son controle de
                // coherence echoue sur une somme qui ne correspond pas.
                let octet = self.read_u8(t.mem_addr.wrapping_add(i), p, &nvic);
                self.flash.write_u8((t.flash_offset + i) as usize, octet);
            }
        }
    }

    /// Realise la copie demandee par un canal du controleur de transferts.
    ///
    /// La destination du pilote d'ecran est un registre de peripherique, donc
    /// fixe ; une destination en memoire, elle, avance comme la source. Meme
    /// regle pour la source, ce qui couvre les deux sens sans registre de
    /// direction, dont le role n'est pas encore etabli.
    fn executer_transfert_dma(
        &mut self,
        t: crate::emulator::peripherals::dma::Transfert,
        p: &mut Peripherals,
    ) {
        use crate::emulator::peripherals::display::PANNEAU_DONNEES;
        use crate::emulator::peripherals::dma::LARGEUR_UNITE;

        const MAX: u32 = 1 << 20;
        let est_peripherique = |a: u32| (0x4000_0000..0x5000_0000).contains(&a);
        if t.unites == 0 || t.unites > MAX {
            p.dma.irq_a_lever = true;
            return;
        }
        let mut nvic = Nvic::default();
        let pas_source = if est_peripherique(t.source) {
            0
        } else {
            LARGEUR_UNITE
        };
        let pas_dest = if est_peripherique(t.destination) {
            0
        } else {
            LARGEUR_UNITE
        };
        // Une trame poussee vers le panneau est aussi rendue a l'afficheur : le
        // firmware ne fait jamais ecrire l'ecran par le coeur.
        let vers_panneau = t.destination == PANNEAU_DONNEES;
        let mut trame: Vec<u16> = Vec::new();
        if vers_panneau {
            trame.reserve(t.unites as usize);
        }
        if vers_panneau && pas_source == LARGEUR_UNITE {
            // Cas de loin le plus frequent : seize mille unites par trame,
            // soixante fois par seconde. Les lire une par une a travers le bus,
            // puis les ecrire une par une dans un registre dont seul le total
            // compte, coutait deux millions d'acces par seconde de console pour
            // rien.
            for i in 0..t.unites {
                trame.push(self.read_u16(t.source.wrapping_add(i * pas_source), p, &nvic));
            }
        } else {
            for i in 0..t.unites {
                let src = t.source.wrapping_add(i * pas_source);
                let dst = t.destination.wrapping_add(i * pas_dest);
                let unite = self.read_u16(src, p, &nvic);
                if vers_panneau {
                    trame.push(unite);
                }
                self.write_u16(dst, unite, p, &mut nvic);
            }
        }
        if vers_panneau {
            p.display.recevoir_trame(&trame);
        }
        p.dma.canaux[t.canal].ctrl &= !crate::emulator::peripherals::dma::DEPART;
        p.dma.irq_a_lever = true;
    }

    /// Ecriture d'un octet en memoire vive, sans passer par le decodage MMIO.
    fn ecrire_octet_brut(&mut self, addr: u32, val: u8, _p: &mut Peripherals, _nvic: &Nvic) {
        match addr {
            map::PRAM_BASE..=map::PRAM_END => self.pram.write_u8(addr as usize, val),
            map::SRAM_BASE..=map::SRAM_END => {
                self.sram.write_u8((addr - map::SRAM_BASE) as usize, val)
            }
            map::MAILBOX_BASE..=map::MAILBOX_END => self
                .sram
                .write_mailbox_u8((addr - map::MAILBOX_BASE) as usize, val),
            _ => {}
        }
    }

    /// Realise la somme de controle demandee par l'accelerateur.
    ///
    /// Comme pour le DMA de la flash, le peripherique ne voit pas la memoire :
    /// c'est le bus qui parcourt la zone source.
    fn executer_calcul(&mut self, c: crate::emulator::peripherals::Calcul, p: &mut Peripherals) {
        const MAX: u32 = 1 << 20;
        if c.length > MAX {
            return;
        }
        let nvic = Nvic::default();
        let mut crc: u16 = 0;
        for i in 0..c.length {
            let octet = self.read_u8(c.source.wrapping_add(i), p, &nvic);
            crc ^= octet as u16;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ c.polynome
                } else {
                    crc >> 1
                };
            }
        }
        p.crc.resultat = crc as u32;
    }

    fn read_mmio_u32(&mut self, addr: u32, p: &mut Peripherals) -> u32 {
        let pc = self.current_pc;
        let valeur = self.lire_mmio(addr, p);
        // La valeur rendue n'est connue qu'apres le dispatch : journaliser avant
        // aurait consigne zero pour toutes les lectures.
        self.mmio_trace.record_any_read(addr, pc, valeur);
        valeur
    }

    fn lire_mmio(&mut self, addr: u32, p: &mut Peripherals) -> u32 {
        let page = addr & !0xFFF;
        let off = addr & 0xFFF;
        match page {
            periph::CHECKSUM => p.crc.read_reg(off),
            // 0x4000E000 et 0x40007000 restent hors modele, et 0x4000A000
            // (UART0) aussi. La premiere porte l'interface serie de la dalle :
            // la modeliser en SPI ordinaire intercepte les registres que le
            // pilote d'ecran scrute, et l'ecran reste noir quoi qu'on charge.
            // Les rendre a la voie non modelisee, qui rend zero et journalise,
            // est ce qui marche. 0x4000B000, lui, est route vers l'UART : c'est
            // UART1, mesure, pas l'ADC qu'annonce l'etiquette SAR_ADC1.
            periph::SAR_ADC0 if crate::emulator::peripherals::SarAdc::handles(off) => {
                p.adc[0].read_reg(off)
            }
            periph::UART1 if crate::emulator::peripherals::UartController::handles(off) => {
                p.uart.read_reg(off)
            }
            periph::PMU if crate::emulator::peripherals::PmuController::handles(off) => {
                p.pmu.read_reg(off)
            }
            periph::GPIO0 => p.gpio.read_reg(off),
            periph::SYSCTRL0 => p.sys.read_reg(off),
            // FEUSE (0x30..0x3f) puis les registres d'horloge/PLL de SN_SYS0.
            periph::FUSES if (0x30..=0x3f).contains(&off) => p.fuses.read_reg(off),
            periph::WDT if crate::emulator::peripherals::AdcPile::handles(off) => {
                p.adc_pile.read_reg(off)
            }
            periph::DMA if crate::emulator::peripherals::DmaController::handles(off) => {
                p.dma.read_reg(off)
            }
            periph::GPIO_PORT0 if crate::emulator::peripherals::GpioPort::handles(off) => {
                p.port0.read_reg(off)
            }
            periph::GPIO_PORT1 if crate::emulator::peripherals::GpioPort::handles(off) => {
                p.port1.read_reg(off)
            }
            periph::GPIO_PORT2 if crate::emulator::peripherals::GpioPort::handles(off) => {
                p.port2.read_reg(off)
            }
            periph::FLASH_CTL => p.flashctl.read_reg(off),
            periph::XIP_CTRL => p.xip.read_reg(off),
            periph::FUSES => p.snsys.read_reg(off),
            p_ if (periph::TIMERS..=periph::TIMERS_LAST).contains(&p_) => p.timers.read_reg(off),
            _ => {
                let pc = self.current_pc;
                self.mmio_trace.record_read(addr, pc);
                self.mmio_trace.forcees.get(&addr).copied().unwrap_or(0)
            }
        }
    }

    fn write_mmio_u32(&mut self, addr: u32, val: u32, p: &mut Peripherals) {
        let pc = self.current_pc;
        self.mmio_trace.record_any_write(addr, val, pc);
        let page = addr & !0xFFF;
        let off = addr & 0xFFF;
        match page {
            periph::CHECKSUM => {
                if let Some(c) = p.crc.write_reg(off, val) {
                    self.executer_calcul(c, p);
                }
            }
            periph::SAR_ADC0 if crate::emulator::peripherals::SarAdc::handles(off) => {
                p.adc[0].write_reg(off, val)
            }
            periph::UART1 if crate::emulator::peripherals::UartController::handles(off) => {
                p.uart.write_reg(off, val)
            }
            periph::PMU if crate::emulator::peripherals::PmuController::handles(off) => {
                p.pmu.write_reg(off, val);
            }
            periph::GPIO0 => p.gpio.write_reg(off, val),
            periph::SYSCTRL0 => {
                if p.sys.write_reg(off, val) {
                    self.boot_rom.is_hidden = true;
                }
            }
            periph::FUSES if (0x30..=0x3f).contains(&off) => p.fuses.write_reg(off, val),
            periph::FLASH_CTL => {
                if let Some(t) = p.flashctl.write_reg(off, val) {
                    self.executer_transfert(t, p);
                }
            }
            periph::WDT if crate::emulator::peripherals::AdcPile::handles(off) => {
                p.adc_pile.write_reg(off, val)
            }
            periph::DMA if crate::emulator::peripherals::DmaController::handles(off) => {
                if let Some(t) = p.dma.write_reg(off, val) {
                    self.executer_transfert_dma(t, p);
                }
            }
            periph::GPIO_PORT0 if crate::emulator::peripherals::GpioPort::handles(off) => {
                p.port0.write_reg(off, val)
            }
            periph::GPIO_PORT1 if crate::emulator::peripherals::GpioPort::handles(off) => {
                p.port1.write_reg(off, val)
            }
            periph::GPIO_PORT2 if crate::emulator::peripherals::GpioPort::handles(off) => {
                p.port2.write_reg(off, val)
            }
            periph::XIP_CTRL => p.xip.write_reg(off, val),
            periph::FUSES => p.snsys.write_reg(off, val),
            p_ if (periph::TIMERS..=periph::TIMERS_LAST).contains(&p_) => {
                p.timers.write_reg(off, val)
            }
            _ => {
                let pc = self.current_pc;
                self.mmio_trace.record_write(addr, val, pc)
            }
        }
    }
}
