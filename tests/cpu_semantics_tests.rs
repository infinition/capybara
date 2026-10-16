//! Semantique ARMv7-M par classe d'encodage.
//!
//! Les valeurs attendues sont calculees a la main a partir du pseudo-code de
//! l'ARMv7-M ARM (AddWithCarry, Shift_C, ThumbExpandImm_C, PushStack,
//! PopStack...), jamais en relisant le code de l'emulateur. Les encodages ont
//! ete verifies par desassemblage.

use capybara::emulator::Machine;

const N: u32 = 0x8000_0000;
const Z: u32 = 0x4000_0000;
const C: u32 = 0x2000_0000;
const V: u32 = 0x1000_0000;
const NZCV: u32 = N | Z | C | V;
const BASE: u32 = 0x100;
const RAM: u32 = 0x1801_0000;

fn machine(prog: &[u16], flags: u32) -> Machine {
    let mut m = Machine::new();
    for (i, w) in prog.iter().enumerate() {
        let a = BASE as usize + 2 * i;
        m.bus.pram.write_u8(a, (*w & 0xFF) as u8);
        m.bus.pram.write_u8(a + 1, (*w >> 8) as u8);
    }
    m.cpu.regs.pc = BASE;
    m.cpu.regs.primask = 1;
    m.cpu.regs.msp = RAM + 0x800;
    m.cpu.regs.xpsr = (m.cpu.regs.xpsr & !NZCV) | flags;
    m
}

fn run(m: &mut Machine, n: usize) {
    for _ in 0..n {
        m.step();
    }
}

fn flags(m: &Machine) -> u32 {
    m.cpu.regs.xpsr & NZCV
}

fn wr32(m: &mut Machine, a: u32, v: u32) {
    let i = (a - 0x1800_0000) as usize;
    m.bus.sram.data[i..i + 4].copy_from_slice(&v.to_le_bytes());
}

fn rd32(m: &Machine, a: u32) -> u32 {
    let i = (a - 0x1800_0000) as usize;
    u32::from_le_bytes(m.bus.sram.data[i..i + 4].try_into().unwrap())
}

// ------------------------------------------------------------------ ALU 32 bits

#[test]
fn adc_sbc_rsb_32_bits_retenue_et_debordement() {
    // (nom, w1, w2, r1, r2, NZCV depart, r0 attendu, NZCV attendus)
    let cas: &[(&str, u16, u16, u32, u32, u32, u32, u32)] = &[
        // ADCS.W r0,r1,r2 : 0x7FFFFFFF + 0 + C(1) = 0x80000000 -> N,V
        ("ADCS.W", 0xEB51, 0x0002, 0x7FFF_FFFF, 0, C, 0x8000_0000, N | V),
        // ADCS.W : 0xFFFFFFFF + 1 + 0 = 0 -> Z,C
        ("ADCS.W", 0xEB51, 0x0002, 0xFFFF_FFFF, 1, 0, 0, Z | C),
        // SBCS.W r0,r1,r2 : 5 - 3 - (1-C=1) = 1, pas d'emprunt -> C
        ("SBCS.W", 0xEB71, 0x0002, 5, 3, 0, 1, C),
        // SBCS.W : 0x80000000 - 1 - 0 = 0x7FFFFFFF -> C,V
        ("SBCS.W", 0xEB71, 0x0002, 0x8000_0000, 1, C, 0x7FFF_FFFF, C | V),
        // RSBS.W r0,r1,#0 : 0 - 0x80000000 = 0x80000000 -> N,V (C = 0 : emprunt)
        ("RSBS.W #0", 0xF1D1, 0x0000, 0x8000_0000, 0, 0, 0x8000_0000, N | V),
        // RSBS.W r0,r1,#0 sur 0 : 0 -> Z,C
        ("RSBS.W #0", 0xF1D1, 0x0000, 0, 0, 0, 0, Z | C),
    ];
    for &(nom, w1, w2, r1, r2, f0, r0, f1) in cas {
        let mut m = machine(&[w1, w2], f0);
        m.cpu.regs.set_reg(1, r1);
        m.cpu.regs.set_reg(2, r2);
        run(&mut m, 1);
        assert_eq!(m.cpu.regs.get_reg(0), r0, "{nom} resultat");
        assert_eq!(flags(&m), f1, "{nom} drapeaux");
    }
}

