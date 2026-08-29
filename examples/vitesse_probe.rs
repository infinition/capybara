//! Mesure combien de secondes de console passent par seconde reelle.
//!
//! Usage : cargo run --release --example vitesse_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [secondes reelles]
//!
//! C'est le seul chiffre qui dit si l'emulation peut tenir le temps reel. Une
//! seconde de console vaut 96 millions de cycles, ce que le firmware declare en
//! armant son SysTick a 95999 pour une milliseconde.

use capybara::emulator::etat::Instantane;
use capybara::emulator::peripherals::snsys::CYCLES_PAR_SECONDE;
use capybara::emulator::{Machine, StepResult};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let reelles: f64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(5.0);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));

    // Deux boucles, et il faut regarder les deux. `step` mesure le coeur seul.
    // `run_frame` est ce que l'interface appelle vraiment : elle y ajoute le
    // test des points d'arret et l'interception de la console de debug, et
    // c'est donc elle qui dit ce que l'utilisateur verra.
    for par_trame in [false, true] {
        m.bus.mmio_trace.enabled = false;
        m.is_running = true;
        m.bus.mmio_trace.clear();
        let cycles_debut = m.cpu.cycles;
        let debut = std::time::Instant::now();
        while debut.elapsed().as_secs_f64() < reelles {
            if par_trame {
                if !matches!(m.run_frame(), StepResult::Ok(_)) {
                    break;
                }
            } else {
                for _ in 0..20_000 {
                    if !matches!(m.step(), StepResult::Ok(_)) {
                        break;
                    }
                }
            }
        }
        let ecoule = debut.elapsed().as_secs_f64();
        let cycles = (m.cpu.cycles - cycles_debut) as f64;
        println!(
            "  {:<10} : {:.2} millions de cycles par seconde, soit {:.2} fois le temps reel",
            if par_trame { "run_frame" } else { "step" },
            cycles / ecoule / 1e6,
            cycles / ecoule / CYCLES_PAR_SECONDE as f64
        );
    }

    for journal in [true, false] {
        m.bus.mmio_trace.enabled = journal;
        m.bus.mmio_trace.clear();
        let cycles_debut = m.cpu.cycles;
        let secondes_debut = m.periph.snsys.secondes;
        let trames_debut = m.periph.display.trames;
        let debut = std::time::Instant::now();
        while debut.elapsed().as_secs_f64() < reelles {
            for _ in 0..20_000 {
                if !matches!(m.step(), StepResult::Ok(_)) {
                    break;
                }
            }
        }
        let ecoule = debut.elapsed().as_secs_f64();
        let cycles = (m.cpu.cycles - cycles_debut) as f64;
        println!(
            "  journal {:<5} : {:.2} millions de cycles par seconde, soit {:.2} fois le temps reel",
            journal,
            cycles / ecoule / 1e6,
            cycles / ecoule / CYCLES_PAR_SECONDE as f64
        );
        println!(
            "                  horloge de la console +{} s, {} trames poussees en {:.1} s reelles",
            m.periph.snsys.secondes - secondes_debut,
            m.periph.display.trames - trames_debut,
            ecoule
        );
    }
}
