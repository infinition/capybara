//! Acces multiples 16 bits : LDM, STM, PUSH et POP (ARMv7-M A7.7.40, A7.7.159,
//! A7.7.99, A7.7.101).
//!
//! - LDM T1 `LDM Rn{!}, {liste}` : la base n'est reecrite que si Rn n'est PAS
//!   dans la liste (`wback = registers<n> == '0'`). Sinon Rn garde la valeur
//!   lue. Le firmware s'en sert en PRAM (0x1ce2, 0x1cf0, 0xbe6a, 0xbe94 :
//!   `ldm r0, {r0, ...}` sur le chemin printf flottant).
//! - STM T1 ecrit toujours la base ; Rn dans la liste n'est defini que s'il est
//!   le plus bas, et il est alors range avec sa valeur d'origine.
//! - PUSH : le registre le plus bas a l'adresse la plus basse, LR en haut.
//! - POP : meme ordre, PC en dernier, bit 0 retire de l'adresse.

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
        Ctx {
            regs: Registers::default(),
            bus: MemoryBus::default(),
            periph: Peripherals::default(),
            nvic: Nvic::default(),
        }
    }
    fn exec(&mut self, w: u16) {
        let r = Thumb16::execute(w, &mut self.regs, &mut self.bus, &mut self.periph, &mut self.nvic);
        assert!(matches!(r, StepResult::Ok(_)), "{:04X} doit s'executer", w);
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.bus.write_u32(a, v, &mut self.periph, &mut self.nvic);
    }
    fn r32(&mut self, a: u32) -> u32 {
        self.bus.read_u32(a, &mut self.periph, &self.nvic)
    }
}

const T: u32 = 0x400;

fn table(c: &mut Ctx) {
    c.w32(T, 0x1111_1111);
    c.w32(T + 4, 0x2222_2222);
    c.w32(T + 8, 0x3333_3333);
}

#[test]
fn ldm_sans_rn_dans_la_liste_ecrit_la_base() {
    let mut c = Ctx::new();
    table(&mut c);
    c.regs.set_reg(0, T);
    // LDM r0!, {r1, r2}
    c.exec(0xC806);
    assert_eq!(c.regs.get_reg(1), 0x1111_1111);
    assert_eq!(c.regs.get_reg(2), 0x2222_2222);
    assert_eq!(c.regs.get_reg(0), T + 8, "la base avance de 4 x 2");
}

#[test]
fn ldm_avec_rn_dans_la_liste_garde_la_valeur_lue() {
    let mut c = Ctx::new();
    table(&mut c);
    // LDM r0, {r0, r2, r3} : forme du firmware (PRAM 0x1ce2)
    c.regs.set_reg(0, T);
    c.exec(0xC80D);
    assert_eq!(c.regs.get_reg(0), 0x1111_1111, "r0 = mot lu, pas l'adresse de fin");
    assert_eq!(c.regs.get_reg(2), 0x2222_2222);
    assert_eq!(c.regs.get_reg(3), 0x3333_3333);

    // Rn au milieu de la liste : LDM r1, {r0, r1, r2}
    let mut c = Ctx::new();
    table(&mut c);
    c.regs.set_reg(1, T);
    c.exec(0xC907);
    assert_eq!(c.regs.get_reg(0), 0x1111_1111);
    assert_eq!(c.regs.get_reg(1), 0x2222_2222, "r1 = mot lu");
    assert_eq!(c.regs.get_reg(2), 0x3333_3333);

    // Rn en dernier : LDM r2, {r0, r1, r2}
    let mut c = Ctx::new();
    table(&mut c);
    c.regs.set_reg(2, T);
    c.exec(0xCA07);
    assert_eq!(c.regs.get_reg(2), 0x3333_3333, "r2 = mot lu");
}

#[test]
fn ldm_un_seul_registre_qui_est_la_base() {
    let mut c = Ctx::new();
    table(&mut c);
    c.regs.set_reg(4, T + 4);
    // LDM r4, {r4}
    c.exec(0xCC10);
    assert_eq!(c.regs.get_reg(4), 0x2222_2222);
}

#[test]
fn stm_ecrit_toujours_la_base_et_range_dans_l_ordre() {
    let mut c = Ctx::new();
    c.regs.set_reg(0, T);
    c.regs.set_reg(1, 0xAAAA_0001);
    c.regs.set_reg(2, 0xAAAA_0002);
    c.regs.set_reg(5, 0xAAAA_0005);
    // STM r0!, {r1, r2, r5}
    c.exec(0xC026);
    assert_eq!(c.r32(T), 0xAAAA_0001);
    assert_eq!(c.r32(T + 4), 0xAAAA_0002);
    assert_eq!(c.r32(T + 8), 0xAAAA_0005);
    assert_eq!(c.regs.get_reg(0), T + 12);
}

#[test]
fn stm_rn_le_plus_bas_de_la_liste_range_sa_valeur_d_origine() {
    let mut c = Ctx::new();
    c.regs.set_reg(0, T);
    c.regs.set_reg(1, 0xBBBB_0001);
    // STM r0!, {r0, r1}
    c.exec(0xC003);
    assert_eq!(c.r32(T), T, "r0 range avant la mise a jour de la base");
    assert_eq!(c.r32(T + 4), 0xBBBB_0001);
    assert_eq!(c.regs.get_reg(0), T + 8, "STM ecrit la base meme si Rn est dans la liste");
}

#[test]
fn push_et_pop_16_bits_ordre_et_pointeur_de_pile() {
    let mut c = Ctx::new();
    c.regs.msp = 0x1000;
    c.regs.set_reg(0, 0x0000_00A0);
    c.regs.set_reg(4, 0x0000_00A4);
    c.regs.lr = 0x0000_1235;
    // PUSH {r0, r4, lr}
    c.exec(0xB511);
    assert_eq!(c.regs.get_sp(), 0x1000 - 12);
    assert_eq!(c.r32(0x0FF4), 0xA0, "registre le plus bas a l'adresse la plus basse");
    assert_eq!(c.r32(0x0FF8), 0xA4);
    assert_eq!(c.r32(0x0FFC), 0x1235, "LR en haut");

    c.regs.set_reg(0, 0);
    c.regs.set_reg(4, 0);
    c.regs.pc = 0x2000;
    // POP {r0, r4, pc}
    c.exec(0xBD11);
    assert_eq!(c.regs.get_reg(0), 0xA0);
    assert_eq!(c.regs.get_reg(4), 0xA4);
    assert_eq!(c.regs.pc, 0x1234, "bit Thumb retire de l'adresse");
    assert_eq!(c.regs.get_sp(), 0x1000);
}

#[test]
fn pop_sans_pc_ne_touche_pas_au_pc() {
    let mut c = Ctx::new();
    c.regs.msp = 0x0FF8;
    c.w32(0x0FF8, 7);
    c.w32(0x0FFC, 9);
    c.regs.pc = 0x2002;
    // POP {r1, r7}
    c.exec(0xBC82);
    assert_eq!(c.regs.get_reg(1), 7);
    assert_eq!(c.regs.get_reg(7), 9);
    assert_eq!(c.regs.pc, 0x2002);
    assert_eq!(c.regs.get_sp(), 0x1000);
}

#[test]
fn push_lr_seul() {
    let mut c = Ctx::new();
    c.regs.msp = 0x1000;
    c.regs.lr = 0xCAFE_0001;
    // PUSH {lr}
    c.exec(0xB500);
    assert_eq!(c.regs.get_sp(), 0x0FFC);
    assert_eq!(c.r32(0x0FFC), 0xCAFE_0001);
}
