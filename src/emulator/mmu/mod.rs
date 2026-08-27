pub mod flash;
pub mod rom;
pub mod sram;

pub use flash::SpiFlash;
pub use rom::BootRom;
pub use sram::InternalSram;

use crate::emulator::cpu::nvic::Nvic;
use crate::emulator::peripherals::Peripherals;

pub struct MemoryBus {
    pub flash: SpiFlash,
    pub sram: InternalSram,
    pub boot_rom: BootRom,
}

impl Default for MemoryBus {
    fn default() -> Self {
        Self {
            flash: SpiFlash::default(),
            sram: InternalSram::default(),
            boot_rom: BootRom::default(),
        }
    }
}

impl MemoryBus {
    pub fn read_u8(&mut self, addr: u32, periph: &mut Peripherals, nvic: &Nvic) -> u8 {
        match addr {
            // Boot ROM or Vector Table alias
            0x0000_0000..=0x0000_FFFF => {
                self.boot_rom.read_u8(addr as usize)
            }
            // Boot ROM explicit region
            0x0800_0000..=0x0800_FFFF => {
                let offset = (addr - 0x0800_0000) as usize;
                self.boot_rom.read_u8(offset)
            }
            // Flash SPI XIP mapped (16 MB)
            0x1000_0000..=0x10FF_FFFF => {
                let offset = (addr - 0x1000_0000) as usize;
                self.flash.read_u8(offset)
            }
            // Flash SPI XIP Sonix mapped (0x60000000, 16 MB)
            0x6000_0000..=0x60FF_FFFF => {
                let offset = (addr - 0x6000_0000) as usize;
                self.flash.read_u8(offset)
            }
            // Flash SPI Alias mapped (0x18000000, 16 MB)
            0x1800_0000..=0x18FF_FFFF => {
                let offset = (addr - 0x1800_0000) as usize;
                self.flash.read_u8(offset)
            }
            // SRAM / PRAM (128 KB)
            0x2000_0000..=0x2001_FFFF => {
                let offset = (addr - 0x2000_0000) as usize;
                self.sram.read_u8(offset)
            }
            // Mailbox RAM (16 KB)
            0x2002_0000..=0x2002_3FFF => {
                let offset = (addr - 0x2002_0000) as usize;
                self.sram.read_mailbox_u8(offset)
            }
            // MMIO Peripherals (32-bit aligned reads routed to byte)
            0x4000_0000..=0x4FFF_FFFF => {
                let aligned_addr = addr & !3;
                let val = self.read_mmio_u32(aligned_addr, periph);
                let shift = (addr & 3) * 8;
                ((val >> shift) & 0xFF) as u8
            }
            // NVIC / Cortex-M System Control Space
            0xE000_E000..=0xE000_EFFF => {
                let aligned_addr = addr & !3;
                let val = nvic.read_reg(aligned_addr);
                let shift = (addr & 3) * 8;
                ((val >> shift) & 0xFF) as u8
            }
            _ => 0,
        }
    }

    pub fn write_u8(&mut self, addr: u32, val: u8, periph: &mut Peripherals, nvic: &mut Nvic) {
        match addr {
            0x1000_0000..=0x10FF_FFFF => {
                let offset = (addr - 0x1000_0000) as usize;
                self.flash.write_u8(offset, val);
            }
            0x2000_0000..=0x2001_FFFF => {
                let offset = (addr - 0x2000_0000) as usize;
                self.sram.write_u8(offset, val);
            }
            0x2002_0000..=0x2002_3FFF => {
                let offset = (addr - 0x2002_0000) as usize;
                self.sram.write_mailbox_u8(offset, val);
            }
            0x4000_0000..=0x4FFF_FFFF => {
                let aligned_addr = addr & !3;
                let mut current = self.read_mmio_u32(aligned_addr, periph);
                let shift = (addr & 3) * 8;
                current &= !(0xFF << shift);
                current |= (val as u32) << shift;
                self.write_mmio_u32(aligned_addr, current, periph);
            }
            0xE000_E000..=0xE000_EFFF => {
                let aligned_addr = addr & !3;
                let mut current = nvic.read_reg(aligned_addr);
                let shift = (addr & 3) * 8;
                current &= !(0xFF << shift);
                current |= (val as u32) << shift;
                nvic.write_reg(aligned_addr, current);
            }
            _ => {}
        }
    }

