//! Idle fast-forward, keep-awake and recovery-point lock.

use capybara::emulator::etat::Instantane;
use capybara::emulator::reprises::Journal;
use capybara::emulator::Machine;

const RAM: u32 = 0x1801_0000;
const IDLE_COUNT: u32 = 0x1800_1BFE;
const CALLER: u32 = 0x200;

fn pram_u16(m: &mut Machine, a: u32, w: u16) {
    m.bus.pram.write_u8(a as usize, (w & 0xFF) as u8);
    m.bus.pram.write_u8(a as usize + 1, (w >> 8) as u8);
}

fn sram_u16(m: &Machine, a: u32) -> u16 {
    let i = (a - 0x1800_0000) as usize;
    u16::from_le_bytes([m.bus.sram.data[i], m.bus.sram.data[i + 1]])
}

fn machine() -> Machine {
    let mut m = Machine::new();
    pram_u16(&mut m, CALLER, 0xE7FE); // b .
    m.cpu.regs.msp = RAM + 0x800;
    m.cpu.regs.primask = 1;
    m
}

/// A polling loop (`ldr / cmp / beq`) on a RAM word that never changes.
fn polling_machine() -> Machine {
    let mut m = machine();
    pram_u16(&mut m, 0x300, 0x6808); // ldr r0, [r1]
    pram_u16(&mut m, 0x302, 0x2800); // cmp r0, #0
    pram_u16(&mut m, 0x304, 0xD0FC); // beq 0x300
    m.cpu.regs.pc = 0x300;
    m.cpu.regs.r[1] = RAM;
    m.cpu.regs.primask = 0;
    m
}

/// The fast-forward only skips time: after the same amount of console time the
/// registers are those of the step-exact core.
#[test]
fn fast_forward_matches_step_exact_core() {
    let mut a = polling_machine();
    let mut b = polling_machine();
    b.cpu.step_exact();
    assert!(a.cpu.repos_actif && !b.cpu.repos_actif);
    let target = 5_000_000u64;
    while a.cpu.cycles < target {
        a.step();
    }
    while b.cpu.cycles < target {
        b.step();
    }
    assert_eq!(b.cpu.sauts, 0, "step-exact core never skips");
    assert!(a.cpu.sauts > 0, "the polling loop is skipped");
    assert!((0x300..=0x304).contains(&a.cpu.regs.pc));
    assert!((0x300..=0x304).contains(&b.cpu.regs.pc));
    assert_eq!(a.cpu.regs.r[0], b.cpu.regs.r[0]);
    assert_eq!(a.cpu.regs.r[1], b.cpu.regs.r[1]);
}

/// A loop whose exit condition becomes true is left by the fast-forward.
#[test]
fn fast_forward_still_notices_a_change() {
    let mut m = polling_machine();
    while m.cpu.cycles < 1_000_000 {
        m.step();
    }
    let i = (RAM - 0x1800_0000) as usize;
    m.bus.sram.data[i] = 1;
    let mut n = 0;
    while !(m.cpu.regs.pc > 0x304 || m.cpu.regs.pc < 0x300) && n < 10_000_000 {
        m.step();
        n += 1;
    }
    assert!(m.cpu.regs.pc > 0x304, "left the loop (pc = {:#x})", m.cpu.regs.pc);
}

/// Keep-awake: the idle count is cleared once a second of console time.
#[test]
fn keep_awake_clears_the_idle_count() {
    let mut m = machine();
    m.cpu.regs.pc = CALLER;
    m.veille_interdite = true;
    m.compteur_inactivite = vec![(IDLE_COUNT, 2)];
    let i = (IDLE_COUNT - 0x1800_0000) as usize;
    m.bus.sram.data[i..i + 2].copy_from_slice(&500u16.to_le_bytes());
    let mut pas = 0u64;
    while m.cpu.cycles < 96_000_000 + 100_000 && pas < 50_000_000 {
        m.step();
        pas += 1;
    }
    assert_eq!(sram_u16(&m, IDLE_COUNT), 0, "count cleared after one second ({pas} steps)");
}

/// Recovery points: a locked point is kept in the on-disk index and restores.
#[test]
fn recovery_point_lock_survives_reopening_and_restores() {
    let mut m = machine();
    m.cpu.regs.pc = CALLER;
    let dir = std::env::temp_dir().join(format!("capybara_features_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut j = Journal::default();
    j.ouvrir(dir.clone());
    assert!(j.prendre_maintenant(&m));
    m.cpu.regs.r[0] = 0x1234_5678;
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert!(j.prendre_maintenant(&m));
    assert_eq!(j.points().len(), 2);
    assert!(j.basculer_le_verrou(1), "locked");
    j.attendre_les_ecritures();
    let mut j2 = Journal::default();
    j2.ouvrir(dir.clone());
    assert_eq!(j2.points().len(), 2);
    assert_eq!(j2.verrouilles(), 1, "lock read back from the index");
    assert!(j2.points()[1].verrouille);
    let etat: Instantane = j2.restaurer(1).expect("point readable");
    let mut m2 = machine();
    m2.restaurer(&etat);
    assert_eq!(m2.cpu.regs.r[0], 0x1234_5678);
    let _ = std::fs::remove_dir_all(&dir);
}
