//! Drapeaux des encodages 16 bits dans un bloc IT (ARMv7-M A7.3.3, et le
//! pseudo-code `setflags = !InITBlock()` de chaque instruction 16 bits).
//!
//! Dans un bloc IT, MOVS, ADDS, SUBS, LSLS, LSRS, ASRS, ANDS, EORS, ADCS,
//! SBCS, RORS, RSBS, ORRS, MULS, BICS et MVNS (encodages 16 bits) ne posent
//! aucun drapeau ; CMP, CMN et TST les posent toujours. Hors bloc, tous les
//! posent. Les formes 32 bits suivent leur bit S, bloc IT ou non.

use capybara::emulator::Machine;

const N: u32 = 0x8000_0000;
const Z: u32 = 0x4000_0000;
const C: u32 = 0x2000_0000;
const V: u32 = 0x1000_0000;
const NZCV: u32 = N | Z | C | V;
const BASE: u32 = 0x100;

/// Machine prete a executer `prog` depuis la PRAM (pas a 0 : le coeur y voit
/// un saut nul et s'arrete).
fn machine(prog: &[u16], flags: u32) -> Machine {
    let mut m = Machine::new();
    for (i, w) in prog.iter().enumerate() {
        let a = BASE as usize + 2 * i;
        m.bus.pram.write_u8(a, (*w & 0xFF) as u8);
        m.bus.pram.write_u8(a + 1, (*w >> 8) as u8);
    }
    m.cpu.regs.pc = BASE;
    m.cpu.regs.xpsr = (m.cpu.regs.xpsr & !NZCV) | flags;
    m
}

fn flags(m: &Machine) -> u32 {
    m.cpu.regs.xpsr & NZCV
}

fn run(m: &mut Machine, n: usize) {
    for _ in 0..n {
        m.step();
    }
}

/// Instructions 16 bits qui ne posent pas de drapeau dans un bloc IT, avec
/// les operandes r0 = 5, r1 = 3 (rd = r0 pour l'ALU, r1/r2 sinon), le
/// registre de destination et le resultat attendu (avec C = 1 en entree).
const SANS_DRAPEAU_DANS_IT: &[(&str, u16, u8, u32)] = &[
    ("MOVS r0,#0", 0x2000, 0, 0),
    ("ADDS r0,#1", 0x3001, 0, 6),
    ("SUBS r0,#5", 0x3805, 0, 0),
    ("LSLS r1,r0,#31", 0x07C1, 1, 0x8000_0000),
    ("LSRS r1,r0,#1", 0x0841, 1, 2),
    ("ASRS r1,r0,#1", 0x1041, 1, 2),
    ("LSRS r1,r0,#32", 0x0801, 1, 0),
    ("ASRS r1,r0,#32", 0x1001, 1, 0),
    ("ADDS r2,r0,r1", 0x1842, 2, 8),
    ("SUBS r2,r0,r1", 0x1A42, 2, 2),
    ("ADDS r2,r0,#3", 0x1CC2, 2, 8),
    ("SUBS r2,r0,#3", 0x1EC2, 2, 2),
    ("ANDS r0,r1", 0x4008, 0, 1),
    ("EORS r0,r1", 0x4048, 0, 6),
    ("LSLS r0,r1", 0x4088, 0, 40),
    ("LSRS r0,r1", 0x40C8, 0, 0),
    ("ASRS r0,r1", 0x4108, 0, 0),
    ("ADCS r0,r1", 0x4148, 0, 9),
    ("SBCS r0,r1", 0x4188, 0, 2),
    ("RORS r0,r1", 0x41C8, 0, 0xA000_0000),
    ("RSBS r0,r1,#0", 0x4248, 0, 0xFFFF_FFFD),
    ("ORRS r0,r1", 0x4308, 0, 7),
    ("MULS r0,r1", 0x4348, 0, 15),
    ("BICS r0,r1", 0x4388, 0, 4),
    ("MVNS r0,r1", 0x43C8, 0, 0xFFFF_FFFC),
];

/// Depart avec Z = 1 (EQ vrai) et deux jeux de N, C, V : le resultat hors bloc
/// differe toujours d'au moins un des deux.
const DEPARTS: &[u32] = &[NZCV, Z | C];

#[test]
fn dans_un_bloc_it_les_formes_16_bits_ne_posent_aucun_drapeau() {
    for &(nom, op, rd, attendu) in SANS_DRAPEAU_DANS_IT {
        for &depart in DEPARTS {
            // IT EQ ; op
            let mut m = machine(&[0xBF08, op], depart);
            m.cpu.regs.set_reg(0, 5);
            m.cpu.regs.set_reg(1, 3);
            run(&mut m, 2);
            assert_eq!(m.cpu.regs.get_reg(rd), attendu, "{nom} : resultat dans IT");
            assert_eq!(flags(&m), depart, "{nom} : drapeaux inchanges dans IT (depart {depart:08X})");
            assert_eq!(m.cpu.regs.itstate, 0, "{nom} : bloc termine");
            assert_eq!(m.cpu.regs.pc, BASE + 4);
        }
    }
}

