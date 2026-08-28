use tamagotchi_paradise_rs::emulator::cpu::registers::Registers;
use tamagotchi_paradise_rs::emulator::cpu::thumb16::{StepResult, Thumb16};
use tamagotchi_paradise_rs::emulator::cpu::thumb32::Thumb32;
use tamagotchi_paradise_rs::emulator::cpu::Nvic;
use tamagotchi_paradise_rs::emulator::mmu::{map, periph, MemoryBus};
use tamagotchi_paradise_rs::emulator::peripherals::Peripherals;
use tamagotchi_paradise_rs::emulator::Machine;

/// Dump de la console de l'auteur, absent du depot. Les tests qui en dependent
/// sont neutres quand il n'est pas la.
const REAL_DUMP: &str = r"%SONIX_DUMPS%\Tamagotchi_Paradise_Earth_MX25L12835F.bin";
/// Water est la seule edition qui imprime sa console de debug pendant le boot.
/// C'est donc elle qui rend visible le rejet du fabricant de flash.
const REAL_DUMP_WATER: &str =
    r"%SONIX_DUMPS%\Tamagotchi_Paradise_Water_MX25L12835F.bin";
const REAL_DEVICE_KEY: u32 = 0x0000_0000;
const REAL_ENTRY_SP: u32 = 0x1801_EE38;
const REAL_ENTRY_PC: u32 = 0x0000_02F5;

#[test]
fn machine_sans_firmware_ne_tourne_pas() {
    let machine = Machine::new();
    assert!(!machine.is_running, "rien ne doit s'executer sans dump charge");
    assert!(!machine.bus.pram.loaded);
    assert!(machine.last_report.is_none());
}

#[test]
fn test_thumb16_mov_and_add() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // MOVS r0, #42
    assert!(matches!(
        Thumb16::execute(0x202A, &mut regs, &mut bus, &mut periph, &mut nvic),
        StepResult::Ok(_)
    ));
    assert_eq!(regs.get_reg(0), 42);
    assert!(!regs.flag_z());
    assert!(!regs.flag_n());

    // ADDS r0, #8
    assert!(matches!(
        Thumb16::execute(0x3008, &mut regs, &mut bus, &mut periph, &mut nvic),
        StepResult::Ok(_)
    ));
    assert_eq!(regs.get_reg(0), 50);
}

#[test]
fn test_thumb32_movw_movt() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    Thumb32::execute(0xF244, 0x1100, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(1), 0x0000_4100);
    Thumb32::execute(0xF2C4, 0x5100, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(1), 0x4500_4100);
}

#[test]
fn str_word_immediate_ecrit_32_bits_pas_un_octet() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // STR r1, [r2, #0] = 0x6011. Le flag octet est le bit 12, pas le bit 13.
    regs.set_reg(1, 0xA55A_0000);
    regs.set_reg(2, map::SRAM_BASE);
    assert!(matches!(
        Thumb16::execute(0x6011, &mut regs, &mut bus, &mut periph, &mut nvic),
        StepResult::Ok(_)
    ));
    assert_eq!(bus.read_u32(map::SRAM_BASE, &mut periph, &nvic), 0xA55A_0000);
}

#[test]
fn orr_immediat_32_bits_positionne_le_bit_demande() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // ORR r0, r0, #0x10 = 0xF040 0x0010. Devait positionner le bit 4 de r0.
    Thumb32::execute(0xF040, 0x0010, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 0x10);

    // ADD r0, r0, #1 = 0xF100 0x0001 (op=8).
    Thumb32::execute(0xF100, 0x0001, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 0x11);

    // BIC r1, r1, #0x10 = 0xF021 0x0110 (op=1).
    regs.set_reg(1, 0xFFFF_FFFF);
    Thumb32::execute(0xF021, 0x0110, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(1), 0xFFFF_FFEF);
}

#[test]
fn mov_immediat_32_bits_rn_zero_ne_lit_pas_pc() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    regs.pc = 0x0000_8A68;
    // MOV r1, #0x45000000 = ORR r1, 0xF, #imm = 0xF04F 0x418A.
    // ThumbExpandImm(0x48A) = ROR(0x8A, 9) = 0x45000000.
    Thumb32::execute(0xF04F, 0x418A, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(1), 0x4500_0000);
}

#[test]
fn cbz_et_cbnz_visent_la_bonne_adresse() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // CBNZ r0, +8 = 0xB920. Le PC architectural vaut adresse + 4, or step()
    // n'a avance que de 2 : la cible vaut adresse + 4 + 8, pas + 2 + 8.
    regs.pc = 0x1002; // adresse 0x1000, step() a deja ajoute 2.
    regs.set_reg(0, 1);
    Thumb16::execute(0xB920, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.pc, 0x100C);

    // CBZ r0, +8 = 0xB120, pris quand r0 est nul.
    regs.pc = 0x1002;
    regs.set_reg(0, 0);
    Thumb16::execute(0xB120, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.pc, 0x100C);

    // Non pris : le PC ne bouge pas.
    regs.pc = 0x1002;
    regs.set_reg(0, 0);
    Thumb16::execute(0xB920, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.pc, 0x1002);
}

#[test]
fn tbb_et_tbh_branchent_via_la_table() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // TBB [PC, R1] a l'adresse 0x1000 : le PC architectural vaut 0x1004.
    regs.pc = 0x1004;
    regs.set_reg(1, 4);
    bus.write_u32(0x1008, 0x0C, &mut periph, &mut nvic); // octet de table 0x0C
    Thumb32::execute(0xE8DF, 0xF001, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.pc, 0x1004 + 2 * 0x0C);

    // TBH [PC, R1, LSL#1] : bit 4 du second demi-mot = 1, demi-mot a PC + 2*Rm.
    regs.pc = 0x1004;
    regs.set_reg(1, 2);
    bus.write_u32(0x1008, 0x0005, &mut periph, &mut nvic); // demi-mot de table 5
    Thumb32::execute(0xE8DF, 0xF011, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.pc, 0x1004 + 2 * 5);
}

#[test]
fn alu_32_bits_forme_registre_additionne_et_soustrait() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    regs.set_reg(0, 0x20);
    regs.set_reg(1, 0x0F);
    // Le champ d'operation tient sur 4 bits, comme la forme immediat.
    // ADD.W r0, r0, r1 : op = 1000, donc w1 = 0xEB00.
    Thumb32::execute(0xEB00, 0x0001, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 0x2F);

    // SUB.W r0, r0, r1 : op = 1101, donc w1 = 0xEBA0.
    Thumb32::execute(0xEBA0, 0x0001, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 0x20);

    // MOV.W r2, r1 : op = 0010 avec Rn = PC, donc w1 = 0xEA4F.
    // Cette forme s'executait en SUB avec l'ancien decodage sur 3 bits.
    Thumb32::execute(0xEA4F, 0x0201, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(2), 0x0F);

    // ORR.W r3, r0, r1 : op = 0010 avec un Rn reel.
    Thumb32::execute(0xEA40, 0x0301, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(3), 0x2F);
}

