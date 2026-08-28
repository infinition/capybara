//! Sonde d'interruptions : releve les entrees et les sorties d'exception, et
//! designe celle qui ne revient jamais.
//!
//! Usage : cargo run --release --example irq_probe -- <dump.bin> <cle hex> [budget]

use tamagotchi_paradise_rs::emulator::cpu::registers::Mode;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let budget: u64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(400_000_000);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    if std::env::var("PILE_USEE").is_err() {
        m.remplacer_la_pile();
    }

    let mut pas = 0u64;
    let mut en_handler = false;
    let mut entree = (0u64, 0u32);
    let mut entrees = 0u64;
    let mut sorties = 0u64;
    // Duree de chaque sejour en mode Handler, pour reperer celui qui n'en sort pas.
    let mut plus_long = (0u64, 0u32, 0u64);

    while pas < budget {
        let avant = matches!(m.cpu.regs.mode, Mode::Handler);
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
        let apres = matches!(m.cpu.regs.mode, Mode::Handler);
        if !avant && apres {
            entrees += 1;
            entree = (pas, m.cpu.regs.pc);
            en_handler = true;
        } else if avant && !apres {
            sorties += 1;
            let duree = pas - entree.0;
            if duree > plus_long.2 {
                plus_long = (entree.0, entree.1, duree);
            }
            en_handler = false;
        }
    }

    println!("== exceptions sur {} pas", pas);
    println!("  entrees {}  sorties {}", entrees, sorties);
    println!(
        "  sejour le plus long : entree a {} pas, gestionnaire {:#010x}, {} pas",
        plus_long.0, plus_long.1, plus_long.2
    );
    if en_handler {
        println!(
            "  BLOQUEE : entree a {} pas dans le gestionnaire {:#010x}, jamais sortie",
            entree.0, entree.1
        );
        println!(
            "  PC a l'arret {:#010x}  r0 {:#x}  LR {:#010x}  SP {:#010x}",
            m.cpu.regs.pc,
            m.cpu.regs.get_reg(0),
            m.cpu.regs.lr,
            m.cpu.regs.get_sp()
        );
        // Le TE doit continuer de battre meme pendant un gestionnaire : si la
        // broche est figee, l'attente qui s'y trouve ne peut pas se terminer.
        let p1 = &m.periph.port1;
        println!(
            "  port 1 : entrees {:#010x}  direction {:#010x}  TE {}",
            p1.entrees,
            p1.direction,
            (p1.read_reg(0) >> 10) & 1
        );
        // Ce que l'attente lit vraiment, tour par tour, contre l'etat de la
        // broche au meme instant.
        let mut lectures = std::collections::BTreeSet::new();
        let mut broche = std::collections::BTreeSet::new();
        let mut tours = 0u32;
        for _ in 0..4_000_000u64 {
            if m.cpu.regs.pc == 0x1006_A0B4 {
                lectures.insert(m.cpu.regs.get_reg(0));
                broche.insert((m.periph.port1.read_reg(0) >> 10) & 1);
                tours += 1;
            }
            if !matches!(m.step(), StepResult::Ok(_)) {
                break;
            }
        }
        println!(
            "  {} tours d'attente : valeurs lues {:?}, etat reel de la broche {:?}",
            tours, lectures, broche
        );
    } else {
        println!("  aucune exception en cours a la fin");
    }
}
