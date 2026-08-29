//! Sonde du tas : parcourt la chaine de blocs alloues et rend les trous libres.
//!
//! Usage : cargo run --release --example tas_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [secondes]
//!
//! L'allocateur du firmware (0x10016380) ne tient pas une liste de blocs
//! libres : il parcourt les blocs alloues et cherche un trou assez grand entre
//! deux voisins. Un bloc porte son suivant en tete et sa taille en +8. Quand
//! aucun trou ne convient il saute a l'assertion de 0x1005B4AC, qui boucle sur
//! place. Cette sonde dit s'il reste de la place, et ou elle est partie.

use capybara::emulator::etat::Instantane;
use capybara::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;

/// Descripteur du tas. Le champ +8 porte la tete de la chaine des blocs.
const DESCRIPTEUR: u32 = 0x1800_5D2C;

fn mot(m: &Machine, adr: u32) -> u32 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    let b = |i: usize| d.get(o + i).copied().unwrap_or(0) as u32;
    b(0) | (b(1) << 8) | (b(2) << 16) | (b(3) << 24)
}

/// Rend (nombre de blocs, total alloue, plus grand trou, total libre).
fn parcourir(m: &Machine, tete: u32, detaille: bool) -> (u32, u32, u32, u32) {
    let mut bloc = tete;
    let (mut blocs, mut alloue, mut plus_grand, mut libre) = (0u32, 0u32, 0u32, 0u32);
    while bloc != 0 && blocs < 4096 {
        let suivant = mot(m, bloc);
        let taille = mot(m, bloc + 8);
        blocs += 1;
        alloue += taille;
        if suivant == 0 {
            break;
        }
        let trou = suivant.saturating_sub(bloc).saturating_sub(taille);
        libre += trou;
        plus_grand = plus_grand.max(trou);
        if detaille {
            println!("    bloc {:#010x} taille {:>6}  trou de {:>6} avant {:#010x}", bloc, taille, trou, suivant);
        }
        bloc = suivant;
    }
    (blocs, alloue, plus_grand, libre)
}

fn rapport(m: &Machine, quand: &str, detaille: bool) {
    println!("== tas {}", quand);
    for champ in [0x8u32, 0xC] {
        let tete = mot(m, DESCRIPTEUR + champ);
        if tete == 0 {
            println!("  chaine +{:#x} : vide", champ);
            continue;
        }
        let (blocs, alloue, plus_grand, libre) = parcourir(m, tete, detaille);
        println!(
            "  chaine +{:#x} : {} blocs, {} octets alloues, {} octets libres, plus grand trou {} octets",
            champ, blocs, alloue, libre, plus_grand
        );
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let secondes: f64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(5.0);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));

    // RESET=1 rallume la console avec la flash de l'instantane, donc avec sa
    // sauvegarde. C'est le seul moyen de revoir l'entree en scene de jeu sans
    // rejouer toute la mise en route a la main.
    if std::env::var("RESET").is_ok() {
        m.reset();
        m.is_running = true;
        m.console.clear();
        println!("== console rallumee sur la flash de l'instantane");
    }

    println!("== descripteur du tas");
    for i in 0..8u32 {
        println!("  {:#010x} = {:#010x}", DESCRIPTEUR + i * 4, mot(&m, DESCRIPTEUR + i * 4));
    }
    println!();
    rapport(&m, "au depart", std::env::var("DETAIL").is_ok());

    let budget = (secondes * SECONDE) as u64;
    let mut pas = 0u64;
    let mut prochain = (SECONDE / 2.0) as u64;
    while pas < budget {
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
        if pas >= prochain {
            prochain += (SECONDE / 2.0) as u64;
            let tete = mot(&m, DESCRIPTEUR + 8);
            let (blocs, alloue, plus_grand, libre) = parcourir(&m, tete, false);
            println!(
                "  a {:>7.1} s : compteur {:>4} s, {:>4} blocs, {:>6} alloues, {:>6} libres, plus grand trou {:>6}",
                pas as f64 / SECONDE,
                m.periph.snsys.secondes,
                blocs,
                alloue,
                libre,
                plus_grand
            );
        }
    }

    println!();
    rapport(&m, &format!("apres {:.1} secondes", pas as f64 / SECONDE), std::env::var("DETAIL").is_ok());
}