#[test]
fn ldr_word_immediate_lit_32_bits_pas_un_octet() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // LDR r1, [r1, #0] = 0x6809.
    bus.write_u32(map::SRAM_BASE, 0xDEAD_BEEF, &mut periph, &mut nvic);
    regs.set_reg(1, map::SRAM_BASE);
    assert!(matches!(
        Thumb16::execute(0x6809, &mut regs, &mut bus, &mut periph, &mut nvic),
        StepResult::Ok(_)
    ));
    assert_eq!(regs.get_reg(1), 0xDEAD_BEEF);
}

// -- Carte memoire, datasheet SNC7340 V1.7 section 4 --

#[test]
fn la_pram_est_mappee_a_zero_et_pas_la_rom() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let nvic = Nvic::default();

    bus.pram.write_u8(0, 0xAB);
    assert_eq!(bus.read_u8(0, &mut periph, &nvic), 0xAB);
}

#[test]
fn sram_et_mailbox_aux_adresses_du_datasheet() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    bus.write_u32(map::SRAM_BASE, 0xDEAD_BEEF, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(map::SRAM_BASE, &mut periph, &nvic), 0xDEAD_BEEF);
    // Le sommet de pile du vrai firmware doit tomber dans la SRAM.
    assert!((map::SRAM_BASE..=map::SRAM_END).contains(&REAL_ENTRY_SP));

    bus.write_u32(map::MAILBOX_BASE, 0x1234_5678, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(map::MAILBOX_BASE, &mut periph, &nvic), 0x1234_5678);
    // La mailbox ne fait que 4 Ko, elle ne deborde pas sur la suite.
    assert_eq!(map::MAILBOX_END - map::MAILBOX_BASE + 1, map::MAILBOX_SIZE as u32);
}

#[test]
fn les_deux_fenetres_flash_voient_le_meme_octet() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let nvic = Nvic::default();

    bus.flash.write_u8(0x11000, 0x5A);
    assert_eq!(bus.read_u8(map::ICACHE_BASE + 0x11000, &mut periph, &nvic), 0x5A);
    assert_eq!(bus.read_u8(map::FLASH_BASE + 0x11000, &mut periph, &nvic), 0x5A);
}

#[test]
fn les_fusibles_exposent_la_device_key() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let nvic = Nvic::default();

    periph.fuses.device_key = Some(REAL_DEVICE_KEY);
    // FEUSE3 porte le mot complet, FEUSE2 seulement les 16 bits de poids fort.
    assert_eq!(bus.read_u32(periph::FUSES + 0x3c, &mut periph, &nvic), REAL_DEVICE_KEY);
    assert_eq!(
        bus.read_u32(periph::FUSES + 0x38, &mut periph, &nvic),
        REAL_DEVICE_KEY & 0xFFFF_0000
    );
}

#[test]
fn test_sonix_sys0_osc_ctrl_hide_bit() {
    let mut machine = Machine::new();
    assert!(!machine.bus.boot_rom.is_hidden);
    machine.bus.write_u32(periph::SYSCTRL0, 0x08, &mut machine.periph, &mut machine.cpu.nvic);
    assert!(machine.bus.boot_rom.is_hidden);
}

#[test]
fn test_uart_console_capture() {
    let mut machine = Machine::new();
    for c in b"HI!" {
        machine.bus.write_u32(
            periph::UART1,
            *c as u32,
            &mut machine.periph,
            &mut machine.cpu.nvic,
        );
    }
    assert_eq!(machine.periph.uart.console_history, "HI!");
}

#[test]
fn test_gpio_buttons_and_dial() {
    let mut machine = Machine::new();

    let initial = machine.bus.read_u32(periph::GPIO0, &mut machine.periph, &machine.cpu.nvic);
    assert_eq!(initial, 0xFFFF_FFFF);

    machine.periph.gpio.set_button_a(true);
    let pressed = machine.bus.read_u32(periph::GPIO0, &mut machine.periph, &machine.cpu.nvic);
    assert_eq!(pressed & 1, 0);

    machine.periph.gpio.step_dial(3);
    let dial = machine.bus.read_u32(periph::GPIO0 + 4, &mut machine.periph, &machine.cpu.nvic);
    assert_eq!(dial, 3);
}

#[test]
fn un_registre_non_mappe_est_trace_et_pas_avale() {
    let mut machine = Machine::new();
    machine.bus.mmio_trace.enabled = true;

    // SPI1 n'est pas encore modelise : l'acces doit laisser une trace.
    machine.bus.write_u32(periph::SPI1 + 8, 0x42, &mut machine.periph, &mut machine.cpu.nvic);
    machine.bus.read_u32(periph::SPI1 + 8, &mut machine.periph, &machine.cpu.nvic);

    let stat = machine.bus.mmio_trace.unknown.get(&(periph::SPI1 + 8)).copied();
    let stat = stat.expect("l'acces aurait du etre enregistre");
    assert_eq!(stat.writes, 1);
    assert_eq!(stat.reads, 1);
    assert_eq!(stat.last_write, 0x42);

    let hot = machine.bus.mmio_trace.hottest(1);
    assert_eq!(hot[0].1, "SPI1", "le registre doit etre attribue a son peripherique");
}

// -- Chargement --

/// Construit un dump minimal mais valide : load table V3 en clair, image
/// utilisateur portant une table de vecteurs.
fn dump_synthetique(entry_pc: u32, entry_sp: u32) -> Vec<u8> {
    let mut d = vec![0u8; 2 * 1024 * 1024];
    d[0..8].copy_from_slice(b"SONIXDEV");
    d[0x08..0x0c].copy_from_slice(&0u32.to_le_bytes()); // non chiffre
    d[0x10..0x14].copy_from_slice(&0x6000_1000u32.to_le_bytes());
    d[0x14..0x18].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    d[0x1f8..0x1fc].copy_from_slice(&0x5a5a_0033u32.to_le_bytes());

    d[0x1000..0x1004].copy_from_slice(&entry_sp.to_le_bytes());
    d[0x1004..0x1008].copy_from_slice(&entry_pc.to_le_bytes());
    // NOP a l'adresse de reset, pour que le premier pas soit deterministe.
    let off = 0x1000 + (entry_pc & !1) as usize;
    d[off..off + 2].copy_from_slice(&0x46C0u16.to_le_bytes());
    d
}

#[test]
fn charge_une_load_table_en_clair_et_demarre() {
    let mut machine = Machine::new();
    let path = std::env::temp_dir().join("sonix_clear.bin");
    std::fs::write(&path, dump_synthetique(0x0000_0201, REAL_ENTRY_SP)).unwrap();

    let report = machine.load_firmware_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(report.bootable);
    assert!(!report.encrypted);
    assert_eq!(report.entry_sp, REAL_ENTRY_SP);
    assert_eq!(report.entry_pc, 0x0000_0201);
    assert_eq!(machine.cpu.regs.msp, REAL_ENTRY_SP);
    assert_eq!(machine.cpu.regs.pc, 0x0000_0200);
    assert!(machine.is_running);

    let pc_avant = machine.cpu.regs.pc;
    assert!(matches!(machine.step(), StepResult::Ok(_)));
    assert_eq!(machine.cpu.regs.pc, pc_avant + 2);
}

