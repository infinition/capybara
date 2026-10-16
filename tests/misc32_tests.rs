//! Groupe 32 bits « divers a registre » (ARMv7-M A5.3.12) :
//! `1111 1010 1 op1 Rm | 1111 Rd 10 op2 Rm`.
//!
//! - op1 = 00 : QADD, QDADD, QSUB, QDSUB, extension DSP -> Undefined ici.
//! - op1 = 01 : REV.W (00), REV16.W (01), RBIT (10), REVSH.W (11).
//! - op1 = 11, op2 = 00 : CLZ.
//! - op1 = 10, op2 = 00 : SEL, extension DSP seulement (v7E-M) -> Undefined ici.
//! - tout le reste du groupe : Undefined.
//!
//! Le firmware execute `rev16.w sb, r0` en 0x1006a6ce (FUN_1006a640).

use capybara::emulator::cpu::registers::Registers;
use capybara::emulator::cpu::thumb16::{StepResult, Thumb16};
use capybara::emulator::cpu::thumb32::Thumb32;
use capybara::emulator::cpu::Nvic;
use capybara::emulator::mmu::MemoryBus;
use capybara::emulator::peripherals::Peripherals;

struct Ctx {
    regs: Registers,
    bus: MemoryBus,
    periph: Peripherals,
    nvic: Nvic,
}

impl Ctx {
    fn new() -> Self {
        Ctx {
            regs: Registers::default(),
            bus: MemoryBus::default(),
            periph: Peripherals::default(),
            nvic: Nvic::default(),
        }
    }
    fn exec(&mut self, w1: u16, w2: u16) -> StepResult {
        Thumb32::execute(w1, w2, &mut self.regs, &mut self.bus, &mut self.periph, &mut self.nvic)
    }
}

/// Encodage du groupe : op1 (2 bits), op2 (2 bits), Rd, Rm.
fn enc(op1: u16, op2: u16, rd: u16, rm: u16) -> (u16, u16) {
    (0xFA80 | (op1 << 4) | rm, 0xF080 | (rd << 8) | (op2 << 4) | rm)
}

fn reference(op2: u16, v: u32) -> u32 {
    match op2 {
        0 => {
            ((v & 0xFF) << 24) | ((v & 0xFF00) << 8) | ((v >> 8) & 0xFF00) | (v >> 24)
        }
        1 => {
            let b = v.to_le_bytes();
            u32::from_le_bytes([b[1], b[0], b[3], b[2]])
        }
        2 => {
            let mut r = 0u32;
            for i in 0..32 {
                if v & (1 << i) != 0 {
                    r |= 1 << (31 - i);
                }
            }
            r
        }
        _ => {
            // REVSH : octets 0 et 1 echanges, signe pris sur l'ancien octet 0.
            let lo = ((v & 0xFF) << 8) | ((v >> 8) & 0xFF);
            (lo as u16 as i16) as i32 as u32
        }
    }
}

const VALEURS: &[u32] = &[
    0x0000_0000, 0xFFFF_FFFF, 0x1234_5678, 0x8000_0001, 0x0000_0080, 0x0000_8000,
    0x0000_007F, 0xA5C3_E10F, 0x0000_00E3, 0xE381_8200,
];

#[test]
fn rev_rev16_rbit_revsh_32_bits() {
    for op2 in 0..4u16 {
        for &v in VALEURS {
            let mut c = Ctx::new();
            c.regs.set_reg(3, v);
            let (w1, w2) = enc(1, op2, 9, 3);
            assert!(matches!(c.exec(w1, w2), StepResult::Ok(_)), "{:04X} {:04X}", w1, w2);
            assert_eq!(c.regs.get_reg(9), reference(op2, v), "op2={} v={:08X}", op2, v);
            assert_eq!(c.regs.get_reg(3), v, "Rm intact");
        }
    }
}

