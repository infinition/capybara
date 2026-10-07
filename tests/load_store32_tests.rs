//! Chargements et rangements simples 32 bits : formes litterales, indications
//! memoire et encodages non alloues.
//!
//! - LDR/LDRB/LDRH/LDRSB/LDRSH (litteral), Rn = 1111 : w1[7] = U, immediat
//!   12 bits, base Align(PC, 4) (ARMv7-M A7.7.43 etc.). U = 0 retire l'immediat.
//! - LDRD (litteral) : base Align(PC, 4) (A7.7.50).
//! - PLD, PLI et indications : LDRB/LDRSB/LDRH/LDRSH avec Rt = 1111 ne
//!   chargent rien et ne touchent pas au PC (A5.3.9, A5.3.10).
//!
//! - Encodages non alloues du groupe (A5.3.7 a A5.3.10) : largeur 11,
//!   rangement signe, LDR signe d'un mot, rangement relatif au PC, forme T4
//!   avec P = 0 et W = 0, forme a registre avec w2[10:6] non nul -> Undefined.
//!
//! Appel direct de Thumb32::execute : `regs.pc` y vaut deja l'adresse de
//! l'instruction + 4 (avance par Cpu::step), c'est-a-dire le PC architectural.

use capybara::emulator::cpu::registers::Registers;
use capybara::emulator::cpu::thumb16::StepResult;
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
        let mut c = Ctx {
            regs: Registers::default(),
            bus: MemoryBus::default(),
            periph: Peripherals::default(),
            nvic: Nvic::default(),
        };
        // Motif distinct a chaque octet de 0x1E0 a 0x240.
        for a in (0x1E0u32..0x240).step_by(4) {
            let v = 0x80C0_0000 | (a << 4) | (a & 0xFF);
            c.bus.write_u32(a, v, &mut c.periph, &mut c.nvic);
        }
        c
    }
    fn exec_at(&mut self, addr: u32, w1: u16, w2: u16) -> StepResult {
        self.regs.pc = addr + 4;
        Thumb32::execute(w1, w2, &mut self.regs, &mut self.bus, &mut self.periph, &mut self.nvic)
    }
    fn etat(&self) -> Vec<u32> {
        let mut v: Vec<u32> = (0..16).map(|r| self.regs.get_reg(r)).collect();
        v.push(self.regs.xpsr);
        v
    }
    fn r32(&mut self, a: u32) -> u32 {
        self.bus.read_u32(a, &mut self.periph, &self.nvic)
    }
    fn r16(&mut self, a: u32) -> u16 {
        self.bus.read_u16(a, &mut self.periph, &self.nvic)
    }
    fn r8(&mut self, a: u32) -> u8 {
        self.bus.read_u8(a, &mut self.periph, &self.nvic)
    }
}

/// Instruction a une adresse non alignee sur 4 : PC = 0x206, Align = 0x204.
const A: u32 = 0x202;
const ALIGNED: u32 = 0x204;

#[test]
fn ldr_w_litteral_positif_et_negatif() {
    for (u, imm) in [(1u16, 8u32), (0, 8), (0, 0x24), (1, 0)] {
        let mut c = Ctx::new();
        let w1 = 0xF85F | (u << 7);
        let w2 = 0x1000 | imm as u16; // Rt = r1
        assert!(matches!(c.exec_at(A, w1, w2), StepResult::Ok(_)));
        let addr = if u == 1 { ALIGNED + imm } else { ALIGNED - imm };
        let attendu = c.r32(addr);
        assert_eq!(c.regs.get_reg(1), attendu, "U={} imm={:#x}", u, imm);
    }
}

#[test]
fn ldr_w_litteral_negatif_ne_lit_pas_un_registre_d_offset() {
    // ldr.w r1, [pc, #-8] = F85F 1008. Avant la correction, w1[7] = 0 envoyait
    // vers la forme a registre (offset r8) : on pose r8 pour que l'erreur se voie.
    let mut c = Ctx::new();
    c.regs.set_reg(8, 0x10);
    c.exec_at(A, 0xF85F, 0x1008);
    let attendu = c.r32(ALIGNED - 8);
    assert_eq!(c.regs.get_reg(1), attendu);
    assert_eq!(c.regs.get_reg(8), 0x10);
    // Et avec w2[11] = 1 (lisible comme une forme T4 avec mise a jour) :
    // ldr.w r1, [pc, #-0x900] -> Rn (pc) ne doit pas etre reecrit.
    let mut c = Ctx::new();
    c.bus.write_u32(0x1904 - 0x900, 0x5A5A_1234, &mut c.periph, &mut c.nvic);
    c.exec_at(0x1902, 0xF85F, 0x1900);
    assert_eq!(c.regs.get_reg(1), 0x5A5A_1234);
    assert_eq!(c.regs.pc, 0x1906, "le PC n'est pas une base reecrite");
}