#[test]
fn un_fichier_sans_load_table_n_est_pas_demarrable() {
    let mut machine = Machine::new();
    let path = std::env::temp_dir().join("pas_sonix.bin");
    std::fs::write(&path, vec![0xAAu8; 4096]).unwrap();

    let report = machine.load_firmware_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(!report.bootable);
    assert!(!machine.is_running);
}

#[test]
fn dump_reel_chiffre_sans_cle_reste_inspectable() {
    let path = std::path::Path::new(REAL_DUMP);
    if !path.exists() {
        return;
    }
    // Le dump est recopie dans un dossier temporaire : un fichier <dump>.key
    // pose a cote de l'original suffirait sinon a fournir la cle et le test
    // ne testerait plus rien.
    let nu = std::env::temp_dir().join("sonix_sans_cle.bin");
    std::fs::copy(path, &nu).unwrap();
    let _ = std::fs::remove_file(nu.with_extension("bin.key"));

    let mut machine = Machine::new();
    machine.device_key = None;
    std::env::remove_var("SONIX_DEVICE_KEY");

    let report = machine.load_firmware_file(&nu).unwrap();
    let _ = std::fs::remove_file(&nu);

    assert!(report.encrypted, "le code de boot du dump reel est chiffre");
    assert!(!report.bootable, "sans cle, on ne pretend pas pouvoir demarrer");
    assert!(!machine.is_running);
    // Le code XIP, lui, est en clair et reste lisible.
    assert_eq!(machine.bus.flash.read_u16(0x11000), 0xB082);
}

#[test]
fn dump_reel_avec_cle_demarre_sur_le_vrai_vecteur() {
    let path = std::path::Path::new(REAL_DUMP);
    if !path.exists() {
        return;
    }
    let mut machine = Machine::new();
    machine.device_key = Some(REAL_DEVICE_KEY);

    let report = machine.load_firmware_file(path).unwrap();
    assert!(report.encrypted);
    assert!(report.bootable);
    assert_eq!(report.entry_sp, REAL_ENTRY_SP);
    assert_eq!(report.entry_pc, REAL_ENTRY_PC);
    assert_eq!(machine.cpu.regs.pc, REAL_ENTRY_PC & !1);

    // Les vecteurs MemManage, BusFault et UsageFault du firmware reel.
    assert_eq!(machine.bus.pram.read_u32(0x10), 0x0000_0321);
    assert_eq!(machine.bus.pram.read_u32(0x14), 0x0000_0323);
    assert_eq!(machine.bus.pram.read_u32(0x18), 0x0000_0325);
}

#[test]
fn blx_registre_revient_sur_l_instruction_suivante() {
    let mut regs = Registers::default();

    // BLX r3 a l'adresse 0x1000 : step() a deja avance le PC de 2.
    regs.pc = 0x1002;
    regs.set_reg(3, 0x2001);
    Thumb16::execute(0x4798, &mut regs, &mut MemoryBus::default(), &mut Peripherals::default(), &mut Nvic::default());
    assert_eq!(regs.pc, 0x2000, "la cible perd le bit Thumb");
    assert_eq!(regs.lr, 0x1003, "LR doit viser l'instruction suivante, pas celle d'apres");
}

#[test]
fn cmp_registre_16_bits_pose_les_drapeaux() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // CMP r0, r1 avec deux valeurs egales : Z doit monter.
    regs.set_reg(0, 0x494E4F53);
    regs.set_reg(1, 0x494E4F53);
    Thumb16::execute(0x4288, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert!(regs.flag_z(), "Z apres une comparaison egale");
    assert!(regs.flag_c(), "C apres une soustraction sans emprunt");
    assert_eq!(regs.get_reg(0), 0x494E4F53, "CMP ne range pas de resultat");

    // CMP avec r0 < r1 : Z tombe, C tombe.
    regs.set_reg(1, 0x494E4F54);
    Thumb16::execute(0x4288, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert!(!regs.flag_z());
    assert!(!regs.flag_c(), "emprunt sur une soustraction negative");
}

#[test]
fn ldr_post_indexe_avance_la_base() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    bus.write_u8(0x100, 0xAB, &mut periph, &mut nvic);
    bus.write_u8(0x101, 0xCD, &mut periph, &mut nvic);

    // LDRB r6, [r0], #1 : l'acces se fait a la base, qui avance ensuite.
    regs.set_reg(0, 0x100);
    Thumb32::execute(0xF810, 0x6B01, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(6), 0xAB, "l'octet lu est celui de la base");
    assert_eq!(regs.get_reg(0), 0x101, "la base avance de 1 apres l'acces");

    Thumb32::execute(0xF810, 0x6B01, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(6), 0xCD);
    assert_eq!(regs.get_reg(0), 0x102);
}

#[test]
fn push_w_ecrit_sous_le_pointeur_de_pile() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    regs.msp = 0x1000;
    regs.set_reg(4, 0x11111111);
    regs.set_reg(5, 0x22222222);

    // PUSH.W {r4, r5} = STMDB SP!, {r4, r5}
    Thumb32::execute(0xE92D, 0x0030, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_sp(), 0x0FF8, "la pile descend de deux mots");
    assert_eq!(bus.read_u32(0x0FF8, &mut periph, &nvic), 0x11111111);
    assert_eq!(bus.read_u32(0x0FFC, &mut periph, &nvic), 0x22222222);
}

#[test]
fn bloc_it_saute_l_instruction_quand_la_condition_est_fausse() {
    let mut machine = Machine::new();
    // IT NE puis SUBS r3, r7, r3, a executer depuis la PRAM.
    // Pas a l'adresse 0 : le coeur y voit un saut nul et s'arrete.
    machine.bus.pram.write_u8(0x100, 0x18);
    machine.bus.pram.write_u8(0x101, 0xBF); // 0xBF18 = IT NE
    machine.bus.pram.write_u8(0x102, 0xFB);
    machine.bus.pram.write_u8(0x103, 0x1A); // 0x1AFB = SUBS r3, r7, r3
    machine.cpu.regs.pc = 0x100;
    machine.cpu.regs.set_reg(3, 5);
    machine.cpu.regs.set_reg(7, 100);
    machine.cpu.regs.set_flag_z(true); // condition NE fausse

    machine.step(); // IT NE
    assert_eq!(machine.cpu.regs.itstate & 0x0F, 0x08, "le bloc IT est arme");
    machine.step(); // SUBNE, ne doit pas s'executer
    assert_eq!(machine.cpu.regs.get_reg(3), 5, "l'instruction conditionnelle est sautee");
    assert_eq!(machine.cpu.regs.itstate, 0, "le bloc IT est termine");
}

