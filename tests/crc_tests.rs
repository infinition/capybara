//! The CRC unit follows the vendor SDK's CRC_Start / CRC_Get_CheckSum
//! sequence: any width, Init/XOR options, adder mode, CRC_OK read-back.

use capybara::emulator::Machine;

const BASE: u32 = 0x4003_8000;

fn run(m: &mut Machine, poly: u32, init: u32, xor: u32) -> u32 {
    let src = 0x1800_4000u32;
    for (i, b) in b"123456789".iter().enumerate() {
        m.bus.write_u8(src + i as u32, *b, &mut m.periph, &mut m.cpu.nvic);
    }
    let ordre = 32 - poly.leading_zeros() - 1;
    let w = |m: &mut Machine, off: u32, v: u32| {
        m.bus.write_u32(BASE + off, v, &mut m.periph, &mut m.cpu.nvic)
    };
    let r = |m: &mut Machine, off: u32| m.bus.read_u32(BASE + off, &mut m.periph, &m.cpu.nvic);
    let c = r(m, 0);
    w(m, 0, c & !0x10);
    let c = r(m, 0);
    w(m, 0, c & !1);
    let s = r(m, 0x14);
    w(m, 0x14, (s & !3) | init | (xor << 1));
    w(m, 0x18, poly);
    let s = r(m, 0x14);
    w(m, 0x14, (s & !0x1F0) | (ordre << 4));
    w(m, 0x04, src);
    w(m, 0x08, 9);
    w(m, 0x0C, 1);
    w(m, 0x10, 1);
    let c = r(m, 0);
    w(m, 0, c | 0x10);
    // CRC_GetStatus: EN = 1 and CRC_OK (bit 9) = 1, then EN is cleared.
    let c = r(m, 0);
    assert_eq!(c & 0x210, 0x210);
    w(m, 0, c & !0x10);
    r(m, 0x1C)
}

/// Check value of "123456789" for CRC-32 (type 5) and CRC-16/ARC (type 0, the
/// one the firmware uses), plus the all-ones Init variant (CRC-16/MODBUS).
#[test]
fn crc_follows_the_sdk_sequence() {
    let mut m = Machine::new();
    assert_eq!(run(&mut m, 0xEDB8_8320, 1, 1), 0xCBF4_3926);
    assert_eq!(run(&mut m, 0xA001, 0, 0), 0xBB3D);
    assert_eq!(run(&mut m, 0xA001, 1, 0), 0x4B37);
}
