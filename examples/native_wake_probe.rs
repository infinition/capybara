//! Replay a captured native firmware sleep state, preserving loader boot info.
use capybara::emulator::{Machine, StepResult};
use capybara::emulator::etat::Instantane;
use std::collections::HashMap;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let state = Instantane::lire(std::path::Path::new(&args[1])).unwrap();
    let mut m = Machine::new();
    m.device_key = state.periph.fuses.device_key;
    let report = m.load_firmware_file(&state.firmware).unwrap();
    assert!(report.bootable, "native firmware must be decrypted");
    m.restaurer(&state);
    println!("before pc={:x} sleep={} frames={}", m.cpu.regs.pc, m.en_veille_profonde(),m.periph.display.trames);
    let press = std::env::var_os("NO_PRESS").is_none();
    let button = match std::env::var("PRESS").as_deref() {Ok("A")=>Machine::BOUTON_A, Ok("C")=>Machine::BOUTON_C, _=>Machine::BOUTON_B};
    if press {m.appuyer(button);}
    let press_end = m.cpu.cycles + 10_000_000;
    let budget: u64 = args.get(2).and_then(|s|s.parse().ok()).unwrap_or(20_000_000);
    let mut histogram = HashMap::<u32,u64>::new();
    for i in 0..budget {
        if press && m.cpu.cycles >= press_end {m.relacher(button);}
        if m.cpu.regs.pc == 0xAC50 {println!("panic={} lr={:x}",m.cpu.regs.r[0],m.cpu.regs.lr);break;}
        if i>budget/2 {*histogram.entry(m.cpu.regs.pc).or_default()+=1;}
        match m.step() {StepResult::Ok(_)=>{}, StepResult::Undefined(op)=>{println!("undefined {op:x} pc={:x}",m.cpu.regs.pc);break;}, _=>{println!("stopped pc={:x}",m.cpu.regs.pc);break;}}
    }
    println!("after pc={:x} sleep={} frames={} enabled={} sp={:x}",m.cpu.regs.pc,m.en_veille_profonde(),m.periph.display.trames,m.periph.display.is_enabled,m.cpu.regs.get_sp());
    let mut top:Vec<_>=histogram.into_iter().collect();top.sort_by_key(|(_,n)|std::cmp::Reverse(*n));println!("top={:x?}",top.iter().take(8).collect::<Vec<_>>());
    let mut picture=image::RgbImage::new(128,128);for(i,pixel)in m.periph.display.vram.iter().enumerate(){picture.put_pixel((i%128)as u32,(i/128)as u32,image::Rgb([(((pixel>>11)&31)*255/31)as u8,(((pixel>>5)&63)*255/63)as u8,((pixel&31)*255/31)as u8]));}picture.save("target/native-wake-probe.png").unwrap();
    m.instantane().ecrire(std::path::Path::new("target/native-wake-probe.tamastate")).unwrap();
}
