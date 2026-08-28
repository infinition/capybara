//! Retrouve le drapeau qui dit qu'un son est en cours.
//!
//! Usage : cargo run --release --example drapeau_son_probe --
//!             <dump.bin> <cle hex> <etat.tamastate>
//!
//! Le drapeau ne se reconnait pas a son adresse, qui change d'une edition a
//! l'autre, mais a son allure : il vaut zero au silence, passe a une valeur non
//! nulle pendant que les voix changent, et retombe a zero apres. On suit donc
//! le tableau des voix, on note quand il bouge, et on garde les octets de
//! memoire vive qui suivent exactement ce rythme.

use std::collections::BTreeMap;

use tamagotchi_paradise_rs::emulator::etat::Instantane;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;
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
    m.localiser_les_voix();

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

    // Zone de travail du firmware, la ou vivent ses variables.
    let debut = 0x1801_4200u32;
    let fin = 0x1801_4300u32;
    let taille = (fin - debut) as usize;

    // Pour chaque octet : combien de fois il est non nul pendant qu'une note
    // change, et combien de fois il l'est alors que rien ne joue.
    let mut pendant = vec![0u32; taille];
    let mut hors = vec![0u32; taille];
    let mut releves_pendant = 0u32;
    let mut releves_hors = 0u32;

    let mut derniere = 0.0f32;
    let mut change_a = 0u64;

    while pas < (120.0 * SECONDE) as u64 {
        if pas % periode == 0 {
            let b = touches[(pas / periode) as usize % touches.len()];
            m.appuyer(b);
            tenue = Some((pas + duree, b));
        }
        if let Some((f, b)) = tenue {
            if pas >= f {
                m.relacher(b);
                tenue = None;
            }
        }
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
        if pas % 2000 != 0 {
            continue;
        }
        m.localiser_les_voix();

        let note = m.note_courante();
        if note != derniere {
            derniere = note;
            change_a = pas;
        }
        // Une note joue si elle a change il y a moins d'un quart de seconde.
        let joue = note > 0.0 && pas.saturating_sub(change_a) < (0.25 * SECONDE) as u64;

        let d = &m.bus.sram.data;
        let base = (debut - SRAM) as usize;
        if joue {
            releves_pendant += 1;
            for i in 0..taille {
                if d[base + i] != 0 {
                    pendant[i] += 1;
                }
            }
        } else if note == 0.0 {
            releves_hors += 1;
            for i in 0..taille {
                if d[base + i] != 0 {
                    hors[i] += 1;
                }
            }
        }
    }

    println!(
        "== {} releves pendant une note, {} au silence",
        releves_pendant, releves_hors
    );
    if releves_pendant == 0 || releves_hors == 0 {
        println!("   pas assez de matiere pour conclure");
        return;
    }

    // On garde les octets presents a chaque note et absents a chaque silence.
    let mut candidats: BTreeMap<u32, (f32, f32)> = BTreeMap::new();
    for i in 0..taille {
        let p = pendant[i] as f32 / releves_pendant as f32;
        let h = hors[i] as f32 / releves_hors as f32;
        if p > 0.9 && h < 0.1 {
            candidats.insert(debut + i as u32, (p, h));
        }
    }
    println!("\n== {} octets suivent le rythme des notes\n", candidats.len());
    for (adresse, (p, h)) in candidats.iter().take(40) {
        println!(
            "   {:#010X}  non nul sur {:.0} % des notes, {:.0} % des silences",
            adresse,
            p * 100.0,
            h * 100.0
        );
    }
}
