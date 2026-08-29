//! Sonde de sauvegarde persistante : ecrit un emplacement depuis un instantane,
//! puis rallume la console dessus.
//!
//! Usage :
//!   cargo run --release --example partie_probe -- <dump.bin> <cle hex> <nom> [secondes]
//!
//! `DEPUIS_ETAT=chemin.tamastate` remplit d'abord l'emplacement avec les pages
//! de flash de cet instantane, ce qui revient a dire "voici une partie deja
//! jouee". Sans lui, la sonde se contente d'ouvrir l'emplacement et de faire
//! demarrer la console dessus, ce que fait l'interface au lancement.

use capybara::emulator::etat::Instantane;
use capybara::emulator::sauvegarde::{self, Sauvegarde};
use capybara::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;

fn demi(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn horloge(m: &Machine) -> String {
    let o = (0x1800_1BA4u32 - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    let champ = |i: usize| {
        let p = o + i * 2;
        d.get(p).copied().unwrap_or(0) as u32 | ((d.get(p + 1).copied().unwrap_or(0) as u32) << 8)
    };
    format!(
        "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
        champ(0),
        champ(1),
        champ(2),
        champ(3),
        champ(4),
        champ(5)
    )
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let nom = a.next().expect("nom de l'emplacement");
    let secondes: f64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(20.0);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    let empreinte = m.empreinte.clone().expect("empreinte du dump");
    println!("== empreinte du dump : {}", empreinte);
    println!("== dossier : {}", sauvegarde::dossier_du_dump(&empreinte).display());

    if let Ok(chemin) = std::env::var("DEPUIS_ETAT") {
        let etat = Instantane::lire(std::path::Path::new(&chemin)).expect("lecture de l'etat");
        m.restaurer(&etat);
        let partie = Sauvegarde::depuis(&m);
        let cible = sauvegarde::chemin(&empreinte, &nom);
        partie.ecrire(&cible).expect("ecriture de la sauvegarde");
        println!("== {} pages ecrites dans {}", partie.pages.len(), cible.display());
        // On repart d'une machine propre pour eprouver la relecture.
        m = Machine::new();
        m.device_key = Some(key);
        m.load_firmware_file(&path).unwrap();
    }

    println!("== emplacements connus : {:?}", sauvegarde::emplacements(&empreinte));

    let chemin = sauvegarde::chemin(&empreinte, &nom);
    match m.ouvrir_sauvegarde(chemin) {
        Ok(true) => println!("== partie « {} » chargee", nom),
        Ok(false) => println!("== partie « {} » neuve", nom),
        Err(e) => {
            println!("== sauvegarde illisible : {}", e);
            return;
        }
    }
    m.reset();
    m.is_running = true;

    let budget = (secondes * SECONDE) as u64;
    let mut pas = 0u64;
    let mut scene = demi(&m, 0x1800_1BF4);
    println!("== demarrage sur cette partie");
    while pas < budget {
        if !matches!(m.step(), StepResult::Ok(_)) {
            println!("  arret a PC={:#010x}", m.cpu.regs.pc);
            break;
        }
        pas += 1;
        let maintenant = demi(&m, 0x1800_1BF4);
        if maintenant != scene {
            println!(
                "  a {:>5.1} s : scene {} -> {},  horloge {}",
                pas as f64 / SECONDE,
                scene,
                maintenant,
                horloge(&m)
            );
            scene = maintenant;
        }
    }
    println!("== fin : scene {}, horloge {}", scene, horloge(&m));
}