#[test]
fn multiplications_longues_et_divisions_32_bits() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // UMULL r0, r1, r2, r3 : 0xFBA2 0x0103. Le firmware s'en sert pour diviser
    // par 1000 via le reciproque magique 0x10624DD3.
    regs.set_reg(2, 12_000_000);
    regs.set_reg(3, 0x1062_4DD3);
    Thumb32::execute(0xFBA2, 0x0103, &mut regs, &mut bus, &mut periph, &mut nvic);
    let produit = ((regs.get_reg(1) as u64) << 32) | regs.get_reg(0) as u64;
    assert_eq!(produit, 12_000_000u64 * 0x1062_4DD3u64);
    // Le decalage de 38 bits qui suit donne bien 12000 kHz.
    assert_eq!((produit >> 38) as u32, 12_000);

    // SMULL r0, r1, r2, r3 : 0xFB82 0x0103, avec un operande negatif.
    regs.set_reg(2, (-3i32) as u32);
    regs.set_reg(3, 1000);
    Thumb32::execute(0xFB82, 0x0103, &mut regs, &mut bus, &mut periph, &mut nvic);
    let signe = (((regs.get_reg(1) as u64) << 32) | regs.get_reg(0) as u64) as i64;
    assert_eq!(signe, -3000);

    // UDIV r0, r2, r3 : 0xFBB2 0xF0F3. Le champ de discrimination tient sur
    // quatre bits ; masque sur trois, cette forme etait inatteignable.
    regs.set_reg(2, 1000);
    regs.set_reg(3, 3);
    Thumb32::execute(0xFBB2, 0xF0F3, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 333);

    // SDIV r0, r2, r3 : 0xFB92 0xF0F3.
    regs.set_reg(2, (-1000i32) as u32);
    regs.set_reg(3, 3);
    Thumb32::execute(0xFB92, 0xF0F3, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0) as i32, -333);

    // Division par zero : resultat nul, pas de panique.
    regs.set_reg(3, 0);
    Thumb32::execute(0xFBB2, 0xF0F3, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 0);
}

#[test]
fn la_fenetre_xip_suit_la_base_programmee() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    bus.flash.write_u8(0x11000, 0x42);
    bus.flash.write_u8(0x00000, 0x99);

    // Base par defaut : la fenetre montre le debut de la flash.
    assert_eq!(bus.read_u8(map::ICACHE_BASE, &mut periph, &nvic), 0x99);

    // Le firmware programme la base sur le debut de la region XIP.
    bus.write_u32(periph::XIP_CTRL + 4, 0x6001_1000, &mut periph, &mut nvic);
    bus.write_u32(periph::XIP_CTRL, 3, &mut periph, &mut nvic);
    assert_eq!(bus.read_u8(map::ICACHE_BASE, &mut periph, &nvic), 0x42);

    // C'est ce decalage qui fait tomber un saut vers 0x1006D1C4 sur le
    // prologue reel, a l'offset flash 0x11000 + 0x6D1C4.
    bus.flash.write_u8(0x11000 + 0x6D1C4, 0x7E);
    assert_eq!(bus.read_u8(map::ICACHE_BASE + 0x6D1C4, &mut periph, &nvic), 0x7E);
}

#[test]
fn la_region_bit_band_adresse_un_bit_a_la_fois() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // L'alias 0x42340000 vise l'octet 0x4001A000, bit 0 ; 0x42340004 son bit 1.
    // Le firmware scrute un statut par ce chemin, et sans lui le coeur partait
    // executer les octets de l'alias comme du code.
    assert_eq!(map::bitband_target(0x4234_0000), Some((0x4001_A000, 0)));
    assert_eq!(map::bitband_target(0x4234_0004), Some((0x4001_A000, 1)));
    assert_eq!(map::bitband_target(0x2200_0000), Some((0x2000_0000, 0)));
    // Hors des deux fenetres, aucune traduction.
    assert_eq!(map::bitband_target(0x4001_A000), None);

    // Ecrire par l'alias ne touche que le bit vise de la mailbox.
    bus.write_u32(map::MAILBOX_BASE, 0, &mut periph, &mut nvic);
    bus.write_u32(0x2200_0008, 1, &mut periph, &mut nvic); // octet 0, bit 2
    assert_eq!(bus.read_u32(map::MAILBOX_BASE, &mut periph, &nvic) & 0xFF, 0b100);
    assert_eq!(bus.read_u32(0x2200_0008, &mut periph, &nvic), 1);
    assert_eq!(bus.read_u32(0x2200_0004, &mut periph, &nvic), 0);

    bus.write_u32(0x2200_0008, 0, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(map::MAILBOX_BASE, &mut periph, &nvic) & 0xFF, 0);
}

#[test]
fn le_dma_du_controleur_flash_recopie_vraiment() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    for i in 0..64u32 {
        bus.flash.write_u8((0xD49000 + i) as usize, (i as u8).wrapping_mul(3));
    }

    // Sequence relevee dans le firmware : adresse flash, longueur, adresse RAM,
    // puis depart.
    let base = periph::FLASH_CTL;
    bus.write_u32(base + 0x10C, 0x60D4_9000, &mut periph, &mut nvic);
    bus.write_u32(base + 0x104, 64, &mut periph, &mut nvic);
    bus.write_u32(base + 0x100, map::SRAM_BASE + 0x100, &mut periph, &mut nvic);
    // Bit de direction pose pour aller de la flash vers la memoire, puis depart.
    bus.write_u32(base + 0x108, 2, &mut periph, &mut nvic);
    bus.write_u32(base + 0x108, 3, &mut periph, &mut nvic);

    for i in 0..64u32 {
        let attendu = (i as u8).wrapping_mul(3);
        let obtenu = bus.read_u8(map::SRAM_BASE + 0x100 + i, &mut periph, &nvic);
        assert_eq!(obtenu, attendu, "octet {} du transfert", i);
    }
    // Le bit de depart est retombe, le transfert est termine.
    assert_eq!(bus.read_u32(base + 0x108, &mut periph, &nvic) & 1, 0);
}

#[test]
fn l_adc_signale_sa_conversion_terminee() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // Avant toute demande, le bit de fin est absent.
    assert_eq!(bus.read_u32(periph::SAR_ADC1 + 0x14, &mut periph, &nvic) & 0x40, 0);

    // Le firmware ecrit le canal puis attend le bit 6, teste par LSLS #25 / BMI.
    bus.write_u32(periph::SAR_ADC1, 3, &mut periph, &mut nvic);
    let statut = bus.read_u32(periph::SAR_ADC1 + 0x14, &mut periph, &nvic);
    assert_ne!(statut << 25 & 0x8000_0000, 0, "le bit 6 doit etre pose");
}

#[test]
fn decalages_par_registre_et_extensions() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // LSL.W r2, r0, r1 : 0xFA00 0xF201, la forme du set_bit du firmware.
    regs.set_reg(0, 1);
    regs.set_reg(1, 12);
    Thumb32::execute(0xFA00, 0xF201, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(2), 1 << 12);

    // Un decalage de 32 ou plus vide le resultat, la ou l'operateur Rust panique.
    regs.set_reg(1, 40);
    Thumb32::execute(0xFA00, 0xF201, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(2), 0);

    // ASR.W conserve le signe : 0xFA40 0xF201.
    regs.set_reg(0, 0xFFFF_FF00);
    regs.set_reg(1, 4);
    Thumb32::execute(0xFA40, 0xF201, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(2), 0xFFFF_FFF0);

    // UXTB r2, r0 : 0xFA5F 0xF280.
    regs.set_reg(0, 0xDEAD_BE95);
    Thumb32::execute(0xFA5F, 0xF280, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(2), 0x95);
    // SXTB r2, r0 : 0xFA4F 0xF280, le bit de signe est propage.
    Thumb32::execute(0xFA4F, 0xF280, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(2), 0xFFFF_FF95);
}

