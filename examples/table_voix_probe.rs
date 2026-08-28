//! Retrouve le tableau des voix audio dans un firmware ou il a bouge.
//!
//! Usage : cargo run --release --example table_voix_probe --
//!             <dump.bin> <cle hex> <etat.tamastate>
//!
//! Le tableau ne se reconnait pas a son adresse, qui change d'une edition a
//! l'autre, mais a sa forme : chaque voix porte l'horloge du coeur en tete,
//! 0x05B8D800 soit 96 MHz, un temoin d'activite en `+8` et un volume en `+0xC`.
//! On attend qu'un son joue, puis on balaie la memoire vive a la recherche de
//! cette signature. La sonde rend les adresses candidates et le pas entre
//! elles, ce qui donne la base et la taille d'une entree.

use tamagotchi_paradise_rs::emulator::etat::Instantane;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;
const HORLOGE: u32 = 0x05B8_D800;
const SRAM: u32 = 0x1800_0000;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));

    let touches = [
        Machine::BOUTON_A,
        Machine::BOUTON_B,
        Machine::BOUTON_MOLETTE,
        Machine::BOUTON_C,
    ];
    let periode = (1.5 * SECONDE) as u64;
    let duree = (0.25 * SECONDE) as u64;
    let mut tenue: Option<(u64, u32)> = None;
    let mut pas = 0u64;

    // On attend un son, puis on regarde la memoire pendant qu'il joue.
    while pas < (400.0 * SECONDE) as u64 {
        if pas % periode == 0 {
            let b = touches[(pas / periode) as usize % touches.len()];
            m.appuyer(b);
            tenue = Some((pas + duree, b));
        }
        if let Some((fin, b)) = tenue {
            if pas >= fin {
                m.relacher(b);
                tenue = None;
            }
        }
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
        if pas % 200 != 0 || !m.son_en_cours() {
            continue;
        }

        let trouves = balayer(&m);
        if trouves.is_empty() {
            continue;
        }
        println!("== son en cours a {:.1} s de temps console", pas as f64 / SECONDE);
        println!("   {} entrees portant l'horloge du coeur\n", trouves.len());
        let mut precedent: Option<u32> = None;
        for (adresse, actif, valeur, volume) in &trouves {
            let ecart = precedent.map(|p| adresse - p).unwrap_or(0);
            println!(
                "   {:#010X}  actif {}  champ {:>6}  volume {:>3}  {}",
                adresse,
                actif,
                valeur,
                volume,
                if ecart > 0 { format!("+{:#X} depuis la precedente", ecart) } else { String::new() }
            );
            precedent = Some(*adresse);
        }
        let actives: Vec<_> = trouves.iter().filter(|t| t.1 != 0 && t.2 > 0).collect();
        if !actives.is_empty() {
            println!("\n   voix qui sonne : {:#010X}, champ {}", actives[0].0, actives[0].2);
            println!(
                "   hauteur si periode : {:.1} Hz",
                750_000.0 / actives[0].2 as f32
            );
        }
        if let Some(base) = trouves.first() {
            println!("\n   VOIX_AUDIO candidat : {:#010X}", base.0);
        }
        return;
    }
    println!("aucun son en 400 s de temps console, ou signature absente");
}

/// Adresses de SRAM portant l'horloge du coeur, avec leur temoin, leur champ de
/// hauteur et leur volume.
fn balayer(m: &Machine) -> Vec<(u32, u8, u32, u32)> {
    let d = &m.bus.sram.data;
    let mut trouves = Vec::new();
    let mut o = 0usize;
    while o + 0x10 <= d.len() {
        if u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) == HORLOGE {
            let champ = u32::from_le_bytes([d[o + 4], d[o + 5], d[o + 6], d[o + 7]]);
            let volume = u32::from_le_bytes([d[o + 12], d[o + 13], d[o + 14], d[o + 15]]);
            trouves.push((SRAM + o as u32, d[o + 8], champ, volume));
        }
        o += 4;
    }
    trouves
}