    pub fn read_u16(&mut self, addr: u32, periph: &mut Peripherals, nvic: &Nvic) -> u16 {
        let b0 = self.read_u8(addr, periph, nvic) as u16;
        let b1 = self.read_u8(addr + 1, periph, nvic) as u16;
        b0 | (b1 << 8)
    }

    pub fn write_u16(&mut self, addr: u32, val: u16, periph: &mut Peripherals, nvic: &mut Nvic) {
        self.write_u8(addr, (val & 0xFF) as u8, periph, nvic);
        self.write_u8(addr + 1, ((val >> 8) & 0xFF) as u8, periph, nvic);
    }

    pub fn read_u32(&mut self, addr: u32, periph: &mut Peripherals, nvic: &Nvic) -> u32 {
        let b0 = self.read_u8(addr, periph, nvic) as u32;
        let b1 = self.read_u8(addr + 1, periph, nvic) as u32;
        let b2 = self.read_u8(addr + 2, periph, nvic) as u32;
        let b3 = self.read_u8(addr + 3, periph, nvic) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    pub fn write_u32(&mut self, addr: u32, val: u32, periph: &mut Peripherals, nvic: &mut Nvic) {
        match addr {
            0x4000_0000..=0x4FFF_FFFF => {
                self.write_mmio_u32(addr, val, periph);
            }
            0xE000_E000..=0xE000_EFFF => {
                nvic.write_reg(addr, val);
            }
            _ => {
                self.write_u8(addr, (val & 0xFF) as u8, periph, nvic);
                self.write_u8(addr + 1, ((val >> 8) & 0xFF) as u8, periph, nvic);
                self.write_u8(addr + 2, ((val >> 16) & 0xFF) as u8, periph, nvic);
                self.write_u8(addr + 3, ((val >> 24) & 0xFF) as u8, periph, nvic);
            }
        }
    }

    fn read_mmio_u32(&mut self, addr: u32, periph: &mut Peripherals) -> u32 {
        match addr {
            0x4000_0000..=0x4000_00FF => periph.timers.read_reg(addr - 0x4000_0000),
            0x4100_0000..=0x4100_00FF => periph.uart.read_reg(addr - 0x4100_0000),
            0x4300_0000..=0x4300_00FF => periph.display.read_reg(addr - 0x4300_0000),
            0x4400_0000..=0x4400_00FF => periph.gpio.read_reg(addr - 0x4400_0000),
            0x4500_0000..=0x4500_00FF => periph.sys.read_reg(addr - 0x4500_0000),
            _ => 0,
        }
    }

    fn write_mmio_u32(&mut self, addr: u32, val: u32, periph: &mut Peripherals) {
        match addr {
            0x4000_0000..=0x4000_00FF => periph.timers.write_reg(addr - 0x4000_0000, val),
            0x4100_0000..=0x4100_00FF => periph.uart.write_reg(addr - 0x4100_0000, val),
            0x4300_0000..=0x4300_00FF => periph.display.write_reg(addr - 0x4300_0000, val),
            0x4400_0000..=0x4400_00FF => periph.gpio.write_reg(addr - 0x4400_0000, val),
            0x4500_0000..=0x4500_00FF => {
                let hide_rom = periph.sys.write_reg(addr - 0x4500_0000, val);
                if hide_rom {
                    self.boot_rom.is_hidden = true;
                }
            }
            _ => {}
        }
    }
}
