//! Mesure la vitesse brute du coeur, avec et sans journal des acces.
//!
//! Usage : cargo run --release --example bench_probe -- <dump.bin> <cle hex> [pas]

use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let budget: u64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(100_000_000);

    for journal in [true, false] {
        let mut m = Machine::new();
        m.device_key = Some(key);
        m.load_firmware_file(&path).unwrap();
        m.remplacer_la_pile();
        m.bus.mmio_trace.enabled = journal;

        let debut = std::time::Instant::now();
        let mut pas = 0u64;
        while pas < budget {
            if !matches!(m.step(), StepResult::Ok(_)) {
                break;
            }
            pas += 1;
        }
        let duree = debut.elapsed().as_secs_f64();
        println!(
            "  journal {:<5} : {} pas en {:.2} s, soit {:.1} millions de pas par seconde",
            journal,
            pas,
            duree,
            pas as f64 / duree / 1e6
        );
    }
}