#[test]
fn bfc_efface_le_champ_au_lieu_d_extraire() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // BFC r4, #0, #3 = 0xF36F 0x0402, l'alignement de pile du handler SysTick.
    // Decode en SBFX, il mettait r4 a zero, donc SP a zero.
    regs.set_reg(4, 0x1801_EE3F);
    Thumb32::execute(0xF36F, 0x0402, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(4), 0x1801_EE38);

    // UBFX r0, r0, #4, #3 : extraction non signee des bits 6:4.
    regs.set_reg(0, 0b0101_0000);
    Thumb32::execute(0xF3C0, 0x1002, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 0b101);

    // BFI r0, r1, #4, #4 : insertion de quatre bits a partir du rang 4.
    regs.set_reg(0, 0xFFFF_FF0F);
    regs.set_reg(1, 0xA);
    Thumb32::execute(0xF361, 0x1007, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 0xFFFF_FFAF);
}

#[test]
fn ldrd_et_strd_transferent_deux_registres() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    regs.set_reg(4, map::SRAM_BASE);
    regs.set_reg(2, 0x1111_2222);
    regs.set_reg(1, 0x3333_4444);
    // STRD r2, r1, [r4, #4] = 0xE9C4 0x2101.
    Thumb32::execute(0xE9C4, 0x2101, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(map::SRAM_BASE + 4, &mut periph, &nvic), 0x1111_2222);
    assert_eq!(bus.read_u32(map::SRAM_BASE + 8, &mut periph, &nvic), 0x3333_4444);

    // LDRD r5, r6, [r4, #4] = 0xE9D4 0x5601.
    Thumb32::execute(0xE9D4, 0x5601, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(5), 0x1111_2222);
    assert_eq!(regs.get_reg(6), 0x3333_4444);
}

#[test]
fn lire_r15_rend_le_pc_architectural() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // ADD r3, pc = 0x447B, a l'adresse 0x1058 : step() a deja ajoute 2, le PC
    // architectural vaut 0x105C. Deux octets manquants suffisaient a faire
    // pointer un pointeur de fonction sur le BX lr de la fonction precedente.
    regs.pc = 0x105A;
    regs.set_reg(3, 0x1000);
    Thumb16::execute(0x447B, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(3), 0x105C + 0x1000);
}

#[test]
fn le_retour_d_exception_restaure_le_contexte() {
    use tamagotchi_paradise_rs::emulator::Mode;

    let mut machine = Machine::new();
    machine.bus.pram.load(&[0u8; 256]);
    machine.cpu.regs.msp = map::SRAM_BASE + 0x1000;
    machine.cpu.regs.pc = 0x200;
    machine.cpu.regs.set_reg(0, 0xAAAA_AAAA);

    let sp_avant = machine.cpu.regs.get_sp();
    // SysTick actif, interruption armee, compteur a bout.
    machine.cpu.nvic.syst_csr = 0b11;
    machine.cpu.nvic.syst_rvr = 1;
    machine.cpu.nvic.syst_cvr = 0;
    machine.cpu.nvic.systick_pending = true;

    machine.step();
    assert_eq!(machine.cpu.regs.mode, Mode::Handler);
    assert_eq!(machine.cpu.regs.lr, 0xFFFF_FFF9, "EXC_RETURN attendu dans LR");
    assert_eq!(machine.cpu.regs.get_sp(), sp_avant - 32);

    // Le coeur doit reconnaitre EXC_RETURN et depiler, pas s'arreter.
    machine.cpu.regs.pc = 0xFFFF_FFF9;
    machine.step();
    assert_eq!(machine.cpu.regs.mode, Mode::Thread);
    assert_eq!(machine.cpu.regs.get_sp(), sp_avant);
    assert_eq!(machine.cpu.regs.get_reg(0), 0xAAAA_AAAA);
    assert_eq!(machine.cpu.regs.pc, 0x200);
}

#[test]
fn l_accelerateur_calcule_le_crc_de_la_page_de_sauvegarde() {
    let path = std::path::Path::new(REAL_DUMP);
    if !path.exists() {
        return;
    }
    let mut machine = Machine::new();
    machine.device_key = Some(REAL_DEVICE_KEY);
    machine.load_firmware_file(path).unwrap();

    // Une page de sauvegarde porte sa propre somme en tete : les deux premiers
    // octets valent le CRC des 4092 suivants, et les deux d'apres son complement.
    // C'est ce controle qui rejetait la sauvegarde tant que l'accelerateur
    // rendait zero, et le firmware affichait alors la chaine de repli du SDK
    // "unsupport chip", sans rapport avec le fabricant de la flash.
    const PAGE: usize = 0xEFE000;
    let attendu = u16::from_le_bytes([
        machine.bus.flash.read_u8(PAGE),
        machine.bus.flash.read_u8(PAGE + 1),
    ]);
    let complement = u16::from_le_bytes([
        machine.bus.flash.read_u8(PAGE + 2),
        machine.bus.flash.read_u8(PAGE + 3),
    ]);
    assert_eq!(attendu, !complement, "en-tete de page coherent");

    // Recopie de la page en SRAM par le DMA du controleur de flash.
    let tampon = map::SRAM_BASE + 0x6000;
    let fc = periph::FLASH_CTL;
    let (p, n) = (&mut machine.periph, &mut machine.cpu.nvic);
    machine.bus.write_u32(fc + 0x10C, 0x6000_0000 + PAGE as u32, p, n);
    machine.bus.write_u32(fc + 0x104, 0x1000, p, n);
    machine.bus.write_u32(fc + 0x100, tampon, p, n);
    machine.bus.write_u32(fc + 0x108, 2, p, n);
    machine.bus.write_u32(fc + 0x108, 3, p, n);

    // Puis la sequence exacte de l'accelerateur, relevee dans le firmware.
    let cs = periph::CHECKSUM;
    machine.bus.write_u32(cs + 0x18, 0xA001, p, n);
    machine.bus.write_u32(cs + 0x14, 0xF0, p, n);
    machine.bus.write_u32(cs + 0x04, tampon + 4, p, n);
    machine.bus.write_u32(cs + 0x08, 0xFFC, p, n);
    machine.bus.write_u32(cs + 0x00, 0x10, p, n);

    let obtenu = machine.bus.read_u32(cs + 0x1C, p, n) as u16;
    assert_eq!(obtenu, attendu, "le CRC calcule doit valider la page");
    // Le bit de lancement doit etre retombe.
    assert_eq!(machine.bus.read_u32(cs, p, n) & 0x10, 0);
}