#[test]
fn ldrb_ldrh_ldrsb_ldrsh_litteraux() {
    // (w1 avec U = 0, largeur, signe)
    for (w1, taille, signe) in [
        (0xF81Fu16, 1u32, false),
        (0xF83F, 2, false),
        (0xF91F, 1, true),
        (0xF93F, 2, true),
    ] {
        for (u, imm) in [(0u16, 5u32), (1, 7), (0, 0x22), (1, 0x30)] {
            let mut c = Ctx::new();
            let w1 = w1 | (u << 7);
            let w2 = 0x3000 | imm as u16; // Rt = r3
            assert!(matches!(c.exec_at(A, w1, w2), StepResult::Ok(_)));
            let addr = if u == 1 { ALIGNED + imm } else { ALIGNED - imm };
            let attendu = match (taille, signe) {
                (1, false) => c.r8(addr) as u32,
                (1, true) => c.r8(addr) as i8 as i32 as u32,
                (2, false) => c.r16(addr) as u32,
                _ => c.r16(addr) as i16 as i32 as u32,
            };
            assert_eq!(c.regs.get_reg(3), attendu, "w1={:04X} imm={:#x}", w1, imm);
        }
    }
}

#[test]
fn ldrd_litteral_part_du_pc_aligne() {
    // LDRD r2, r3, [pc, #8] = E9DF 2302 ; PC = 0x206, base = 0x204.
    let mut c = Ctx::new();
    assert!(matches!(c.exec_at(A, 0xE9DF, 0x2302), StepResult::Ok(_)));
    let (a, b) = (c.r32(ALIGNED + 8), c.r32(ALIGNED + 12));
    assert_eq!(c.regs.get_reg(2), a);
    assert_eq!(c.regs.get_reg(3), b);
    // LDRD r2, r3, [pc, #-8] = E95F 2302
    let mut c = Ctx::new();
    c.exec_at(A, 0xE95F, 0x2302);
    let (a, b) = (c.r32(ALIGNED - 8), c.r32(ALIGNED - 4));
    assert_eq!(c.regs.get_reg(2), a);
    assert_eq!(c.regs.get_reg(3), b);
    // Adresse deja alignee : meme resultat qu'avant.
    let mut c = Ctx::new();
    c.exec_at(0x200, 0xE9DF, 0x2302);
    let a = c.r32(0x204 + 8);
    assert_eq!(c.regs.get_reg(2), a);
}

#[test]
fn pld_pli_et_indications_ne_chargent_rien() {
    let cas: &[(&str, u16, u16)] = &[
        ("PLD [r0, #4]", 0xF890, 0xF004),
        ("PLD [r0, #-4]", 0xF810, 0xFC04),
        ("PLD [r0, r1]", 0xF810, 0xF001),
        ("PLD [pc, #0x10]", 0xF89F, 0xF010),
        ("PLD [pc, #-0x10]", 0xF81F, 0xF010),
        ("PLI [r0, #4]", 0xF990, 0xF004),
        ("PLI [r0, #-4]", 0xF910, 0xFC04),
        ("PLI [pc, #-2]", 0xF91F, 0xF002),
        ("LDRH Rt=pc (indication)", 0xF8B0, 0xF004),
        ("LDRSH Rt=pc (indication)", 0xF9B0, 0xF004),
        ("LDRH Rt=pc litteral", 0xF8BF, 0xF004),
    ];
    for &(nom, w1, w2) in cas {
        let mut c = Ctx::new();
        c.regs.set_reg(0, 0x210);
        c.regs.set_reg(1, 4);
        let avant: Vec<u32> = (0..15).map(|r| c.regs.get_reg(r)).collect();
        assert!(matches!(c.exec_at(A, w1, w2), StepResult::Ok(_)), "{}", nom);
        assert_eq!(c.regs.pc, A + 4, "{} : le PC ne doit pas sauter", nom);
        let apres: Vec<u32> = (0..15).map(|r| c.regs.get_reg(r)).collect();
        assert_eq!(avant, apres, "{} : aucun registre modifie", nom);
    }
}

