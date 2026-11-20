//! Run the installed Paramix launcher without booting or modifying a save.
//! Usage: paramix_probe <dump.bin> [instruction budget]
use capybara::emulator::{Machine, StepResult};
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).unwrap();
    let budget: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30_000_000);
    let word = |off| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
    let stub = (0x90000..0x110000).step_by(0x1000).find(|&off| {
        let address = word(off + 0x28);
        if !(0x10000000..0x11000000).contains(&address) { return false; }
        let menu = (address - 0x10000000 + 0x11000) as usize;
        bytes.get(menu + 0x140..menu + 0x144) == Some(b"FELD")
    }).expect("no installed Paramix launcher");
    let mut m = Machine::new();
    m.bus.flash.load_binary(0, &bytes);
    m.bus.flash.figer_reference();
    m.periph.xip.base = 0x60011000;
    m.periph.xip.ctrl = 3;
    m.cpu.regs.pc = 0x10000000 + stub as u32 - 0x11000 + 0x20;
    m.cpu.regs.msp = 0x1801F000;
    if let Ok(path) = std::env::var("STATE") {
        m.restaurer(&capybara::emulator::etat::Instantane::lire(std::path::Path::new(&path)).unwrap());
    }
    if let Ok(pc) = std::env::var("PC") {
        m.cpu.regs.pc = u32::from_str_radix(&pc, 16).unwrap();
        m.cpu.regs.msp = 0x1801F000;
    }
    if std::env::var_os("RETURN").is_some() {
        m.cpu.regs.lr = 0xF001;
        m.bus.pram.data[0xF000..0xF002].copy_from_slice(&0xBE00u16.to_le_bytes());
    }
    let output = std::env::var("OUT").unwrap_or_else(|_| "target/paramix-probe".into());
    let button = std::env::var("PRESS").ok().map(|key| match key.as_str() {
        "A" => Machine::BOUTON_A, "B" => Machine::BOUTON_B,
        "C" => Machine::BOUTON_C, _ => panic!("PRESS must be A, B or C"),
    });
    let mut histogram = HashMap::<u32, u64>::new();
    let mut resets = 0;
    let trap = std::env::var("TRAP").ok().and_then(|s| u32::from_str_radix(&s, 16).ok());
    let mut trail = std::collections::VecDeque::new();
    for i in 0..budget {
        let pc = m.cpu.regs.pc;
        if trap.is_some() {
            if Some(pc) == trap {
                println!("trap {pc:x} lr={:x} trail={trail:x?}", m.cpu.regs.lr);
                break;
            }
            trail.push_back(pc);
            if trail.len() > 32 { trail.pop_front(); }
        }
        if i > budget / 2 { *histogram.entry(pc).or_default() += 1; }
        if m.cpu.nvic.reset_requested { resets += 1; }
        if let Some(button) = button {
            if i == budget * 2 / 3 { m.appuyer(button); }
            if i == budget * 3 / 4 { m.relacher(button); }
        }
        match m.step() {
            StepResult::Ok(_) => {},
            StepResult::Undefined(op) => { println!("undefined {op:04x} at {pc:08x}"); break; },
            _ => { println!("stopped at {pc:08x}"); break; },
        }
    }
    println!("resets={resets} pc={:08x} frames={} spi_bytes={}", m.cpu.regs.pc, m.periph.display.trames, m.periph.spi0.total_octets_emis);
    println!("registers={:x?}", m.cpu.regs.r);
    println!("pram config={:x?}", &m.bus.pram.data[0x140..0x168]);
    m.instantane().ecrire(std::path::Path::new(&format!("{output}.tamastate"))).unwrap();
    let mut picture = image::RgbImage::new(128, 128);
    for (i, pixel) in m.periph.display.vram.iter().enumerate() {
        picture.put_pixel((i % 128) as u32, (i / 128) as u32, image::Rgb([
            (((pixel >> 11) & 31) * 255 / 31) as u8,
            (((pixel >> 5) & 63) * 255 / 63) as u8,
            ((pixel & 31) * 255 / 31) as u8,
        ]));
    }
    picture.save(format!("{output}.png")).unwrap();
    let mut top: Vec<_> = histogram.into_iter().collect();
    top.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    for (pc, count) in top.into_iter().take(12) {
        let instructions = m.get_disassembly_at(pc, 1);
        println!("{pc:08x}: {count} {} {}", instructions[0].mnemonic, instructions[0].operands);
    }
}