#[test]
fn hors_bloc_it_les_memes_formes_posent_leurs_drapeaux() {
    for &(nom, op, rd, attendu) in SANS_DRAPEAU_DANS_IT {
        let mut change = false;
        for &depart in DEPARTS {
            let mut m = machine(&[op], depart);
            m.cpu.regs.set_reg(0, 5);
            m.cpu.regs.set_reg(1, 3);
            run(&mut m, 1);
            assert_eq!(m.cpu.regs.get_reg(rd), attendu, "{nom} : resultat hors IT");
            // N et Z suivent toujours le resultat hors bloc.
            let nz = flags(&m) & (N | Z);
            let nz_attendu = (if attendu & 0x8000_0000 != 0 { N } else { 0 })
                | (if attendu == 0 { Z } else { 0 });
            assert_eq!(nz, nz_attendu, "{nom} : N et Z hors IT");
            change |= flags(&m) != depart;
        }
        assert!(change, "{nom} : hors bloc, les drapeaux doivent bouger");
    }
}

#[test]
fn valeurs_exactes_des_drapeaux_hors_bloc() {
    // (programme, r0, r1, drapeaux de depart, drapeaux attendus)
    let cas: &[(&str, u16, u32, u32, u32, u32)] = &[
        // LSLS r1,r0,#1 : retenue = bit 31 sorti.
        ("LSLS #1", 0x0041, 0x8000_0001, 0, 0, C),
        // LSRS r1,r0,#1 : retenue = bit 0 sorti, resultat nul.
        ("LSRS #1", 0x0841, 1, 0, 0, Z | C),
        // ASRS r1,r0,#32 : tout le signe.
        ("ASRS #32", 0x1001, 0x8000_0000, 0, 0, N | C),
        // ADDS r0,#1 sur 0x7FFFFFFF : debordement signe.
        ("ADDS #1", 0x3001, 0x7FFF_FFFF, 0, 0, N | V),
        // SUBS r0,#1 sur 0 : emprunt (C = 0), N = 1.
        ("SUBS #1", 0x3801, 0, 0, C, N),
        // ADCS r0,r1 avec C = 1 : 0xFFFFFFFF + 0 + 1 = 0, retenue.
        ("ADCS", 0x4148, 0xFFFF_FFFF, 0, C, Z | C),
        // SBCS r0,r1 avec C = 0 : 0 - 0 - 1 = -1, emprunt.
        ("SBCS", 0x4188, 0, 0, 0, N),
        // MULS : C et V intacts.
        ("MULS", 0x4348, 0, 7, C | V, Z | C | V),
    ];
    for &(nom, op, r0, r1, depart, attendu) in cas {
        let mut m = machine(&[op], depart);
        m.cpu.regs.set_reg(0, r0);
        m.cpu.regs.set_reg(1, r1);
        run(&mut m, 1);
        assert_eq!(flags(&m), attendu, "{nom}");
    }
}

#[test]
fn cmp_cmn_tst_posent_leurs_drapeaux_meme_dans_un_bloc_it() {
    // (nom, op, r0, r1, drapeaux attendus) ; depart Z = 1 (EQ vrai), N = C = V = 0.
    let cas: &[(&str, u16, u32, u32, u32)] = &[
        ("CMP r0,#1", 0x2801, 0, 0, N),
        ("CMP r0,r1", 0x4288, 5, 3, C),
        ("CMN r0,r1", 0x42C8, 0xFFFF_FFFF, 1, Z | C),
        ("TST r0,r1", 0x4208, 0x8000_0000, 0x8000_0000, N),
        ("CMP hi r8,r0", 0x4580, 0, 0, Z | C), // r8 = 0 - r0 = 0
    ];
    for &(nom, op, r0, r1, attendu) in cas {
        let mut m = machine(&[0xBF08, op], Z);
        m.cpu.regs.set_reg(0, r0);
        m.cpu.regs.set_reg(1, r1);
        run(&mut m, 2);
        assert_eq!(flags(&m), attendu, "{nom} dans IT");
    }
}

#[test]
fn la_derniere_instruction_du_bloc_ne_pose_pas_de_drapeau() {
    // ITT EQ ; ADDS r0,#1 ; ADDS r0,#1 ; ADDS r0,#1 (hors bloc)
    let mut m = machine(&[0xBF04, 0x3001, 0x3001, 0x3001], Z);
    m.cpu.regs.set_reg(0, 0xFFFF_FFFE);
    run(&mut m, 3);
    assert_eq!(m.cpu.regs.get_reg(0), 0);
    assert_eq!(m.cpu.regs.itstate, 0, "le bloc est fini apres la derniere");
    assert_eq!(flags(&m), Z, "ni la premiere ni la derniere ne posent");
    run(&mut m, 1);
    assert_eq!(m.cpu.regs.get_reg(0), 1);
    assert_eq!(flags(&m), 0, "apres le bloc, ADDS pose de nouveau");
}

