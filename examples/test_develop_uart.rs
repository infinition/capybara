//! Teste directement la scene PSID_DEVELOP_UARTTEST (scene 16).

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

    // 1. Boot 3 secondes
    let fin_boot = m.cpu.cycles + (3.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_boot {
        m.step();
    }

    println!("== Forcage de la scene 16 (PSID_DEVELOP_UARTTEST) ==");
    forcer_scene(&mut m, 16);

    m.bus.mmio_trace.all.clear();
    m.bus.mmio_trace.enabled = true;

    let mut histogramme_pc: BTreeMap<u32, u64> = BTreeMap::new();
    let fin_test = m.cpu.cycles + (3.0 * SECONDE) as u64;
    let mut total_instructions = 0u64;

    while m.cpu.cycles < fin_test {
        let pc = m.cpu.regs.pc;
        *histogramme_pc.entry(pc).or_default() += 1;
        total_instructions += 1;
        m.step();
    }

    println!("==================================================");
    println!("Total instructions en scene 16 : {}", total_instructions);
    println!("Acces MMIO sur page 0x4000B000 (UART1) :");
    let mut any_uart = false;
    for (adr, stat) in m.bus.mmio_trace.all.iter() {
        if (adr & !0xFFF) == 0x4000_B000 {
            any_uart = true;
            println!(
                "  {:#010x} (+{:#04x}) : {} lectures, {} ecritures (derniere val ecrite: {:#x})",
                adr,
                adr & 0xFFF,
                stat.reads,
                stat.writes,
                stat.last_write
            );
        }
    }
    if !any_uart {
        println!("  (aucun acces UART1 direct)");
    }

    println!("\nTop 10 PC executes en scene 16 :");
    let mut top_pc: Vec<(&u32, &u64)> = histogramme_pc.iter().collect();
    top_pc.sort_by(|a, b| b.1.cmp(a.1));
    for (&pc, &count) in top_pc.iter().take(10) {
        let pct = (count as f64 / total_instructions as f64) * 100.0;
        println!("  {:#010x} : {:>10} ({:.2}%)", pc, count, pct);
    }
}
