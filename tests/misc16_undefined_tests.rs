//! Encodages non alloues : ils doivent remonter `StepResult::Undefined` au lieu
//! d'etre executes en silence, et les vraies indications doivent rester des NOP.
//!
//! Groupe divers 16 bits (ARMv7-M A5.2.5) : 1011 0110 hors CPS (SETEND
//! compris, absent de v7-M), 1011 0111, 1011 1000, 1011 1010 10xx.
//! Les encodages 32 bits non alloues des chargements simples sont testes dans
//! load_store32_tests.rs.

use capybara::emulator::cpu::registers::Registers;
use capybara::emulator::cpu::thumb16::{StepResult, Thumb16};
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
        let mut c = Ctx {
            regs: Registers::default(),
            bus: MemoryBus::default(),
            periph: Peripherals::default(),
            nvic: Nvic::default(),
        };
        for r in 0..13u8 {
            c.regs.set_reg(r, 0x300 + 0x10 * r as u32);
        }
        c.regs.msp = 0x800;
        c.regs.pc = 0x206;
        c
    }
    fn e16(&mut self, w: u16) -> StepResult {
        Thumb16::execute(w, &mut self.regs, &mut self.bus, &mut self.periph, &mut self.nvic)
    }
    fn etat(&self) -> Vec<u32> {
        let mut v: Vec<u32> = (0..16).map(|r| self.regs.get_reg(r)).collect();
        v.push(self.regs.xpsr);
        v.push(self.regs.primask);
        v
    }
}

#[test]
fn misc_16_bits_non_alloues_sont_undefined() {
    let mut cas: Vec<u16> = Vec::new();
    // 1011 0110 xxxx xxxx hors CPS (B660..B67F)
    for w in 0xB600u16..=0xB6FF {
        if (w & 0xFFE0) != 0xB660 {
            cas.push(w);
        }
    }
    cas.extend(0xB700u16..=0xB7FF);
    cas.extend(0xB800u16..=0xB8FF);
    cas.extend(0xBA80u16..=0xBABF);
    for w in cas {
        let mut c = Ctx::new();
        let avant = c.etat();
        assert!(
            matches!(c.e16(w), StepResult::Undefined(x) if x == w),
            "{:04X} doit etre Undefined",
            w
        );
        assert_eq!(c.etat(), avant, "{:04X} ne doit rien modifier", w);
    }
}

#[test]
fn indications_16_bits_restent_des_nop() {
    // NOP, YIELD, WFE, WFI, SEV et indications non allouees 1011 1111 xxxx 0000.
    for w in (0xBF00u16..=0xBFF0).step_by(0x10) {
        let mut c = Ctx::new();
        let avant = c.etat();
        assert!(matches!(c.e16(w), StepResult::Ok(_)), "{:04X}", w);
        assert_eq!(c.etat(), avant, "{:04X}", w);
        assert_eq!(c.regs.itstate, 0);
    }
}

#[test]
fn le_reste_du_groupe_divers_s_execute_toujours() {
    let mut c = Ctx::new();
    // CPSID i / CPSIE i
    assert!(matches!(c.e16(0xB672), StepResult::Ok(_)));
    assert_eq!(c.regs.primask, 1);
    assert!(matches!(c.e16(0xB662), StepResult::Ok(_)));
    assert_eq!(c.regs.primask, 0);
    // ADD SP, #8 ; SUB SP, #8
    assert!(matches!(c.e16(0xB002), StepResult::Ok(_)));
    assert_eq!(c.regs.get_sp(), 0x808);
    assert!(matches!(c.e16(0xB082), StepResult::Ok(_)));
    assert_eq!(c.regs.get_sp(), 0x800);
    // UXTB r0, r1 ; REV r2, r1 ; REV16 ; REVSH
    c.regs.set_reg(1, 0x1234_56F8);
    assert!(matches!(c.e16(0xB2C8), StepResult::Ok(_)));
    assert_eq!(c.regs.get_reg(0), 0xF8);
    assert!(matches!(c.e16(0xBA0A), StepResult::Ok(_)));
    assert_eq!(c.regs.get_reg(2), 0xF856_3412);
    assert!(matches!(c.e16(0xBA4A), StepResult::Ok(_)));
    assert_eq!(c.regs.get_reg(2), 0x3412_F856);
    assert!(matches!(c.e16(0xBACA), StepResult::Ok(_)));
    assert_eq!(c.regs.get_reg(2), 0xFFFF_F856);
    // CBZ r3 (r3 != 0 : pas de saut)
    assert!(matches!(c.e16(0xB10B), StepResult::Ok(_)));
    // PUSH {r0} / POP {r0}
    assert!(matches!(c.e16(0xB401), StepResult::Ok(_)));
    assert!(matches!(c.e16(0xBC01), StepResult::Ok(_)));
    assert_eq!(c.regs.get_sp(), 0x800);
    // IT EQ
    assert!(matches!(c.e16(0xBF08), StepResult::Ok(_)));
    assert_eq!(c.regs.itstate, 0x08);
    // BKPT reste un point d'arret
    assert!(matches!(c.e16(0xBE00), StepResult::Breakpoint));
}