#[test]
fn rev16_w_du_firmware() {
    // 0x1006a6ce : rev16.w sb, r0 = FA90 F990
    let mut c = Ctx::new();
    c.regs.set_reg(0, 0x8182_E3AA);
    assert!(matches!(c.exec(0xFA90, 0xF990), StepResult::Ok(_)));
    assert_eq!(c.regs.get_reg(9), 0x8281_AAE3);
}

#[test]
fn revsh_signe_et_rbit_valeurs_connues() {
    let mut c = Ctx::new();
    c.regs.set_reg(1, 0x1234_0080);
    let (w1, w2) = enc(1, 3, 2, 1);
    c.exec(w1, w2);
    assert_eq!(c.regs.get_reg(2), 0xFFFF_8000, "REVSH etend le signe");
    c.regs.set_reg(1, 0x0000_0001);
    let (w1, w2) = enc(1, 2, 2, 1);
    c.exec(w1, w2);
    assert_eq!(c.regs.get_reg(2), 0x8000_0000, "RBIT");
}

#[test]
fn les_formes_32_bits_concordent_avec_les_formes_16_bits() {
    // REV BA00, REV16 BA40, REVSH BAC0 (rm = r1, rd = r2)
    for (op2, w16) in [(0u16, 0xBA0Au16), (1, 0xBA4A), (3, 0xBACA)] {
        for &v in VALEURS {
            let mut a = Ctx::new();
            a.regs.set_reg(1, v);
            Thumb16::execute(w16, &mut a.regs, &mut a.bus, &mut a.periph, &mut a.nvic);
            let mut b = Ctx::new();
            b.regs.set_reg(1, v);
            let (w1, w2) = enc(1, op2, 2, 1);
            b.exec(w1, w2);
            assert_eq!(a.regs.get_reg(2), b.regs.get_reg(2), "op2={} v={:08X}", op2, v);
        }
    }
}

#[test]
fn les_drapeaux_ne_changent_pas() {
    for op2 in 0..4u16 {
        let mut c = Ctx::new();
        c.regs.xpsr = 0xF000_0000 | 0x0100_0000;
        c.regs.set_reg(4, 0);
        let (w1, w2) = enc(1, op2, 5, 4);
        c.exec(w1, w2);
        assert_eq!(c.regs.xpsr, 0xF100_0000);
    }
}

#[test]
fn clz_reste_correct() {
    let mut c = Ctx::new();
    for (v, n) in [(0u32, 32u32), (1, 31), (0x8000_0000, 0), (0x0001_0000, 15)] {
        c.regs.set_reg(6, v);
        let (w1, w2) = enc(3, 0, 7, 6);
        assert!(matches!(c.exec(w1, w2), StepResult::Ok(_)));
        assert_eq!(c.regs.get_reg(7), n);
    }
}

#[test]
fn sel_et_les_encodages_libres_du_groupe_sont_undefined() {
    // op1 = 00 : QADD, QDADD, QSUB, QDSUB (DSP).
    // SEL (op1 = 10, op2 = 00) : DSP seulement.
    // op1 = 10 et 11 avec op2 != 00 : non alloues.
    for (op1, op2) in [
        (0u16, 0u16), (0, 1), (0, 2), (0, 3),
        (2, 0), (2, 1), (2, 2), (2, 3), (3, 1), (3, 2), (3, 3),
    ] {
        let mut c = Ctx::new();
        c.regs.set_reg(8, 0x1234_5678);
        let (w1, w2) = enc(op1, op2, 8, 1);
        assert!(
            matches!(c.exec(w1, w2), StepResult::Undefined(_)),
            "op1={} op2={} doit etre Undefined",
            op1,
            op2
        );
        assert_eq!(c.regs.get_reg(8), 0x1234_5678, "rien d'ecrit");
    }
    // Addition parallele SADD8 (FA8n F00m, DSP) et REV avec w2[7:6] = 11.
    let mut c = Ctx::new();
    assert!(matches!(c.exec(0xFA81, 0xF002), StepResult::Undefined(_)));
    assert!(matches!(c.exec(0xFA91, 0xF2C1), StepResult::Undefined(_)));
}
