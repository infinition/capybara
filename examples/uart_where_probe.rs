//! Sonde de localisation exacte des lectures de 0x4000B014 et histogramme PC.

use capybara::emulator::Machine;
use std::collections::BTreeMap;

const SECONDE: f64 = 96_000_000.0;
const SCENE: u32 = 0x1800_1BF4;
const ETAT_MACHINE: u32 = 0x1800_1BFA;

fn forcer_scene(m: &mut Machine, scene: u16) {
    let o = (SCENE - 0x1800_0000) as usize;
    m.bus.sram.data[o] = (scene & 0xFF) as u8;
    m.bus.sram.data[o + 1] = (scene >> 8) as u8;
    let eo = (ETAT_MACHINE - 0x1800_0000) as usize;
    m.bus.sram.data[eo] &= !0x07;
}

fn main() {
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();
    m.remplacer_la_pile();

    // 1. Boot 3s
    let fin_boot = m.cpu.cycles + (3.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_boot {
        m.step();
    }

    // 2. Scene 112 (TAMASPACE_TUSHIN)
    forcer_scene(&mut m, 112);
    let fin_s112 = m.cpu.cycles + (1.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_s112 {
        m.step();
    }

    // 3. Scene 113 (TAMASPACE_PLAY)
    forcer_scene(&mut m, 113);
    let fin_s113 = m.cpu.cycles + (1.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_s113 {
        m.step();
    }

    // 4. Appui bouton B
    m.appuyer(capybara::emulator::Machine::BOUTON_B);
    let fin_b = m.cpu.cycles + (0.3 * SECONDE) as u64;
    while m.cpu.cycles < fin_b {
        m.step();
    }
    m.relacher(capybara::emulator::Machine::BOUTON_B);

    // 5. Mesure pendant 5 secondes
    let mut lecteurs_ls: BTreeMap<u32, u64> = BTreeMap::new();
    let mut histogramme_pc: BTreeMap<u32, u64> = BTreeMap::new();
    let mut total_cycles = 0u64;

    let fin_mesure = m.cpu.cycles + (5.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_mesure {
        let pc = m.cpu.regs.pc;
        *histogramme_pc.entry(pc).or_default() += 1;
        total_cycles += 1;
        m.step();
    }

    println!("==================================================");
    println!("Total instructions executees en 5s : {}", total_cycles);
    println!("Lectures enregistrees de 0x4000B014 (par PC) :");
    for (adr, stat) in m.bus.mmio_trace.all.iter() {
        if *adr == 0x4000_B014 {
            println!(
                "  0x4000B014 : {} lectures, first_pc = {:#010x}",
                stat.reads, stat.first_pc
            );
        }
    }

    println!("\nTop 15 adresses PC executees (ou passe le temps) :");
    let mut top_pc: Vec<(&u32, &u64)> = histogramme_pc.iter().collect();
    top_pc.sort_by(|a, b| b.1.cmp(a.1));
    for (&pc, &count) in top_pc.iter().take(15) {
        let pct = (count as f64 / total_cycles as f64) * 100.0;
        println!("  {:#010x} : {:>10} ({:.2}%)", pc, count, pct);
    }
}