#[test]
fn ldr_mot_vers_le_pc_reste_un_branchement() {
    // ldr pc, [sp], #0x14 (PRAM 0x3ee) = F85D FB14
    let mut c = Ctx::new();
    c.regs.msp = 0x220;
    c.bus.write_u32(0x220, 0x0000_1235, &mut c.periph, &mut c.nvic);
    assert!(matches!(c.exec_at(A, 0xF85D, 0xFB14), StepResult::Ok(_)));
    assert_eq!(c.regs.pc, 0x1234);
    assert_eq!(c.regs.get_sp(), 0x234);
}

#[test]
fn ldrb_ordinaire_inchange() {
    // LDRB.W r6, [r0, #3] = F890 6003
    let mut c = Ctx::new();
    c.regs.set_reg(0, 0x210);
    c.exec_at(A, 0xF890, 0x6003);
    let attendu = c.r8(0x213) as u32;
    assert_eq!(c.regs.get_reg(6), attendu);
}

#[test]
fn chargements_rangements_32_bits_non_alloues_sont_undefined() {
    let cas: &[(&str, u16, u16)] = &[
        ("largeur 11, rangement", 0xF860, 0x1000),
        ("largeur 11, chargement", 0xF870, 0x1000),
        ("largeur 11, imm12", 0xF8F0, 0x1004),
        ("STRB signe", 0xF900, 0x1000),
        ("STRB signe imm12", 0xF980, 0x1004),
        ("STRH signe", 0xF9A0, 0x1004),
        ("LDR signe (mot)", 0xF950, 0x1000),
        ("LDR signe (mot) imm12", 0xF9D0, 0x1004),
        ("STR.W [pc, #4]", 0xF8CF, 0x1004),
        ("STR.W [pc, #-4]", 0xF84F, 0x1004),
        ("STRB.W [pc]", 0xF88F, 0x1004),
        ("LDR T4 P=0 W=0", 0xF850, 0x1804),
        ("LDR T4 P=0 W=0 U=1", 0xF850, 0x1A04),
        ("STRB T4 P=0 W=0", 0xF800, 0x1804),
        ("LDR registre w2[6]=1", 0xF850, 0x1040),
        ("LDR registre w2[10]=1", 0xF850, 0x1402),
        ("STRH registre w2[8]=1", 0xF820, 0x1102),
    ];
    for &(nom, w1, w2) in cas {
        let mut c = Ctx::new();
        c.regs.set_reg(0, 0x210);
        c.regs.pc = A + 4;
        let avant = c.etat();
        let r = c.exec_at(A, w1, w2);
        assert!(matches!(r, StepResult::Undefined(_)), "{} ({:04X} {:04X}) doit etre Undefined", nom, w1, w2);
        assert_eq!(c.etat(), avant, "{} ne doit rien modifier", nom);
    }
}

#[test]
fn chargements_rangements_32_bits_valides_s_executent() {
    let cas: &[(&str, u16, u16)] = &[
        ("LDR.W r1, [r0, r2, lsl #2]", 0xF850, 0x1022),
        ("LDR.W r1, [r0, #4]", 0xF8D0, 0x1004),
        ("LDR r1, [r0, #-4]", 0xF850, 0x1C04),
        ("LDR r1, [r0], #4", 0xF850, 0x1B04),
        ("LDR r1, [r0, #4]!", 0xF850, 0x1F04),
        ("LDRT r1, [r0, #4]", 0xF850, 0x1E04),
        ("STRB r1, [r0, #-1]", 0xF800, 0x1C01),
        ("STRH.W r1, [r0, #2]", 0xF8A0, 0x1002),
        ("LDRSB.W r1, [r0, #1]", 0xF990, 0x1001),
        ("LDRSH r1, [r0, r2]", 0xF930, 0x1002),
        ("LDR.W r1, [pc, #4]", 0xF8DF, 0x1004),
    ];
    for &(nom, w1, w2) in cas {
        let mut c = Ctx::new();
        c.regs.set_reg(0, 0x210);
        c.regs.set_reg(2, 4);
        assert!(matches!(c.exec_at(A, w1, w2), StepResult::Ok(_)), "{} ({:04X} {:04X})", nom, w1, w2);
    }
}
