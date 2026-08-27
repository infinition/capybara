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
    // ADD r0, r0, r1 = 0xEA80 0x0001 (op=100).
    Thumb32::execute(0xEA80, 0x0001, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 0x2F);

    // SUB r0, r0, r1 = 0xEA40 0x0001 (op=010).
    Thumb32::execute(0xEA40, 0x0001, &mut regs, &mut bus, &mut periph, &mut nvic);
    assert_eq!(regs.get_reg(0), 0x20);
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
            periph::UART0,
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
    let mut machine = Machine::new();
    machine.device_key = None;
    std::env::remove_var("SONIX_DEVICE_KEY");

    let report = machine.load_firmware_file(path).unwrap();
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
