//! Sonde d'horloge : cherche en memoire vive les compteurs qui avancent, et a
//! quel rythme.
//!
//! Usage : cargo run --release --example horloge_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [pas]
//!
//! Le firmware n'expose pas de calendrier sur sa page d'horloge : son temps est
//! tenu en logiciel. Le seul moyen de trouver ou, c'est de comparer la memoire
//! avant et apres une duree connue.

use capybara::emulator::etat::Instantane;
use capybara::emulator::{Machine, StepResult};

/// Cycles du coeur pour une seconde de temps console, le SysTick etant arme a
/// 95999 pour une milliseconde.
const SECONDE: u64 = 96_000_000;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let budget: u64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(5 * SECONDE);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));

    // L'ecran affiche une date : la retrouver en memoire donne la structure
    // d'horloge, et permet ensuite de la surveiller directement.
    if let Ok(v) = std::env::var("CHERCHE") {
        let cible: u32 = v.trim_start_matches("0x").parse().unwrap_or(2025);
        let d = &m.bus.sram.data;
        println!("== occurrences de {} en memoire vive", cible);
        for i in 0..d.len().saturating_sub(2) {
            let h = u16::from_le_bytes([d[i], d[i + 1]]) as u32;
            if h == cible {
                let suite: Vec<String> =
                    d[i..(i + 12).min(d.len())].iter().map(|o| format!("{:02x}", o)).collect();
                println!("  {:#010x}  {}", 0x1800_0000u32 + i as u32, suite.join(" "));
            }
        }
        println!();
    }

    // La PRAM porte le noyau du SDK, dont le compteur de millisecondes du
    // SysTick : la chercher en memoire vive seule laissait de cote la moitie
    // des candidats.
    let avant = m.bus.sram.data.clone();
    let avant_pram = m.bus.pram.data.clone();
    let mut pas = 0u64;
    while pas < budget {
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
    }
    let secondes = pas as f64 / SECONDE as f64;
    println!("== {} pas, soit {:.2} secondes de temps console", pas, secondes);

    // Un compteur de temps avance d'un petit nombre, proportionnel a la duree.
    // Tout le reste bouge de facon erratique ou pas du tout.
    let mut trouves = Vec::new();
    // Le plafond ecarte les pointeurs et les sommes de controle, qui sautent de
    // millions. Un compteur de temps, meme en millisecondes, reste sous
    // quelques milliers par seconde.
    let plafond = (4000.0 * secondes.max(1.0)) as u32;
    for (base, avant, apres) in [
        (0x1800_0000u32, &avant, &m.bus.sram.data),
        (0x0000_0000u32, &avant_pram, &m.bus.pram.data),
    ] {
        for i in (0..avant.len().min(apres.len()) - 4).step_by(4) {
            let a = u32::from_le_bytes([avant[i], avant[i + 1], avant[i + 2], avant[i + 3]]);
            let b = u32::from_le_bytes([apres[i], apres[i + 1], apres[i + 2], apres[i + 3]]);
            if b > a {
                let delta = b - a;
                if delta <= plafond {
                    trouves.push((base + i as u32, a, b, delta));
                }
            }
        }
    }
    trouves.sort_by_key(|&(_, _, _, d)| d);

    println!("\n== compteurs croissants, du plus lent au plus rapide");
    for &(adresse, a, b, delta) in trouves.iter().take(60) {
        let par_seconde = delta as f64 / secondes;
        println!(
            "  {:#010x}  {:>10} -> {:<10}  +{:<6}  {:.2} par seconde",
            adresse, a, b, delta, par_seconde
        );
    }
    println!("\n  {} compteurs au total", trouves.len());
}