#[test]
fn adc_sans_s_ne_touche_pas_les_drapeaux() {
    // ADC.W r1,r1,r1 (S = 0) : 0x7FFFFFFF*2 + 1 = 0xFFFFFFFF, NZCV intacts.
    let mut m = machine(&[0xEB41, 0x0101], C);
    m.cpu.regs.set_reg(1, 0x7FFF_FFFF);
    run(&mut m, 1);
    assert_eq!(m.cpu.regs.get_reg(1), 0xFFFF_FFFF);
    assert_eq!(flags(&m), C);
}

#[test]
fn decalages_par_registre_32_bits_quantites_limites() {
    // (w1 de LSLS/LSRS/ASRS/RORS.W r0,r1,r2 ; quantite ; r1 ; r0 ; C attendu)
    let v = 0x8000_0001u32;
    let cas: &[(&str, u16, u32, u32, u32, bool)] = &[
        ("LSLS #0", 0xFA11, 0, v, v, true), // retenue entree conservee (C=1)
        ("LSLS #1", 0xFA11, 1, v, 2, true),
        ("LSLS #32", 0xFA11, 32, v, 0, true), // C = bit 0
        ("LSLS #33", 0xFA11, 33, v, 0, false),
        ("LSLS #256", 0xFA11, 256, v, v, true), // seuls 8 bits : 256 -> 0
        ("LSRS #32", 0xFA31, 32, v, 0, true), // C = bit 31
        ("LSRS #33", 0xFA31, 33, v, 0, false),
        ("ASRS #32", 0xFA51, 32, v, 0xFFFF_FFFF, true),
        ("ASRS #200", 0xFA51, 200, 0x7FFF_FFFF, 0, false),
        ("RORS #32", 0xFA71, 32, v, v, true), // C = bit 31 du resultat
        ("RORS #1", 0xFA71, 1, v, 0xC000_0000, true),
    ];
    for &(nom, w1, s, r1, r0, c) in cas {
        let mut m = machine(&[w1, 0xF002], C);
        m.cpu.regs.set_reg(1, r1);
        m.cpu.regs.set_reg(2, s);
        run(&mut m, 1);
        assert_eq!(m.cpu.regs.get_reg(0), r0, "{nom}");
        assert_eq!(m.cpu.regs.flag_c(), c, "{nom} C");
    }
}

#[test]
fn decalages_immediats_32_bits_et_rrx() {
    // LSRS.W r0,r1,#32 ; ASRS.W r0,r1,#32 ; RRXS r0,r1
    let cas: &[(&str, u16, u32, u32, u32, u32)] = &[
        ("LSRS #32", 0x0011, 0x8000_0000, 0, 0, Z | C),
        ("ASRS #32", 0x0021, 0x8000_0000, 0, 0xFFFF_FFFF, N | C),
        ("RRXS C=1", 0x0031, 0x0000_0003, C, 0x8000_0001, N | C),
        ("RRXS C=0", 0x0031, 0x0000_0002, 0, 0x0000_0001, 0),
    ];
    for &(nom, w2, r1, f0, r0, f1) in cas {
        let mut m = machine(&[0xEA5F, w2], f0);
        m.cpu.regs.set_reg(1, r1);
        run(&mut m, 1);
        assert_eq!(m.cpu.regs.get_reg(0), r0, "{nom}");
        assert_eq!(flags(&m), f1, "{nom} drapeaux");
    }
}

