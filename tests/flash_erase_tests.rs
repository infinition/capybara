//! NOR flash behaviour of the SPI flash controller: erase commands bring a
//! range back to 0xFF and programming can only clear bits.

use capybara::emulator::cpu::Nvic;
use capybara::emulator::mmu::{map, periph, MemoryBus};
use capybara::emulator::peripherals::Peripherals;

const CMD: u32 = 0x004;
const ADDRH: u32 = 0x008;
const ADDRL: u32 = 0x00C;
const SECTOR: u32 = 1 << 10;
const BLOCK: u32 = 1 << 9;

fn erase(
    bus: &mut MemoryBus,
    p: &mut Peripherals,
    n: &mut Nvic,
    cmd: u32,
    offset: u32,
) {
    // SPIFC_Erase writes the address shifted right by one bit.
    let a = offset >> 1;
    bus.write_u32(periph::FLASH_CTL + ADDRH, a >> 16, p, n);
    bus.write_u32(periph::FLASH_CTL + ADDRL, a & 0xFFFF, p, n);
    bus.write_u32(periph::FLASH_CTL + CMD, cmd, p, n);
}

fn program(bus: &mut MemoryBus, p: &mut Peripherals, n: &mut Nvic, offset: u32, data: &[u8]) {
    for (i, b) in data.iter().enumerate() {
        bus.write_u8(map::SRAM_BASE + 0x200 + i as u32, *b, p, n);
    }
    let base = periph::FLASH_CTL;
    bus.write_u32(base + 0x10C, 0x6000_0000 | offset, p, n);
    bus.write_u32(base + 0x104, data.len() as u32, p, n);
    bus.write_u32(base + 0x100, map::SRAM_BASE + 0x200, p, n);
    // Direction bit clear: memory to flash.
    bus.write_u32(base + 0x108, 0, p, n);
    bus.write_u32(base + 0x108, 1, p, n);
}

#[test]
fn sector_erase_sets_the_range_to_ff_and_nothing_else() {
    let mut bus = MemoryBus::default();
    let mut p = Peripherals::default();
    let mut n = Nvic::default();
    let base = 0xD4_E000u32;
    for i in 0..0x3000u32 {
        bus.flash.write_u8((base + i) as usize, 0x5A);
    }
    erase(&mut bus, &mut p, &mut n, SECTOR, base + 0x1000);
    for i in 0..0x3000u32 {
        let v = bus.flash.read_u8((base + i) as usize);
        let in_sector = (0x1000..0x2000).contains(&i);
        assert_eq!(v, if in_sector { 0xFF } else { 0x5A }, "offset {:#x}", i);
    }
}

#[test]
fn block_erase_covers_sixty_four_kilobytes() {
    let mut bus = MemoryBus::default();
    let mut p = Peripherals::default();
    let mut n = Nvic::default();
    let base = 0xD0_0000u32;
    for i in [0u32, 0xFFFF, 0x1_0000] {
        bus.flash.write_u8((base + i) as usize, 0x00);
    }
    erase(&mut bus, &mut p, &mut n, BLOCK, base + 0x1234);
    assert_eq!(bus.flash.read_u8(base as usize), 0xFF);
    assert_eq!(bus.flash.read_u8((base + 0xFFFF) as usize), 0xFF);
    assert_eq!(bus.flash.read_u8((base + 0x1_0000) as usize), 0x00);
}

#[test]
fn programming_clears_bits_but_cannot_set_them() {
    let mut bus = MemoryBus::default();
    let mut p = Peripherals::default();
    let mut n = Nvic::default();
    let off = 0xD4_A000u32;
    erase(&mut bus, &mut p, &mut n, SECTOR, off);
    program(&mut bus, &mut p, &mut n, off, &[0xF0, 0x3C]);
    assert_eq!(bus.flash.read_u8(off as usize), 0xF0);
    assert_eq!(bus.flash.read_u8(off as usize + 1), 0x3C);
    // Programming again without an erase ANDs the new value in.
    program(&mut bus, &mut p, &mut n, off, &[0x0F, 0xFF]);
    assert_eq!(bus.flash.read_u8(off as usize), 0x00);
    assert_eq!(bus.flash.read_u8(off as usize + 1), 0x3C);
    // Erase then program gives exactly the programmed bytes.
    erase(&mut bus, &mut p, &mut n, SECTOR, off);
    program(&mut bus, &mut p, &mut n, off, &[0x0F, 0xFF]);
    assert_eq!(bus.flash.read_u8(off as usize), 0x0F);
    assert_eq!(bus.flash.read_u8(off as usize + 1), 0xFF);
}
