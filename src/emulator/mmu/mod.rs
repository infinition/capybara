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
}

/// Bases des peripheriques, datasheet V1.7 figure 4-1.
pub mod periph {
    pub const PMU: u32 = 0x4000_1000;
    pub const ISO: u32 = 0x4000_2000;
    pub const RTC: u32 = 0x4000_3000;
    pub const SYSCTRL0: u32 = 0x4000_4000;
    pub const SYSCTRL1: u32 = 0x4000_5000;
    pub const USB: u32 = 0x4000_7000;
    pub const SAR_ADC: u32 = 0x4000_A000;
    pub const I2S4: u32 = 0x4000_E000;
    pub const I2S2: u32 = 0x4001_2000;
    pub const I2S0: u32 = 0x4001_9000;
    pub const SPI1: u32 = 0x4002_0000;
    pub const IDMA1: u32 = 0x4002_5000;
    pub const IDMA0: u32 = 0x4002_B000;
    pub const GPIO2: u32 = 0x4002_F000;
    pub const GPIO1: u32 = 0x4003_0000;
    pub const GPIO0: u32 = 0x4003_1000;
    pub const I2C1: u32 = 0x4003_3000;
    pub const UART1: u32 = 0x4003_4000;
    pub const UART0: u32 = 0x4003_8000;
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
            SAR_ADC => "SAR_ADC",
            I2S4 => "I2S4",
            I2S2 => "I2S2",
            I2S0 => "I2S0",
            SPI1 => "SPI1",
            IDMA1 => "IDMA1",
            IDMA0 => "IDMA0",
            GPIO2 => "GPIO2",
            GPIO1 => "GPIO1",
            GPIO0 => "GPIO0",
            I2C1 => "I2C1",
            UART1 => "UART1",
            UART0 => "UART0",
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
}

impl MmioTrace {
    fn record_read(&mut self, addr: u32) {
        if self.enabled {
            self.unknown.entry(addr).or_default().reads += 1;
        }
    }

    fn record_write(&mut self, addr: u32, val: u32) {
        if self.enabled {
            let e = self.unknown.entry(addr).or_default();
            e.writes += 1;
            e.last_write = val;
        }
    }

    fn record_any_read(&mut self, addr: u32) {
        if self.enabled {
            self.all.entry(addr).or_default().reads += 1;
        }
    }

    fn record_any_write(&mut self, addr: u32, val: u32) {
        if self.enabled {
            let e = self.all.entry(addr).or_default();
            e.writes += 1;
            e.last_write = val;
        }
    }

    fn record_off_map_read(&mut self, addr: u32) {
        if self.enabled {
            self.off_map.entry(addr & !3).or_default().reads += 1;
        }
    }

    fn record_off_map_write(&mut self, addr: u32, val: u32) {
        if self.enabled {
            let e = self.off_map.entry(addr & !3).or_default();
            e.writes += 1;
            e.last_write = val;
        }
    }

    pub fn clear(&mut self) {
        self.unknown.clear();
        self.all.clear();
        self.off_map.clear();
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
    pub flash: SpiFlash,
    pub pram: Pram,
    pub sram: InternalSram,
    pub boot_rom: BootRom,
    pub mmio_trace: MmioTrace,
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self {
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
        match addr {
            map::PRAM_BASE..=map::PRAM_END => self.pram.read_u8(addr as usize),
            map::ROM_BASE..=map::ROM_END => {
                self.boot_rom.read_u8((addr - map::ROM_BASE) as usize)
            }
            map::ICACHE_BASE..=map::ICACHE_END => {
                self.flash.read_u8((addr - map::ICACHE_BASE) as usize)
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
                self.mmio_trace.record_off_map_read(addr);
                0
            }
        }
    }

    pub fn write_u8(&mut self, addr: u32, val: u8, periph: &mut Peripherals, nvic: &mut Nvic) {
        match addr {
            map::PRAM_BASE..=map::PRAM_END => self.pram.write_u8(addr as usize, val),
            map::ICACHE_BASE..=map::ICACHE_END => {
                self.flash.write_u8((addr - map::ICACHE_BASE) as usize, val)
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
            _ => self.mmio_trace.record_off_map_write(addr, val as u32),
        }
    }

    pub fn read_u16(&mut self, addr: u32, periph: &mut Peripherals, nvic: &Nvic) -> u16 {
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

    fn read_mmio_u32(&mut self, addr: u32, p: &mut Peripherals) -> u32 {
        self.mmio_trace.record_any_read(addr);
        let page = addr & !0xFFF;
        let off = addr & 0xFFF;
        match page {
            periph::UART0 => p.uart.read_reg(off),
            periph::GPIO0 => p.gpio.read_reg(off),
            periph::SYSCTRL0 => p.sys.read_reg(off),
            // Seuls les FEUSE sont modelises dans la zone systeme, le reste de
            // la page doit rester visible dans la trace.
            periph::FUSES if (0x30..=0x3f).contains(&off) => p.fuses.read_reg(off),
            p_ if (periph::TIMERS..=periph::TIMERS_LAST).contains(&p_) => p.timers.read_reg(off),
            _ => {
                self.mmio_trace.record_read(addr);
                0
            }
        }
    }

    fn write_mmio_u32(&mut self, addr: u32, val: u32, p: &mut Peripherals) {
        self.mmio_trace.record_any_write(addr, val);
        let page = addr & !0xFFF;
        let off = addr & 0xFFF;
        match page {
            periph::UART0 => p.uart.write_reg(off, val),
            periph::GPIO0 => p.gpio.write_reg(off, val),
            periph::SYSCTRL0 => {
                if p.sys.write_reg(off, val) {
                    self.boot_rom.is_hidden = true;
                }
            }
            periph::FUSES if (0x30..=0x3f).contains(&off) => p.fuses.write_reg(off, val),
            p_ if (periph::TIMERS..=periph::TIMERS_LAST).contains(&p_) => {
                p.timers.write_reg(off, val)
            }
            _ => self.mmio_trace.record_write(addr, val),
        }
    }
}