#[test]
fn logique_32_bits_retenue_du_decaleur() {
    // ANDS r0,r1,#0xFF000000 (immediat tourne) : C = bit 31 de l'immediat = 1.
    let mut m = machine(&[0xF011, 0x407F], V);
    m.cpu.regs.set_reg(1, 0x1234_5678);
    run(&mut m, 1);
    assert_eq!(m.cpu.regs.get_reg(0), 0x1200_0000);
    assert_eq!(flags(&m), C | V, "C du decaleur, V intact");
    // TEQ.W r1,r2,LSL#1 : C = bit 31 de r2 avant decalage.
    let mut m = machine(&[0xEA91, 0x0F42], 0);
    m.cpu.regs.set_reg(1, 0x0000_0002);
    m.cpu.regs.set_reg(2, 0x8000_0001);
    run(&mut m, 1);
    assert_eq!(flags(&m), Z | C, "0x2 ^ 0x2 = 0, retenue = bit sorti");
}

// ------------------------------------------------------------------ multiplications et divisions

#[test]
fn multiplications_longues_et_accumulees() {
    let neg = |x: i32| x as u32;
    // SMULL r0,r1,r2,r3 : -3 * 7 = -21
    let mut m = machine(&[0xFB82, 0x0103], 0);
    m.cpu.regs.set_reg(2, neg(-3));
    m.cpu.regs.set_reg(3, 7);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(1), m.cpu.regs.get_reg(0)), (0xFFFF_FFFF, neg(-21)));
    // UMULL : 0xFFFFFFFF^2 = 0xFFFFFFFE_00000001
    let mut m = machine(&[0xFBA2, 0x0103], 0);
    m.cpu.regs.set_reg(2, 0xFFFF_FFFF);
    m.cpu.regs.set_reg(3, 0xFFFF_FFFF);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(1), m.cpu.regs.get_reg(0)), (0xFFFF_FFFE, 1));
    // SMLAL : acc 0x00000001_00000000 + (-1 * 1) = 0x00000000_FFFFFFFF
    let mut m = machine(&[0xFBC2, 0x0103], 0);
    m.cpu.regs.set_reg(0, 0);
    m.cpu.regs.set_reg(1, 1);
    m.cpu.regs.set_reg(2, 0xFFFF_FFFF);
    m.cpu.regs.set_reg(3, 1);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(1), m.cpu.regs.get_reg(0)), (0, 0xFFFF_FFFF));
    // UMLAL : acc 0xFFFFFFFF_FFFFFFFF + 1*1 = 0 (retenue perdue)
    let mut m = machine(&[0xFBE2, 0x0103], 0);
    m.cpu.regs.set_reg(0, 0xFFFF_FFFF);
    m.cpu.regs.set_reg(1, 0xFFFF_FFFF);
    m.cpu.regs.set_reg(2, 1);
    m.cpu.regs.set_reg(3, 1);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(1), m.cpu.regs.get_reg(0)), (0, 0));
    // MLA r0,r1,r2,r3 = r3 + r1*r2 ; MLS = r3 - r1*r2
    for (w2, attendu) in [(0x3002u16, 100u32 + 6 * 7), (0x3012, 100 - 42)] {
        let mut m = machine(&[0xFB01, w2], N | Z | C | V);
        m.cpu.regs.set_reg(1, 6);
        m.cpu.regs.set_reg(2, 7);
        m.cpu.regs.set_reg(3, 100);
        run(&mut m, 1);
        assert_eq!(m.cpu.regs.get_reg(0), attendu);
        assert_eq!(flags(&m), NZCV, "MLA/MLS ne posent pas de drapeau");
    }
}

