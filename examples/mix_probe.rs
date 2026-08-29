//! Temporaire : distribution des instructions executees, pour guider le decodage.
use capybara::emulator::etat::Instantane;
use capybara::emulator::{Machine, StepResult};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let key = u32::from_str_radix(a.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().unwrap();
    let pas: u64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(50_000_000);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).unwrap());
    m.bus.mmio_trace.enabled = false;

    let mut nib = [0u64; 16];
    let mut longues = 0u64;
    let mut total = 0u64;
    let mut zones = std::collections::BTreeMap::<u32, u64>::new();
    let mut hauts = std::collections::BTreeMap::<u16, u64>::new();
    for _ in 0..pas {
        let pc = m.cpu.regs.pc;
        let w = m.bus.read_u16(pc, &mut m.periph, &m.cpu.nvic);
        nib[(w >> 12) as usize] += 1;
        let l = (w & 0xF800) == 0xE800 || (w & 0xF800) == 0xF000 || (w & 0xF800) == 0xF800;
        if l { longues += 1; *hauts.entry(w >> 8).or_default() += 1; }
        *zones.entry(pc & 0xFF00_0000).or_default() += 1;
        total += 1;
        if !matches!(m.step(), StepResult::Ok(_)) { break; }
    }
    println!("total {total}, longues {longues} ({:.1} %)", longues as f64 * 100.0 / total as f64);
    for (i, c) in nib.iter().enumerate() {
        println!("  nibble {i:X} : {:>10}  {:>5.2} %", c, *c as f64 * 100.0 / total as f64);
    }
    for (z, c) in zones { println!("  zone {z:08X} : {:>5.2} %", c as f64 * 100.0 / total as f64); }
    let mut v: Vec<_> = hauts.into_iter().collect();
    v.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    for (h, c) in v { println!("  w1 haut {h:02X} : {:>5.2} %", c as f64 * 100.0 / total as f64); }
}
