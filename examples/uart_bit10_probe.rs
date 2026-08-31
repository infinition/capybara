//! Sonde de mesure du bit 10 et du comportement de l'UART (0x4000B014).
//!
//! Releve le comportement du firmware selon les valeurs imposees sur 0x4000B014.

use capybara::emulator::Machine;
use std::collections::BTreeMap;

const SECONDE: f64 = 96_000_000.0;
const SCENE: u32 = 0x1800_1BF4;
const ETAT_MACHINE: u32 = 0x1800_1BFA;

fn lire16(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn forcer_scene(m: &mut Machine, scene: u16) {
    let o = (SCENE - 0x1800_0000) as usize;
    m.bus.sram.data[o] = (scene & 0xFF) as u8;
    m.bus.sram.data[o + 1] = (scene >> 8) as u8;
    let eo = (ETAT_MACHINE - 0x1800_0000) as usize;
    m.bus.sram.data[eo] &= !0x07;
}

fn tester_combinaison(path: &str, key: u32, force_val: u32) {
    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();
    m.remplacer_la_pile();

    // 1. Initialisation jusqu'au jeu actif (3 secondes)
    let fin_boot = m.cpu.cycles + (3.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_boot {
        m.step();
    }

    // 2. Forcer la scene 115 (menu de communication)
    println!("== Transition scene 115 ==");
    forcer_scene(&mut m, 115);
    let fin_scene115 = m.cpu.cycles + (1.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_scene115 {
        m.step();
    }

    // Valider 's amuser' par un appui B pour aller en scene 116
    m.appuyer(capybara::emulator::Machine::BOUTON_B);
    let fin_b1 = m.cpu.cycles + (0.2 * SECONDE) as u64;
    while m.cpu.cycles < fin_b1 {
        m.step();
    }
    m.relacher(capybara::emulator::Machine::BOUTON_B);

    // Laisser la scene 116 s'afficher
    let fin_scene116 = m.cpu.cycles + (1.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_scene116 {
        m.step();
    }

    // Second appui B pour declencher l'ouverture du lien serie
    println!("== Second appui bouton B pour ouvrir le lien serie ==");
    m.appuyer(capybara::emulator::Machine::BOUTON_B);
    let fin_b2 = m.cpu.cycles + (0.2 * SECONDE) as u64;
    while m.cpu.cycles < fin_b2 {
        m.step();
    }
    m.relacher(capybara::emulator::Machine::BOUTON_B);

    // 5. Imposer la valeur sur 0x4000B014
    m.bus.mmio_trace.forcees.insert(0x4000_B014, force_val);
    m.bus.mmio_trace.all.clear();
    m.bus.mmio_trace.enabled = true;

    let mut histogramme_pc: BTreeMap<u32, u64> = BTreeMap::new();
    let mut total_instructions = 0u64;

    // Laisser tourner 3 secondes console
    let fin_test = m.cpu.cycles + (3.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_test {
        let pc = m.cpu.regs.pc;
        *histogramme_pc.entry(pc).or_default() += 1;
        total_instructions += 1;
        m.step();
    }

    println!("==================================================");
    println!(
        "Mesure pour 0x4000B014 = {:#06x} (bit6={}, bit10={})",
        force_val,
        (force_val >> 6) & 1,
        (force_val >> 10) & 1
    );
    println!(
        "Scene finale : {}, total instructions : {}",
        lire16(&m, SCENE),
        total_instructions
    );

    // Top 8 des adresses executees
    let mut top_pc: Vec<(&u32, &u64)> = histogramme_pc.iter().collect();
    top_pc.sort_by(|a, b| b.1.cmp(a.1));
    println!("Top adresses PC executees :");
    for (&pc, &count) in top_pc.iter().take(8) {
        let pct = (count as f64 / total_instructions as f64) * 100.0;
        println!("  {:#010x} : {:>8} ({:.1}%)", pc, count, pct);
    }

    // Acces MMIO pendant la fenetre
    println!("Acces MMIO enregistres sur page 0x4000B000 (UART1) :");
    let mut any_uart = false;
    for (&adr, stat) in m.bus.mmio_trace.all.iter() {
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
        println!("  (aucun acces UART1 direct pendant cette tranche)");
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap_or_else(|| {
        "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin".to_string()
    });
    let key = 0x5AAF34FB;

    // Combinaisons a tester :
    // 1. 0x000 : Aucun bit (bit 6 = 0, bit 10 = 0)
    // 2. 0x040 : bit 6 seul (TEMT = 1, bit 10 = 0)
    // 3. 0x400 : bit 10 seul (bit 6 = 0, bit 10 = 1)
    // 4. 0x440 : bit 6 et bit 10
    // 5. 0x060 : bit 5 et bit 6 (THRE=1, TEMT=1, bit 10 = 0)
    // 6. 0x460 : bit 5, bit 6 et bit 10

    for &val in &[0x000, 0x040, 0x400, 0x440, 0x060, 0x460] {
        tester_combinaison(&path, key, val);
    }
}
