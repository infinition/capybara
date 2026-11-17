//! Replay button presses without modifying the source snapshot or save.
use capybara::emulator::{etat::Instantane, Machine, StepResult};

fn main() {
    let path = std::env::args().nth(1).expect("snapshot.tamastate");
    let state = Instantane::lire(std::path::Path::new(&path)).unwrap();
    let mut machine = Machine::new();
    machine.device_key = state.periph.fuses.device_key;
    let report = machine.load_firmware_file(&state.firmware).unwrap();
    assert!(report.bootable);
    machine.restaurer(&state);
    println!("vectors match loader: {}", machine.bus.pram.read_u32(0) == report.entry_sp
        && machine.bus.pram.read_u32(4) == report.entry_pc);
    let start = machine.cpu.cycles;
    let mut previous = (false, 0u32, 0u32);
    let mut audible = 0;
    let mut held = None;
    for i in 0..400_000_000u64 {
        let elapsed = machine.cpu.cycles - start;
        if elapsed >= 576_000_000 { break; }
        let button = [Machine::BOUTON_A, Machine::BOUTON_B, Machine::BOUTON_C][(elapsed / 96_000_000 % 3) as usize];
        let next = (elapsed % 96_000_000 < 15_000_000).then_some(button);
        if held != next {
            if let Some(button) = held { machine.relacher(button); }
            if let Some(button) = next { machine.appuyer(button); }
            held = next;
        }
        if !matches!(machine.step(), StepResult::Ok(_)) {
            println!("stopped at pc={:x} after {i} instructions", machine.cpu.regs.pc);
            break;
        }
        if i % 10000 != 0 { continue; }
        machine.localiser_les_voix();
        let pwm = machine.periph.timers.buzzer_frequency();
        let note = machine.note_courante();
        let current = (machine.son_en_cours(), pwm as u32, note as u32);
        if note > 0.0 { audible += 1; }
        if current != previous {
            println!("cycle={} active={} pwm={pwm:.1} note={note:.1} regs={:x?}",
                machine.cpu.cycles - start, current.0, machine.periph.timers.buzzer_regs);
            previous = current;
        }
    }
    println!("audible samples={audible} elapsed cycles={}", machine.cpu.cycles - start);
}
