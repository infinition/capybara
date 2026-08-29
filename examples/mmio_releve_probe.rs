//! Releve tout ce que la console touche cote materiel, depuis un instantane.
//!
//! Usage : cargo run --release --example mmio_releve_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [secondes]
//!
//! Sortie faite pour etre comparee d'un etat a l'autre avec `diff` : une ligne
//! par registre, triee par adresse, et les compteurs ramenes a la seconde puis
//! arrondis par ordre de grandeur. Sans cet arrondi le bruit de cadence noierait
//! la difference qu'on cherche.
//!
//! C'est le defaut qu'avaient les mesures precedentes : elles comparaient l'etat
//! a lui meme, en relevant les pages avant et apres dans la meme execution. Une
//! page deja ouverte n'y ressort jamais. Ici on compare deux executions.

use tamagotchi_paradise_rs::emulator::etat::Instantane;
use tamagotchi_paradise_rs::emulator::mmu::periph;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;
const SCENE: u32 = 0x1800_1BF4;

/// Ordre de grandeur d'un compte par seconde. Deux relevés du meme regime
/// doivent tomber sur le meme palier, deux regimes differents non.
fn palier(par_seconde: f64) -> &'static str {
    match par_seconde {
        v if v < 0.5 => ".",
        v if v < 5.0 => "unites",
        v if v < 50.0 => "dizaines",
        v if v < 500.0 => "centaines",
        v if v < 5_000.0 => "milliers",
        v if v < 50_000.0 => "dizaines de milliers",
        _ => "au dela",
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

    // MMIO_FORCE=adr:val,... impose une valeur de lecture sur un registre non
    // modelise. Sans cela un pilote qui attend un temoin d'etat tourne en rond
    // et on ne voit jamais la suite de sa banque.
    if let Ok(v) = std::env::var("MMIO_FORCE") {
        for paire in v.split(',') {
            if let Some((a, val)) = paire.split_once(':') {
                let a = u32::from_str_radix(a.trim().trim_start_matches("0x"), 16);
                let val = u32::from_str_radix(val.trim().trim_start_matches("0x"), 16);
                if let (Ok(a), Ok(val)) = (a, val) {
                    m.bus.mmio_trace.forcees.insert(a, val);
                }
            }
        }
    }

    // Une demi seconde pour que le regime s'etablisse, sans trace, puis on
    // remet le compteur a zero et on mesure.
    avancer(&mut m, 0.5);
    m.bus.mmio_trace.all.clear();
    m.bus.mmio_trace.enabled = true;
    let scene = lire16(&m, SCENE);

    // ENTREES="seconde:broche:duree,..." rejoue des appuis pendant la mesure.
    // Sans cela on ne voit jamais la mise en route d'un peripherique : elle a
    // lieu a l'appui, donc avant la fenetre si l'instantane est pris apres.
    let mut appuis: Vec<(f64, u32, f64)> = std::env::var("ENTREES")
        .ok()
        .map(|v| {
            v.split(',')
                .filter_map(|e| {
                    let c: Vec<&str> = e.split(':').collect();
                    if c.len() != 3 {
                        return None;
                    }
                    Some((
                        c[0].parse().ok()?,
                        u32::from_str_radix(c[1].trim_start_matches("0x"), 16).ok()?,
                        c[2].parse().ok()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    appuis.sort_by(|a, b| a.0.total_cmp(&b.0));
    let depart = m.cpu.cycles;
    let mut relachements: Vec<(f64, u32)> = Vec::new();
    let mut horloge = 0.0f64;
    while horloge < secondes {
        while appuis.first().is_some_and(|a| a.0 <= horloge) {
            let (t, broche, duree) = appuis.remove(0);
            m.appuyer(broche);
            relachements.push((t + duree, broche));
        }
        while relachements.first().is_some_and(|r| r.0 <= horloge) {
            let (_, broche) = relachements.remove(0);
            m.relacher(broche);
        }
        relachements.sort_by(|a, b| a.0.total_cmp(&b.0));
        avancer(&mut m, 0.05);
        horloge = (m.cpu.cycles - depart) as f64 / SECONDE;
    }

    let nom = tamagotchi_paradise_rs::emulator::scenes::TableScenes::reperer(
        &m.bus.flash.data,
        m.periph.xip.base,
    )
    .and_then(|t| t.nom(scene).map(str::to_string))
    .unwrap_or_else(|| "?".to_string());
    println!("# scene {} {}", scene, nom);
    println!("# {} secondes de temps console", secondes);
    let mut v: Vec<_> = m.bus.mmio_trace.all.iter().map(|(a, s)| (*a, *s)).collect();
    v.sort_by_key(|(a, _)| *a);
    for (adresse, s) in &v {
        println!(
            "{:#010x}  {:<10}  lectures {:<22}  ecritures {:<22}  premier PC {:#010x}",
            adresse,
            periph::name_of(adresse & !0xFFF),
            palier(s.reads as f64 / secondes),
            palier(s.writes as f64 / secondes),
            s.first_pc
        );
    }
    println!("# {} registres sur {} pages", v.len(), pages(&v));
}

fn pages(v: &[(u32, tamagotchi_paradise_rs::emulator::mmu::MmioStat)]) -> usize {
    let p: std::collections::BTreeSet<u32> = v.iter().map(|(a, _)| a & !0xFFF).collect();
    p.len()
}

fn avancer(m: &mut Machine, secondes: f64) {
    let fin = m.cpu.cycles + (secondes * SECONDE) as u64;
    while m.cpu.cycles < fin {
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
    }
}

fn lire16(m: &Machine, adresse: u32) -> u16 {
    let o = (adresse - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    if o + 2 > d.len() {
        return 0;
    }
    u16::from_le_bytes([d[o], d[o + 1]])
}