#[test]
fn le_firmware_reel_valide_sa_sauvegarde_et_cesse_de_se_plaindre() {
    let path = std::path::Path::new(REAL_DUMP);
    if !path.exists() {
        return;
    }
    let mut machine = Machine::new();
    machine.device_key = Some(REAL_DEVICE_KEY);
    machine.load_firmware_file(path).unwrap();

    // La boucle de formatage du printf appelle la sortie avec le caractere dans
    // r0 : l'intercepter donne la console de debug du firmware.
    const SORTIE: u32 = 0x0000_1070;
    let mut console = Vec::new();
    for _ in 0..40_000_000u64 {
        if machine.cpu.regs.pc == SORTIE {
            console.push((machine.cpu.regs.get_reg(0) & 0xFF) as u8);
        }
        if !matches!(machine.step(), StepResult::Ok(_)) {
            break;
        }
    }

    let texte = String::from_utf8_lossy(&console);
    assert!(
        !texte.contains("unsupport chip"),
        "la sauvegarde doit etre validee, console obtenue : {}",
        texte
    );
}

#[test]
fn le_controleur_rend_le_fabricant_de_la_flash_montee() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();
    let fc = periph::FLASH_CTL;

    // Sequence relevee en 0x000039C0 : poser le bit 15 de la commande, attendre
    // qu'il retombe ainsi que le bit 1, puis lire la paire d'identification.
    bus.write_u32(fc + 0x04, 1 << 15, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(fc + 0x04, &mut periph, &nvic) & ((1 << 15) | 2), 0);
    let paire = bus.read_u32(fc + 0x18, &mut periph, &nvic);

    // Le firmware en fait `(paire & 0xFFFF) << 8` puis compare les bits 23:16 au
    // fabricant. Tout ce qui n'est ni 0xC2 ni 0xC8 le fige sans sortie.
    let identifiant = (paire & 0xFFFF) << 8;
    assert_eq!(
        (identifiant >> 16) & 0xFF,
        0xC2,
        "le fabricant doit etre Macronix, sinon le firmware boucle sur son message d'erreur"
    );
}

#[test]
fn le_firmware_reel_accepte_la_flash_et_quitte_son_identification() {
    let path = std::path::Path::new(REAL_DUMP_WATER);
    if !path.exists() {
        return;
    }
    let mut machine = Machine::new();
    machine.device_key = Some(REAL_DEVICE_KEY);
    machine.load_firmware_file(path).unwrap();

    // 0x1006A018 est la boucle d'impression sans sortie du rejet de fabricant.
    // 0x000093C8 suit l'identification dans la fonction d'initialisation :
    // l'atteindre prouve que l'appel a rendu la main.
    const BOUCLE_REJET: u32 = 0x1006_A018;
    const APRES_IDENTIFICATION: u32 = 0x0000_93C8;
    // L'identification aboutit vers 41,4 millions de pas : l'essentiel du delai
    // vient des temporisations d'initialisation qui la suivent.
    let mut passe = false;
    for _ in 0..60_000_000u64 {
        let pc = machine.cpu.regs.pc;
        assert_ne!(pc, BOUCLE_REJET, "le firmware a rejete le fabricant de la flash");
        if pc == APRES_IDENTIFICATION {
            passe = true;
            break;
        }
        if !matches!(machine.step(), StepResult::Ok(_)) {
            break;
        }
    }
    assert!(passe, "l'identification de la flash doit rendre la main a l'initialisation");
}

#[test]
fn le_port_relit_ses_sorties_et_le_niveau_de_ses_entrees() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();
    let p1 = periph::GPIO_PORT1;

    // Broche 5 en sortie, broche 10 en entree : c'est le brochage reel, ou la
    // broche 5 commande le CS de l'ecran et la broche 10 recoit son TE.
    bus.write_u32(p1 + 0x04, 1 << 5, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(p1 + 0x04, &mut periph, &nvic), 1 << 5);

    // Une sortie doit se relire telle qu'ecrite, sinon la lecture-modification
    // -ecriture du bit-band perd l'etat a chaque bit repositionne.
    let donnees = bus.read_u32(p1, &mut periph, &nvic) & !(1 << 5);
    bus.write_u32(p1, donnees, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(p1, &mut periph, &nvic) & (1 << 5), 0);

    // Une entree ignore le verrou de sortie et suit le niveau exterieur.
    periph.port1.entrees &= !(1 << 10);
    assert_eq!(bus.read_u32(p1, &mut periph, &nvic) & (1 << 10), 0);
    periph.port1.entrees |= 1 << 10;
    assert_ne!(bus.read_u32(p1, &mut periph, &nvic) & (1 << 10), 0);
}

#[test]
fn le_te_de_l_ecran_bat_sur_la_broche_dix_du_port_un() {
    let mut periph = Peripherals::default();
    let demi = tamagotchi_paradise_rs::emulator::peripherals::gpio_port::TE_DEMI_PERIODE;

    let etat = |p: &Peripherals| (p.port1.read_reg(0) >> 10) & 1;
    let depart = etat(&periph);
    let _ = periph.port1.tick(demi as u32 + 1);
    assert_ne!(etat(&periph), depart, "le TE doit changer d'etat a la demi-periode");
    let _ = periph.port1.tick(demi as u32);
    assert_eq!(etat(&periph), depart, "puis revenir, sinon aucun front ne se produit");
}

#[test]
fn le_front_du_te_leve_une_interruption_quand_elle_est_autorisee() {
    use tamagotchi_paradise_rs::emulator::peripherals::gpio_port::{TE_DEMI_PERIODE, TE_PIN};
    let mut periph = Peripherals::default();
    let demi = TE_DEMI_PERIODE as u32;

    // Sans autorisation, le front ne doit rien lever.
    let _ = periph.port1.tick(demi + 1);
    assert!(!periph.port1.tick(demi), "aucune interruption tant qu'elle n'est pas autorisee");

    periph.port1.irq_enable = 1 << TE_PIN;
    let _ = periph.port1.tick(demi);
    // Le front montant suivant pose le drapeau et signale l'interruption.
    let mut leve = false;
    for _ in 0..4 {
        leve |= periph.port1.tick(demi);
    }
    assert!(leve, "le front montant du TE doit lever l'interruption du port 1");
    assert_ne!(periph.port1.irq_status & (1 << TE_PIN), 0, "le drapeau doit rester lisible");

    // Le gestionnaire l'efface en ecrivant un a la meme position en 0x20.
    periph.port1.write_reg(0x20, 1 << TE_PIN);
    assert_eq!(periph.port1.irq_status & (1 << TE_PIN), 0);
}

