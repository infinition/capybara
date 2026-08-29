//! Sonde d'allocations : compte les prises et les rendus de memoire, par
//! appelant et par taille.
//!
//! Usage : cargo run --release --example alloc_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [secondes]
//!
//! L'allocateur du firmware entre en 0x10016358 et la liberation en
//! 0x100162F0. Un appelant qui prend plus qu'il ne rend est une fuite ; c'est
//! le seul moyen de savoir pourquoi la scene de jeu remplit un tas de 32 Ko au
//! point qu'une sauvegarde de 4 Ko n'y tient plus.

use std::collections::BTreeMap;

use capybara::emulator::etat::Instantane;
use capybara::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;

/// Entree de l'allocateur : r0 porte la taille, r1 le choix d'extremite.
const ALLOUER: u32 = 0x1001_6358;
/// Entree de la liberation : r0 porte le bloc.
const LIBERER: u32 = 0x1001_62F0;
/// Scene courante, sur deux octets.
const SCENE: u32 = 0x1800_1BF4;

fn demi(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let secondes: f64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(20.0);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));

    // RESET=1 rallume la console avec la flash de l'instantane, donc avec sa
    // sauvegarde. C'est le seul moyen de revoir l'entree en scene de jeu, et
    // donc les prises de memoire qui la remplissent, sans rejouer toute la mise
    // en route a la main.
    if std::env::var("RESET").is_ok() {
        m.reset();
        m.is_running = true;
        m.console.clear();
        println!("== console rallumee sur la flash de l'instantane");
    }

    let mut appuis: Vec<(u64, u32, u64)> = std::env::var("ENTREES")
        .ok()
        .map(|v| {
            v.split(',')
                .filter_map(|e| {
                    let c: Vec<&str> = e.split(':').collect();
                    if c.len() != 3 {
                        return None;
                    }
                    let quand: f64 = c[0].parse().ok()?;
                    let duree: f64 = c[2].parse().ok()?;
                    Some((
                        (quand * SECONDE) as u64,
                        u32::from_str_radix(c[1].trim_start_matches("0x"), 16).ok()?,
                        (duree * SECONDE) as u64,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    appuis.sort_by_key(|a| a.0);
    let mut relachements: Vec<(u64, u32)> = Vec::new();

    // (appelant, taille) -> nombre de prises. Les liberations sont comptees a
    // part, le bloc rendu ne portant pas sa taille dans les registres.
    let mut prises: BTreeMap<(u32, u32), u64> = BTreeMap::new();
    let mut rendus: BTreeMap<u32, u64> = BTreeMap::new();
    let mut scene = demi(&m, SCENE);
    println!("== depart en scene {}", scene);

    let budget = (secondes * SECONDE) as u64;
    let mut pas = 0u64;
    while pas < budget {
        while appuis.first().is_some_and(|a| a.0 <= pas) {
            let (_, broche, duree) = appuis.remove(0);
            m.appuyer(broche);
            relachements.push((pas + duree, broche));
        }
        relachements.retain(|&(quand, broche)| {
            if quand <= pas {
                m.relacher(broche);
                false
            } else {
                true
            }
        });
        match m.cpu.regs.pc {
            ALLOUER => {
                *prises.entry((m.cpu.regs.lr & !1, m.cpu.regs.get_reg(0))).or_default() += 1;
            }
            LIBERER => {
                *rendus.entry(m.cpu.regs.lr & !1).or_default() += 1;
            }
            _ => {}
        }
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
        let maintenant = demi(&m, SCENE);
        if maintenant != scene {
            println!("  a {:>6.1} s : scene {} -> {}", pas as f64 / SECONDE, scene, maintenant);
            scene = maintenant;
        }
    }

    println!("\n== prises de memoire, par appelant et taille");
    let mut liste: Vec<_> = prises.iter().collect();
    liste.sort_by_key(|((_, taille), n)| std::cmp::Reverse(*taille as u64 * **n));
    for ((appelant, taille), n) in liste.iter().take(30) {
        println!("  depuis {:#010x}  {:>6} octets  x{}", appelant, taille, n);
    }

    println!("\n== rendus de memoire, par appelant");
    let mut liste: Vec<_> = rendus.iter().collect();
    liste.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (appelant, n) in liste.iter().take(30) {
        println!("  depuis {:#010x}  x{}", appelant, n);
    }

    let total_prises: u64 = prises.values().sum();
    let total_rendus: u64 = rendus.values().sum();
    println!("\n  {} prises, {} rendus, solde {}", total_prises, total_rendus, total_prises as i64 - total_rendus as i64);
}
