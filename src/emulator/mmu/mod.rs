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
    /// Les deux convertisseurs a approximations successives.
    pub const SAR_ADC0: u32 = 0x4000_A000;
    pub const SAR_ADC1: u32 = 0x4000_B000;
    pub const I2S4: u32 = 0x4000_E000;
    pub const I2S2: u32 = 0x4001_2000;
    pub const I2S0: u32 = 0x4001_9000;
    pub const SPI1: u32 = 0x4002_0000;
    /// Controleur de la flash SPI NOR externe et son DMA.
    pub const FLASH_CTL: u32 = 0x4002_2000;
    pub const IDMA1: u32 = 0x4002_5000;
    pub const IDMA0: u32 = 0x4002_B000;
    /// Controleur de la fenetre XIP cachee. La figure 4-1 place GPIO2 ici,
    /// mais le firmware y programme la base de la fenetre 0x10000000.
    pub const XIP_CTRL: u32 = 0x4002_F000;
    pub const GPIO1: u32 = 0x4003_0000;
    pub const GPIO0: u32 = 0x4003_1000;
    pub const I2C1: u32 = 0x4003_3000;
    pub const UART1: u32 = 0x4003_4000;
    /// Accelerateur de somme de controle. La figure 4-1 annonce UART0 ici,
    /// mais le firmware y programme source, longueur, polynome et resultat.
    pub const CHECKSUM: u32 = 0x4003_8000;
    pub const WDT: u32 = 0x4003_A000;
    /// Timers CT32B1 a CT32B7, une page de 4 Ko chacun.
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
            SAR_ADC0 => "SAR_ADC0",
            SAR_ADC1 => "SAR_ADC1",
            I2S4 => "I2S4",
            I2S2 => "I2S2",
            I2S0 => "I2S0",
            SPI1 => "SPI1",
            FLASH_CTL => "FLASH_CTL",
            IDMA1 => "IDMA1",
            IDMA0 => "IDMA0",
            XIP_CTRL => "XIP_CTRL",
            GPIO1 => "GPIO1",
            GPIO0 => "GPIO0",
            I2C1 => "I2C1",
            UART1 => "UART1",
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
    /// Page dont les acces sont journalises dans l'ordre, pour reconstituer un
    /// protocole. Les compteurs seuls ne disent pas la sequence.
    pub log_page: Option<u32>,
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
        if self.log_page == Some(addr & !0xFFF) && self.log.len() < 4000 {
            self.log.push(LogEntry { pc, addr, is_write, value });
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
            map::ROM_BASE..=map::ROM_END => {
                self.boot_rom.read_u8((addr - map::ROM_BASE) as usize)
            }
            map::ICACHE_BASE..=map::ICACHE_END => {
                let off = periph.xip.flash_offset(addr - map::ICACHE_BASE);
                self.flash.read_u8(off)
            }
            map::SRAM_BASE..=map::SRAM_END => {
                self.sram.read_u8((addr - map::SRAM_BASE) as usize)
            }
            map::MAILBOX_BASE..=map::MAILBOX_END => {
                self.sram.read_mailbox_u8((addr - map::MAILBOX_BASE) as usize)
            }
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
            map::MAILBOX_BASE..=map::MAILBOX_END => {
                self.sram.write_mailbox_u8((addr - map::MAILBOX_BASE) as usize, val)
            }
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

    pub fn read_u16(&mut self, addr: u32, periph: &mut Peripherals, nvic: &Nvic) -> u16 {
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
        self.write_u8(addr, (val & 0xFF) as u8, periph, nvic);
        self.write_u8(addr + 1, ((val >> 8) & 0xFF) as u8, periph, nvic);
    }

    pub fn read_u32(&mut self, addr: u32, periph: &mut Peripherals, nvic: &Nvic) -> u32 {
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
    fn executer_transfert(&mut self, t: crate::emulator::peripherals::Transfer, p: &mut Peripherals) {
        // Une longueur aberrante trahirait un descripteur mal renseigne : on
        // borne pour ne pas parcourir toute la memoire.
        const MAX: u32 = 1 << 20;
        if t.len == 0 || t.len > MAX {
            return;
        }
        let nvic = Nvic::default();
        for i in 0..t.len {
            let octet = self.flash.read_u8((t.flash_offset + i) as usize);
            self.ecrire_octet_brut(t.mem_addr.wrapping_add(i), octet, p, &nvic);
        }
    }

    /// Ecriture d'un octet en memoire vive, sans passer par le decodage MMIO.
    fn ecrire_octet_brut(&mut self, addr: u32, val: u8, _p: &mut Peripherals, _nvic: &Nvic) {
        match addr {
            map::PRAM_BASE..=map::PRAM_END => self.pram.write_u8(addr as usize, val),
            map::SRAM_BASE..=map::SRAM_END => {
                self.sram.write_u8((addr - map::SRAM_BASE) as usize, val)
            }
            map::MAILBOX_BASE..=map::MAILBOX_END => {
                self.sram.write_mailbox_u8((addr - map::MAILBOX_BASE) as usize, val)
            }
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
                crc = if crc & 1 != 0 { (crc >> 1) ^ c.polynome } else { crc >> 1 };
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
            periph::UART1 => p.uart.read_reg(off),
            periph::GPIO0 => p.gpio.read_reg(off),
            periph::SYSCTRL0 => p.sys.read_reg(off),
            // FEUSE (0x30..0x3f) puis les registres d'horloge/PLL de SN_SYS0.
            periph::FUSES if (0x30..=0x3f).contains(&off) => p.fuses.read_reg(off),
            periph::SAR_ADC0 if crate::emulator::peripherals::SarAdc::handles(off) => {
                p.adc[0].read_reg(off)
            }
            periph::SAR_ADC1 if crate::emulator::peripherals::SarAdc::handles(off) => {
                p.adc[1].read_reg(off)
            }
            periph::FLASH_CTL => p.flashctl.read_reg(off),
            periph::XIP_CTRL => p.xip.read_reg(off),
            periph::FUSES => p.snsys.read_reg(off),
            p_ if (periph::TIMERS..=periph::TIMERS_LAST).contains(&p_) => p.timers.read_reg(off),
            _ => {
                let pc = self.current_pc;
                self.mmio_trace.record_read(addr, pc);
                0
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
            periph::UART1 => p.uart.write_reg(off, val),
            periph::GPIO0 => p.gpio.write_reg(off, val),
            periph::SYSCTRL0 => {
                if p.sys.write_reg(off, val) {
                    self.boot_rom.is_hidden = true;
                }
            }
            periph::FUSES if (0x30..=0x3f).contains(&off) => p.fuses.write_reg(off, val),
            periph::SAR_ADC0 if crate::emulator::peripherals::SarAdc::handles(off) => {
                p.adc[0].write_reg(off, val)
            }
            periph::SAR_ADC1 if crate::emulator::peripherals::SarAdc::handles(off) => {
                p.adc[1].write_reg(off, val)
            }
            periph::FLASH_CTL => {
                if let Some(t) = p.flashctl.write_reg(off, val) {
                    self.executer_transfert(t, p);
                }
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