#[test]
fn le_canal_de_transfert_recopie_puis_signale_sa_fin() {
    use tamagotchi_paradise_rs::emulator::peripherals::dma;
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();
    let canal = periph::DMA + dma::CANAL0;
    let source = map::SRAM_BASE + 0x400;
    let destination = map::SRAM_BASE + 0x800;

    for i in 0..8u32 {
        bus.write_u32(source + 4 * i, 0x1000 + i, &mut periph, &mut nvic);
    }

    // Sequence du pilote en 0x000044B8 : compte, destination, source, depart.
    bus.write_u32(canal + dma::COMPTE, 8, &mut periph, &mut nvic);
    bus.write_u32(canal + dma::DESTINATION, destination, &mut periph, &mut nvic);
    bus.write_u32(canal + dma::SOURCE, source, &mut periph, &mut nvic);
    bus.write_u32(canal + dma::CTRL, dma::DEPART, &mut periph, &mut nvic);

    for i in 0..8u32 {
        assert_eq!(
            bus.read_u32(destination + 4 * i, &mut periph, &nvic),
            0x1000 + i,
            "le canal doit avoir recopie le mot {}",
            i
        );
    }
    // Le bit de depart retombe, le drapeau reste lisible jusqu'a l'acquittement.
    assert_eq!(bus.read_u32(canal + dma::CTRL, &mut periph, &nvic) & dma::DEPART, 0);
    assert_ne!(bus.read_u32(periph::DMA + dma::STATUS, &mut periph, &nvic) & 1, 0);
    assert!(periph.dma.irq_a_lever, "la fin de transfert doit etre signalee");

    bus.write_u32(periph::DMA + dma::ACQUIT, 1, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(periph::DMA + dma::STATUS, &mut periph, &nvic) & 1, 0);
}