#[test]
fn le_bloc_garde_sa_condition_apres_un_subs() {
    // C'est l'effet observable de l'ancien comportement : ITT NE ; SUBS r0,#1
    // (1 -> 0) ; MOVS r1,#9. Avec Z pose par SUBS, la seconde etait sautee.
    let mut m = machine(&[0xBF1C, 0x3801, 0x2109], 0);
    m.cpu.regs.set_reg(0, 1);
    m.cpu.regs.set_reg(1, 0x55);
    run(&mut m, 3);
    assert_eq!(m.cpu.regs.get_reg(0), 0);
    assert_eq!(m.cpu.regs.get_reg(1), 9, "la seconde instruction NE s'execute");
    assert_eq!(flags(&m), 0);
}

#[test]
fn ite_la_branche_else_reste_sautee() {
    // ITE EQ ; MOVS r0,#5 ; MOVS r1,#9 avec Z = 1 : then execute, else sautee.
    let mut m = machine(&[0xBF0C, 0x2005, 0x2109], Z);
    m.cpu.regs.set_reg(1, 0x55);
    run(&mut m, 3);
    assert_eq!(m.cpu.regs.get_reg(0), 5);
    assert_eq!(m.cpu.regs.get_reg(1), 0x55, "la branche else est sautee");
    assert_eq!(flags(&m), Z);

    // ITE EQ avec Z = 0 : then sautee, else executee, drapeaux intacts.
    let mut m = machine(&[0xBF0C, 0x2005, 0x2109], C);
    m.cpu.regs.set_reg(0, 0x77);
    run(&mut m, 3);
    assert_eq!(m.cpu.regs.get_reg(0), 0x77, "then sautee");
    assert_eq!(m.cpu.regs.get_reg(1), 9, "else executee");
    assert_eq!(flags(&m), C);
}

#[test]
fn un_cmp_dans_le_bloc_change_la_condition_des_suivantes() {
    // ITTT EQ ; CMP r0,#1 (r0 = 0 -> Z = 0) ; MOVS r1,#9 ; MOVS r2,#9
    let mut m = machine(&[0xBF02, 0x2801, 0x2109, 0x2209], Z);
    m.cpu.regs.set_reg(1, 0x55);
    m.cpu.regs.set_reg(2, 0x66);
    run(&mut m, 4);
    assert_eq!(m.cpu.regs.get_reg(1), 0x55, "EQ devenu faux : sautee");
    assert_eq!(m.cpu.regs.get_reg(2), 0x66, "EQ devenu faux : sautee");
    assert_eq!(flags(&m), N);
    assert_eq!(m.cpu.regs.itstate, 0);
}

#[test]
fn une_instruction_sautee_ne_pose_rien() {
    // IT NE avec Z = 1 : ADDS sautee.
    let mut m = machine(&[0xBF18, 0x3001], Z | V);
    m.cpu.regs.set_reg(0, 0x7FFF_FFFF);
    run(&mut m, 2);
    assert_eq!(m.cpu.regs.get_reg(0), 0x7FFF_FFFF);
    assert_eq!(flags(&m), Z | V);
}

#[test]
fn formes_32_bits_suivent_le_bit_s_meme_dans_un_bloc_it() {
    // IT EQ ; ADDS.W r0, r0, #1 (0xF110 0x0001) : S = 1, drapeaux poses.
    let mut m = machine(&[0xBF08, 0xF110, 0x0001], Z);
    m.cpu.regs.set_reg(0, 0xFFFF_FFFF);
    run(&mut m, 2);
    assert_eq!(m.cpu.regs.get_reg(0), 0);
    assert_eq!(flags(&m), Z | C, "ADDS.W pose ses drapeaux dans IT");

    // IT EQ ; ADD.W r0, r0, #1 (0xF100 0x0001) : S = 0, rien.
    let mut m = machine(&[0xBF08, 0xF100, 0x0001], Z | V);
    m.cpu.regs.set_reg(0, 0xFFFF_FFFF);
    run(&mut m, 2);
    assert_eq!(m.cpu.regs.get_reg(0), 0);
    assert_eq!(flags(&m), Z | V, "ADD.W ne pose rien");

    // IT EQ ; LSLS.W r2, r0, r1 (0xFA10 0xF201) : S = 1.
    let mut m = machine(&[0xBF08, 0xFA10, 0xF201], Z);
    m.cpu.regs.set_reg(0, 0x8000_0000);
    m.cpu.regs.set_reg(1, 1);
    run(&mut m, 2);
    assert_eq!(m.cpu.regs.get_reg(2), 0);
    assert_eq!(flags(&m), Z | C, "LSLS.W pose ses drapeaux dans IT");
}

#[test]
fn l_appel_direct_hors_bloc_pose_toujours() {
    use capybara::emulator::cpu::registers::Registers;
    use capybara::emulator::cpu::thumb16::Thumb16;
    use capybara::emulator::cpu::Nvic;
    use capybara::emulator::mmu::MemoryBus;
    use capybara::emulator::peripherals::Peripherals;

    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();
    // MOVS r0,#0 : execute() = hors bloc.
    Thumb16::execute(0x2000, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert!(regs.flag_z());
    // execute_it(.., true, ..) : MOVS r0,#5 ne touche pas Z.
    Thumb16::execute_it(0x2005, true, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 5);
    assert!(regs.flag_z());
}