#[test]
fn divisions_zero_debordement_et_arrondi_vers_zero() {
    // SDIV r0,r1,r2 / UDIV r0,r1,r2 ; DIV_0_TRP = 0 (reset) -> resultat 0
    let cas: &[(&str, u16, u32, u32, u32)] = &[
        ("SDIV -7/2", 0xFB91, (-7i32) as u32, 2, (-3i32) as u32),
        ("SDIV 7/-2", 0xFB91, 7, (-2i32) as u32, (-3i32) as u32),
        ("SDIV MIN/-1", 0xFB91, 0x8000_0000, 0xFFFF_FFFF, 0x8000_0000),
        ("SDIV x/0", 0xFB91, 123, 0, 0),
        ("UDIV max/3", 0xFBB1, 0xFFFF_FFFF, 3, 0x5555_5555),
        ("UDIV x/0", 0xFBB1, 123, 0, 0),
    ];
    for &(nom, w1, a, b, q) in cas {
        let mut m = machine(&[w1, 0xF0F2], 0);
        m.cpu.regs.set_reg(1, a);
        m.cpu.regs.set_reg(2, b);
        run(&mut m, 1);
        assert_eq!(m.cpu.regs.get_reg(0), q, "{nom}");
    }
}

// ------------------------------------------------------------------ champs de bits, inversions, extensions

#[test]
fn champs_de_bits_inversions_extensions() {
    let un = |prog: &[u16], r0: u32, r1: u32| {
        let mut m = machine(prog, 0);
        m.cpu.regs.set_reg(0, r0);
        m.cpu.regs.set_reg(1, r1);
        run(&mut m, 1);
        m.cpu.regs.get_reg(0)
    };
    assert_eq!(un(&[0xF341, 0x1007], 0, 0x0000_0F80), 0xFFFF_FFF8, "SBFX #4,#8");
    assert_eq!(un(&[0xF3C1, 0x1007], 0, 0x0000_0F80), 0x0000_00F8, "UBFX #4,#8");
    assert_eq!(un(&[0xF361, 0x100B], 0xFFFF_FFFF, 0x0000_00A5), 0xFFFF_FA5F, "BFI #4,#8");
    assert_eq!(un(&[0xF36F, 0x100B], 0xFFFF_FFFF, 0), 0xFFFF_F00F, "BFC #4,#8");
    assert_eq!(un(&[0xBA08], 0, 0x1122_3344), 0x4433_2211, "REV");
    assert_eq!(un(&[0xBA48], 0, 0x1122_3344), 0x2211_4433, "REV16");
    assert_eq!(un(&[0xBAC8], 0, 0x1122_3380), 0xFFFF_8033, "REVSH");
    assert_eq!(un(&[0xFA91, 0xF0A1], 0, 0x0000_0001), 0x8000_0000, "RBIT");
    assert_eq!(un(&[0xFAB1, 0xF081], 0, 0), 32, "CLZ 0");
    assert_eq!(un(&[0xFAB1, 0xF081], 0, 0x0001_0000), 15, "CLZ");
    assert_eq!(un(&[0xFA0F, 0xF091], 0, 0x00FF_8000), 0xFFFF_FF80, "SXTH ror #8");
    assert_eq!(un(&[0xFA5F, 0xF0A1], 0, 0x00AB_0000), 0x0000_00AB, "UXTB ror #16");
}

// ------------------------------------------------------------------ acces memoire

#[test]
fn ldm_stm_16_bits_et_base_dans_la_liste() {
    // LDM r0!,{r1,r2} : ecriture de la base
    let mut m = machine(&[0xC806], 0);
    wr32(&mut m, RAM, 11);
    wr32(&mut m, RAM + 4, 22);
    m.cpu.regs.set_reg(0, RAM);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(1), m.cpu.regs.get_reg(2), m.cpu.regs.get_reg(0)), (11, 22, RAM + 8));
    // LDM r0,{r0,r1} : base dans la liste -> pas d'ecriture de la base
    let mut m = machine(&[0xC803], 0);
    wr32(&mut m, RAM, 0xAAAA);
    wr32(&mut m, RAM + 4, 0xBBBB);
    m.cpu.regs.set_reg(0, RAM);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(0), m.cpu.regs.get_reg(1)), (0xAAAA, 0xBBBB));
}