#[test]
fn l_immediat_modifie_replique_son_octet_au_lieu_de_le_decaler() {
    let mut regs = Registers::default();
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();

    // Les quatre motifs de l'architecture, quand imm12[11:10] vaut 00.
    // MOV.W r0, #imm = 0xF04F 0x0<imm3><Rd><imm8>, ici Rd = 0.
    let cas: [(u16, u16, u32); 4] = [
        (0xF04F, 0x00FF, 0x0000_00FF), // imm12[9:8] = 00
        (0xF04F, 0x10FF, 0x00FF_00FF), // 01
        (0xF04F, 0x20FF, 0xFF00_FF00), // 10
        (0xF04F, 0x30FF, 0xFFFF_FFFF), // 11
    ];
    for (w1, w2, attendu) in cas {
        Thumb32::execute(w1, w2, &mut regs, &mut bus, &mut periph, &mut nvic);
        assert_eq!(
            regs.get_reg(0),
            attendu,
            "l'immediat modifie {:#06x} doit valoir {:#010x}",
            w2,
            attendu
        );
    }

    // Consequence directe : CMP.W rX, #-1 doit voir un negatif comme negatif.
    // Le decodeur de sprites s'en sert pour distinguer une repetition d'une
    // suite litterale, et ne voyait que des repetitions.
    regs.set_reg(0, 0xFFFF_FFD1);
    Thumb32::execute(0xF1B0, 0x3FFF, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert!(regs.flag_n(), "0xFFFFFFD1 compare a -1 doit rester negatif");
    assert!(!regs.flag_z());

    regs.set_reg(0, 0x0000_0017);
    Thumb32::execute(0xF1B0, 0x3FFF, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert!(!regs.flag_n(), "0x17 compare a -1 doit rester positif");
}

#[test]
fn le_registre_de_configuration_de_la_flash_se_relit() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();
    let fc = periph::FLASH_CTL;

    // Au repos, le firmware attend 0x40 en 0x0000918C : Quad Enable pose et
    // aucune ecriture en cours. Son bit 0 est le temoin d'ecriture, scrute
    // apres chaque programmation.
    bus.write_u32(fc + 0x04, 1 << 14, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(fc + 0x14, &mut periph, &nvic), 0x40);

    // Sequence d'ecriture de 0x00005808 : la donnee en 0x10, puis l'ordre.
    bus.write_u32(fc + 0x10, 0x42, &mut periph, &mut nvic);
    bus.write_u32(fc + 0x04, 1 << 11, &mut periph, &mut nvic);
    bus.write_u32(fc + 0x04, 1 << 14, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(fc + 0x14, &mut periph, &nvic), 0x42);
}

#[test]
fn le_convertisseur_de_pile_rend_son_echantillon_et_enchaine() {
    use tamagotchi_paradise_rs::emulator::peripherals::adc_pile;
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();
    let adc = periph::WDT;

    // Sequence de depart relevee en 0x000051F6 a 0x000052D8.
    bus.write_u32(adc + adc_pile::CTRL, 0x8000, &mut periph, &mut nvic);
    bus.write_u32(adc + adc_pile::COMMANDE, 0x8000, &mut periph, &mut nvic);
    bus.write_u32(adc + adc_pile::COMMANDE, 0x8001, &mut periph, &mut nvic);

    // Le gestionnaire de l'IRQ 9 extrait l'echantillon par UBFX #6, #10.
    let brut = bus.read_u32(adc + adc_pile::RESULTAT, &mut periph, &nvic);
    assert_eq!(
        (brut >> adc_pile::DECALAGE_RESULTAT) & 0x3FF,
        adc_pile::PILE_PLEINE,
        "les dix bits utiles doivent commencer au rang 6"
    );
    assert!(periph.adc_pile.irq_a_lever, "le depart doit produire un premier echantillon");
    periph.adc_pile.irq_a_lever = false;

    // Le firmware ne relance jamais la conversion : elle doit s'enchainer.
    assert!(!periph.adc_pile.tick(1), "aucun echantillon avant la fin de la conversion");
    assert!(
        periph.adc_pile.tick(adc_pile::DUREE_CONVERSION as u32),
        "le convertisseur doit produire un echantillon a chaque duree ecoulee"
    );

    // Le bit de depart retombe, comme sur la puce.
    assert_eq!(
        bus.read_u32(adc + adc_pile::COMMANDE, &mut periph, &nvic) & adc_pile::DEPART,
        0
    );
}

#[test]
fn remplacer_la_pile_efface_le_drapeau_et_refait_la_somme() {
    // Earth et Land ont ete extraites pile faible, Water et Sky non.
    let path = std::path::Path::new(REAL_DUMP);
    if !path.exists() {
        return;
    }
    let mut machine = Machine::new();
    machine.device_key = Some(REAL_DEVICE_KEY);
    machine.load_firmware_file(path).unwrap();

    // Ce dump porte le drapeau : la console etait en fin de pile a l'extraction.
    let page = Machine::PAGES_SAUVEGARDE[0];
    assert_ne!(
        machine.bus.flash.read_u8(page + 4) & Machine::DRAPEAU_PILE_FAIBLE,
        0,
        "le dump de reference doit porter le drapeau de pile faible"
    );

    machine.remplacer_la_pile();
    for page in Machine::PAGES_SAUVEGARDE {
        assert_eq!(machine.bus.flash.read_u8(page + 4) & Machine::DRAPEAU_PILE_FAIBLE, 0);
        // L'en-tete doit rester d'accord avec le contenu, sinon le firmware
        // rejette la page et la reformate.
        let entete = u16::from_le_bytes([
            machine.bus.flash.read_u8(page),
            machine.bus.flash.read_u8(page + 1),
        ]);
        let complement = u16::from_le_bytes([
            machine.bus.flash.read_u8(page + 2),
            machine.bus.flash.read_u8(page + 3),
        ]);
        assert_eq!(entete, !complement, "en-tete de page coherent apres correction");
    }
}

#[test]
fn une_exception_prise_dans_un_bloc_it_ne_saute_pas_la_premiere_instruction() {
    let mut machine = Machine::new();
    let (p, n) = (&mut machine.periph, &mut machine.cpu.nvic);

    // Vecteur d'IRQ 0 vers un gestionnaire fictif, et pile utilisable.
    machine.bus.write_u32(0x40, 0x0000_2001, p, n);
    machine.cpu.regs.set_sp(map::SRAM_BASE + 0x1000);
    machine.cpu.regs.pc = 0x1000;

    // Le coeur est au milieu d'un bloc IT dont la condition est fausse : les
    // instructions restantes doivent etre sautees, mais seulement celles du
    // code interrompu, jamais celles du gestionnaire.
    machine.cpu.regs.itstate = 0x08; // condition EQ, une instruction restante
    machine.cpu.regs.set_flag_z(false);

    let sp_avant = machine.cpu.regs.get_sp();
    machine.cpu.nvic.iser[0] = 1;
    machine.cpu.nvic.request_irq(0);
    machine.step();

    assert_eq!(machine.cpu.regs.pc, 0x2000, "le gestionnaire doit demarrer");
    assert_eq!(machine.cpu.regs.itstate, 0, "le gestionnaire demarre hors bloc IT");
    assert_eq!(machine.cpu.regs.get_sp(), sp_avant - 32, "le contexte doit etre empile");

    // L'etat du bloc IT voyage dans le xPSR empile et revient au retour.
    let xpsr = machine.bus.read_u32(
        machine.cpu.regs.get_sp() + 28,
        &mut machine.periph,
        &machine.cpu.nvic,
    );
    assert_eq!((xpsr >> 25) & 0x3, 0, "les deux bits bas de l'etat IT");
    assert_eq!((xpsr >> 10) & 0x3F, 0x02, "les six bits hauts de l'etat IT");

    machine.cpu.regs.pc = 0xFFFF_FFF9;
    machine.step();
    assert_eq!(machine.cpu.regs.itstate, 0x08, "le bloc IT interrompu doit reprendre");
}

#[test]
fn un_appui_tire_la_broche_du_bouton_vers_le_bas() {
    let mut machine = Machine::new();

    // Les entrees sont a resistance de tirage : hautes au repos, basses sous
    // l'appui. Le firmware designe ses broches par port dans les bits hauts et
    // broche dans les quatre bits bas.
    let lire = |m: &Machine, id: u32| -> u32 {
        let port = match id >> 4 {
            0 => &m.periph.port0,
            1 => &m.periph.port1,
            _ => &m.periph.port2,
        };
        (port.read_reg(0) >> (id & 0xF)) & 1
    };

    for bouton in [
        Machine::BOUTON_MOLETTE,
        Machine::BOUTON_A,
        Machine::BOUTON_B,
        Machine::BOUTON_C,
        Machine::ENCODEUR_1,
        Machine::ENCODEUR_2,
    ] {
        assert_eq!(lire(&machine, bouton), 1, "broche {:#x} haute au repos", bouton);
        machine.appuyer(bouton);
        assert_eq!(lire(&machine, bouton), 0, "broche {:#x} basse sous l'appui", bouton);
        machine.relacher(bouton);
        assert_eq!(lire(&machine, bouton), 1, "broche {:#x} remonte au relachement", bouton);
    }

    // Un appui ne doit pas deborder sur les broches voisines.
    machine.appuyer(Machine::BOUTON_A);
    assert_eq!(lire(&machine, Machine::BOUTON_C), 1);
    assert_eq!(lire(&machine, Machine::BOUTON_B), 1);
}

#[test]
fn le_dma_flash_lit_et_ecrit_selon_son_bit_de_direction() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let mut nvic = Nvic::default();
    let fc = periph::FLASH_CTL;
    let tampon = map::SRAM_BASE + 0x200;

    for i in 0..32u32 {
        bus.flash.write_u8((0xEFE000 + i) as usize, 0xA0 + i as u8);
    }

    // Le registre de controle porte deux bits distincts : bit 0 le depart, bit 1
    // la direction. Le firmware procede par lecture-modification-ecriture, donc
    // il doit relire ce qu'il a ecrit, sinon le bit de direction se perd et
    // toute lecture passe pour une ecriture.
    bus.write_u32(fc + 0x10C, 0x60EF_E000, &mut periph, &mut nvic);
    bus.write_u32(fc + 0x104, 32, &mut periph, &mut nvic);
    bus.write_u32(fc + 0x100, tampon, &mut periph, &mut nvic);

    // Bit de direction pose : flash vers memoire.
    bus.write_u32(fc + 0x108, 2, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(fc + 0x108, &mut periph, &nvic), 2, "le bit de direction se relit");
    bus.write_u32(fc + 0x108, 3, &mut periph, &mut nvic);
    for i in 0..32u32 {
        assert_eq!(bus.read_u8(tampon + i, &mut periph, &nvic), 0xA0 + i as u8);
    }

    // Bit de direction a zero : memoire vers flash, ce qui permet de sauvegarder.
    for i in 0..32u32 {
        bus.write_u8(tampon + i, 0x50 + i as u8, &mut periph, &mut nvic);
    }
    bus.write_u32(fc + 0x108, 0, &mut periph, &mut nvic);
    assert_eq!(bus.read_u32(fc + 0x108, &mut periph, &nvic), 0, "le bit de direction se relit");
    bus.write_u32(fc + 0x108, 1, &mut periph, &mut nvic);
    for i in 0..32u32 {
        assert_eq!(
            bus.flash.read_u8((0xEFE000 + i) as usize),
            0x50 + i as u8,
            "octet {} remonte en flash",
            i
        );
    }
}

#[test]
fn les_entrees_du_port_2_sont_hautes_au_repos() {
    let mut bus = MemoryBus::default();
    let mut periph = Peripherals::default();
    let nvic = Nvic::default();

    // Le firmware lit les broches 0x20 et 0x21, soit le port 2 broches 0 et 1,
    // les combine en pin0 | (pin1 << 1) et attend la valeur 3. Des entrees a
    // resistance de tirage se lisent hautes au repos.
    let lire = |bus: &mut MemoryBus, p: &mut Peripherals, pin: u32| {
        bus.read_u32(0x4200_0000 + 0x1A000 * 32 + pin * 4, p, &nvic)
    };
    let p0 = lire(&mut bus, &mut periph, 0);
    let p1 = lire(&mut bus, &mut periph, 1);
    assert_eq!(p0 | (p1 << 1), 3, "les deux broches doivent etre au repos");

    // Un appui tire la broche vers le bas, et elle seule.
    periph.port2.appuyer(0);
    assert_eq!(lire(&mut bus, &mut periph, 0), 0);
    assert_eq!(lire(&mut bus, &mut periph, 1), 1);
    periph.port2.relacher(0);
    assert_eq!(lire(&mut bus, &mut periph, 0), 1);
}
