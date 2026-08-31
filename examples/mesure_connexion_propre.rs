//! Mesure complète sur l'état de connexion valide (20260829-003215.tamastate).

use capybara::emulator::etat::Instantane;
use capybara::emulator::Machine;
use std::collections::BTreeMap;

const SECONDE: f64 = 96_000_000.0;
const SCENE: u32 = 0x1800_1BF4;

fn main() {
    let snap_path = "C:\\Users\\infinition\\AppData\\Roaming\\TamagotchiParadise\\data\\sauvegardes\\Tamagotchi_Paradise_Jade_Forest-786fc58c\\reprises\\jadee\\20260829-182309.tamastate";
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Jade_Forest.BIN";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();

    let etat = Instantane::lire(std::path::Path::new(snap_path)).expect("lecture instantane");
    m.restaurer(&etat);

    let o = (SCENE - 0x1800_0000) as usize;
    let scene = m.bus.sram.data[o] as u16 | ((m.bus.sram.data[o + 1] as u16) << 8);
    println!("== Etat restaure : scene {} ==", scene);

    // Si la console est en veille (0x000023D0), ou attend B pour démarrer la communication
    println!("== Envoi premier appui Bouton B ==");
    m.appuyer(0x0B);
    for _ in 0..500_000 {
        m.step();
    }
    m.relacher(0x0B);

    m.bus.mmio_trace.all.clear();
    m.bus.mmio_trace.enabled = true;

    let mut histogramme_pc: BTreeMap<u32, u64> = BTreeMap::new();
    let mut total_instructions = 0u64;
    let mut callers_4ec0: BTreeMap<u32, u64> = BTreeMap::new();
    let mut echantillons_pile_4ec0: Vec<(u32, u32, [u32; 4], Vec<u32>)> = Vec::new();

    let fin_mesure = m.cpu.cycles + (5.0 * SECONDE) as u64;
    let mut appui_second_b = false;

    while m.cpu.cycles < fin_mesure {
        if !appui_second_b && m.cpu.cycles >= etat.cycles + (1.0 * SECONDE) as u64 {
            println!("== Envoi second appui Bouton B ==");
            m.appuyer(0x0B);
            for _ in 0..500_000 {
                m.step();
            }
            m.relacher(0x0B);
            appui_second_b = true;
        }

        let pc = m.cpu.regs.pc;
        *histogramme_pc.entry(pc).or_default() += 1;
        total_instructions += 1;

        if pc == 0x0000_4EC0 {
            let lr = m.cpu.regs.lr;
            *callers_4ec0.entry(lr).or_default() += 1;
            if echantillons_pile_4ec0.len() < 5 {
                let r = [
                    m.cpu.regs.r[0],
                    m.cpu.regs.r[1],
                    m.cpu.regs.r[2],
                    m.cpu.regs.r[3],
                ];
                let msp = m.cpu.regs.msp;
                let mut stack = Vec::new();
                for i in 0..8 {
                    let adr = msp.wrapping_add(i * 4);
                    if (0x1800_0000..0x1802_0000).contains(&adr) {
                        let so = (adr - 0x1800_0000) as usize;
                        let d = &m.bus.sram.data;
                        if so + 3 < d.len() {
                            let val = u32::from_le_bytes([d[so], d[so + 1], d[so + 2], d[so + 3]]);
                            stack.push(val);
                        }
                    }
                }
                echantillons_pile_4ec0.push((m.cpu.cycles as u32, lr, r, stack));
            }
        }

        m.step();
    }

    println!("\n==================================================");
    println!(
        "Total instructions exécutées en 3.0s : {}",
        total_instructions
    );

    println!("\nTop 15 adresses (PC) exécutées :");
    let mut top_pc: Vec<(&u32, &u64)> = histogramme_pc.iter().collect();
    top_pc.sort_by(|a, b| b.1.cmp(a.1));
    for (&pc, &count) in top_pc.iter().take(15) {
        let pct = (count as f64 / total_instructions as f64) * 100.0;
        println!("  {:#010x} : {:>10} ({:.2}%)", pc, count, pct);
    }

    println!("\nAccès MMIO sur page 0x4000B000 (UART1) :");
    for (adr, stat) in m.bus.mmio_trace.all.iter() {
        if (adr & !0xFFF) == 0x4000_B000 {
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

    println!("\nAppelants de 0x00004EC0 (lecture de 0x4000B014) :");
    for (lr, count) in callers_4ec0.iter() {
        println!("  LR: {:#010x} (appelé {} fois)", lr, count);
    }

    println!("\nÉchantillons de contexte lors de l'entrée en 0x00004EC0 :");
    for (i, (cyc, lr, r, st)) in echantillons_pile_4ec0.iter().enumerate() {
        println!("  #{} à cycle {} :", i + 1, cyc);
        println!("    LR = {:#010x}", lr);
        println!(
            "    r0={:#010x}, r1={:#010x}, r2={:#010x}, r3={:#010x}",
            r[0], r[1], r[2], r[3]
        );
        print!("    Pile (MSP) : ");
        for w in st {
            print!("{:#010x} ", w);
        }
        println!();
    }
}