#[test]
fn push_pop_w_et_ldrd_strd_indexes() {
    // PUSH.W {r4,lr} puis POP.W {r4,pc}
    let mut m = machine(&[0xE92D, 0x4010, 0xE8BD, 0x8010], 0);
    let sp0 = m.cpu.regs.msp;
    m.cpu.regs.set_reg(4, 0x4444);
    m.cpu.regs.lr = 0x0000_0201;
    run(&mut m, 1);
    assert_eq!(m.cpu.regs.msp, sp0 - 8);
    assert_eq!((rd32(&m, sp0 - 8), rd32(&m, sp0 - 4)), (0x4444, 0x201));
    m.cpu.regs.set_reg(4, 0);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(4), m.cpu.regs.pc, m.cpu.regs.msp), (0x4444, 0x200, sp0));
    // LDRD r2,r3,[r0,#8]! : pre-indexe avec ecriture
    let mut m = machine(&[0xE9F0, 0x2302], 0);
    wr32(&mut m, RAM + 8, 5);
    wr32(&mut m, RAM + 12, 6);
    m.cpu.regs.set_reg(0, RAM);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(2), m.cpu.regs.get_reg(3), m.cpu.regs.get_reg(0)), (5, 6, RAM + 8));
    // STRD r2,r3,[r0],#-8 : post-indexe
    let mut m = machine(&[0xE860, 0x2302], 0);
    m.cpu.regs.set_reg(0, RAM + 16);
    m.cpu.regs.set_reg(2, 7);
    m.cpu.regs.set_reg(3, 8);
    run(&mut m, 1);
    assert_eq!((rd32(&m, RAM + 16), rd32(&m, RAM + 20), m.cpu.regs.get_reg(0)), (7, 8, RAM + 8));
}

#[test]
fn ldr_post_index_et_ldrsb_pre_index_negatif() {
    // LDR.W r1,[r0],#4
    let mut m = machine(&[0xF850, 0x1B04], 0);
    wr32(&mut m, RAM, 0xCAFE_F00D);
    m.cpu.regs.set_reg(0, RAM);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(1), m.cpu.regs.get_reg(0)), (0xCAFE_F00D, RAM + 4));
    // LDRSB.W r1,[r0,#-1]!
    let mut m = machine(&[0xF910, 0x1D01], 0);
    wr32(&mut m, RAM, 0x0000_8000);
    m.cpu.regs.set_reg(0, RAM + 2);
    run(&mut m, 1);
    assert_eq!((m.cpu.regs.get_reg(1), m.cpu.regs.get_reg(0)), (0xFFFF_FF80, RAM + 1));
}

#[test]
fn ldr_non_aligne_en_sram_comme_le_cortex_m3() {
    // LDR r1,[r0] (16 bits) a une adresse impaire : UNALIGN_TRP = 0,
    // l'acces est fait en petit-boutiste sur les octets (pas d'alignement).
    let mut m = machine(&[0x6801], 0);
    wr32(&mut m, RAM, 0x4433_2211);
    wr32(&mut m, RAM + 4, 0x8877_6655);
    m.cpu.regs.set_reg(0, RAM + 1);
    run(&mut m, 1);
    assert_eq!(m.cpu.regs.get_reg(1), 0x5544_3322);
}

#[test]
fn tbb_tbh_relatifs_au_pc() {
    // TBB [pc,r0] a BASE : table a BASE+4, cible = BASE+4 + 2*entree
    let mut m = machine(&[0xE8DF, 0xF000, 0x0302, 0x0000], 0);
    m.cpu.regs.set_reg(0, 1);
    run(&mut m, 1);
    assert_eq!(m.cpu.regs.pc, BASE + 4 + 2 * 3);
    // TBH [pc,r0,lsl#1]
    let mut m = machine(&[0xE8DF, 0xF010, 0x0002, 0x0010], 0);
    m.cpu.regs.set_reg(0, 1);
    run(&mut m, 1);
    assert_eq!(m.cpu.regs.pc, BASE + 4 + 2 * 0x10);
}
