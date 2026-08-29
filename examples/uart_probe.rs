//! Cherche le peripherique serie en forcant la console dans une scene qui s'en
//! sert.
//!
//! Usage : cargo run --release --example uart_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [numero de scene]
//!
//! Le lien serie ne s'ouvre pas en jouant : il faut atteindre le menu de
//! connexion. Plutot que d'y naviguer a l'aveugle, on ecrit le numero de la
//! scene voulue dans la demande de transition, en `0x18001BF6`, et le firmware
//! y va tout seul.
//!
//! Les numeros viennent de la table des scenes du firmware, un tableau de
//! descripteurs de vingt huit octets portant quatre gestionnaires, un nom et un
//! numero :
//!
//! ```text
//!    1  PSID_DEBUGMENU            17  PSID_DEVELOP_UARTTEST
//!    2  PSID_DEVELOP_COMMONCTRL   18  PSID_DEVELOP_UARTAGEING
//!   24  PSID_DEVELOP_TCP          19  PSID_DEVELOP_UARTBYTE
//!  113  PSID_TAMASPACE_TUSHIN     20  PSID_DEVELOP_UARTHEADER
//!  125  PSID_TESTMODE             29  PSID_HOME
//! ```
//!
//! La sonde releve les pages materielles touchees avant, puis apres, et ne
//! rapporte que les nouvelles : ce sont celles que la scene a reveillees.

use std::collections::BTreeSet;

use tamagotchi_paradise_rs::emulator::etat::Instantane;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;
const SCENE: u32 = 0x1800_1BF4;
const TRANSITION: u32 = 0x1800_1BF6;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let cible: u16 = a.next().and_then(|v| v.parse().ok()).unwrap_or(17);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));
    m.bus.mmio_trace.enabled = true;

    // Deux secondes de jeu ordinaire : c'est la reference.
    avancer(&mut m, 2.0);
    let avant = pages(&m);
    println!("== avant : scene {}, {} pages touchees", lire16(&m, SCENE), avant.len());

    // La transition demandee. Le firmware la prend a sa prochaine boucle.
    ecrire16(&mut m, TRANSITION, cible);
    println!("== transition demandee vers la scene {}\n", cible);

    for etape in 1..=6 {
        avancer(&mut m, 1.0);
        let scene = lire16(&m, SCENE);
        let apres = pages(&m);
        let neuves: Vec<u32> = apres.difference(&avant).copied().collect();
        println!(
            "  a {} s : scene {}, {} pages, {} nouvelles",
            etape,
            scene,
            apres.len(),
            neuves.len()
        );
        for page in &neuves {
            println!(
                "      {:#010x}  {}",
                page,
                tamagotchi_paradise_rs::emulator::mmu::periph::name_of(*page)
            );
        }
        if scene == cible {
            println!("      (scene atteinte)");
        }
    }

    println!("\n== registres les plus touches, hors des pages connues avant");
    let mut v: Vec<_> = m
        .bus
        .mmio_trace
        .all
        .iter()
        .filter(|(a, _)| !avant.contains(&(**a & !0xFFF)))
        .map(|(a, s)| (*a, *s))
        .collect();
    v.sort_by_key(|(_, s)| std::cmp::Reverse(s.reads + s.writes));
    for (adresse, s) in v.iter().take(30) {
        println!(
            "  {:#010x}  {:<10} lectures {:>8}  ecritures {:>8}  derniere {:#010x}  premier PC {:#010x}",
            adresse,
            tamagotchi_paradise_rs::emulator::mmu::periph::name_of(adresse & !0xFFF),
            s.reads,
            s.writes,
            s.last_write,
            s.first_pc
        );
    }
}

fn avancer(m: &mut Machine, secondes: f64) {
    let fin = m.cpu.cycles + (secondes * SECONDE) as u64;
    while m.cpu.cycles < fin {
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
    }
}

fn pages(m: &Machine) -> BTreeSet<u32> {
    m.bus.mmio_trace.all.keys().map(|a| a & !0xFFF).collect()
}

fn lire16(m: &Machine, adresse: u32) -> u16 {
    let o = (adresse - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    if o + 2 > d.len() {
        return 0;
    }
    u16::from_le_bytes([d[o], d[o + 1]])
}

fn ecrire16(m: &mut Machine, adresse: u32, valeur: u16) {
    let o = (adresse - 0x1800_0000) as usize;
    let d = &mut m.bus.sram.data;
    if o + 2 <= d.len() {
        d[o..o + 2].copy_from_slice(&valeur.to_le_bytes());
    }
}
