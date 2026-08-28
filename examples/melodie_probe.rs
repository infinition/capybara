//! Releve la melodie telle que le firmware la compose, et la rend en WAV.
//!
//! Usage : cargo run --release --example melodie_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> <sortie.wav> [melodies]
//!
//! C'est la reference contre laquelle juger le son de l'interface. La note est
//! relevee toutes les mille instructions, cent fois plus fin que la plus courte
//! note du firmware, et le WAV est ecrit au temps de la console : ce qu'on y
//! entend est donc l'ordre et le rythme de la vraie machine, sans rien devoir a
//! la cadence d'affichage.

use tamagotchi_paradise_rs::emulator::etat::Instantane;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;
const TAUX: u32 = 44_100;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let wav_path = a.next().expect("sortie.wav");
    let melodies: usize = a.next().and_then(|v| v.parse().ok()).unwrap_or(3);

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
    let periode = (2.5 * SECONDE) as u64;
    let duree_appui = (0.3 * SECONDE) as u64;
    let budget = (600.0 * SECONDE) as u64;

    // Suite de notes relevee : frequence, et duree en cycles de console.
    let mut suite: Vec<(f32, u64)> = Vec::new();
    let mut note = 0.0f32;
    let mut depuis = 0u64;
    let mut vues = 0usize;
    let mut jouait = false;

    let mut tenue: Option<(u64, u32)> = None;
    let mut pas = 0u64;
    while pas < budget && vues < melodies {
        if pas % periode == 0 {
            let broche = touches[(pas / periode) as usize % touches.len()];
            m.appuyer(broche);
            tenue = Some((pas + duree_appui, broche));
        }
        if let Some((fin, broche)) = tenue {
            if pas >= fin {
                m.relacher(broche);
                tenue = None;
            }
        }
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;

        if pas % 1000 != 0 {
            continue;
        }
        let joue = m.son_en_cours();
        if joue && !jouait {
            m.localiser_les_voix();
        }
        if joue && !jouait {
            // Debut d'une melodie : on repart d'une suite propre.
            depuis = m.cpu.cycles;
            note = 0.0;
        }
        if !joue && jouait {
            suite.push((note, m.cpu.cycles - depuis));
            note = 0.0;
            depuis = m.cpu.cycles;
            vues += 1;
            // Une seconde de silence entre deux melodies, pour les separer a
            // l'oreille dans le fichier rendu.
            suite.push((0.0, SECONDE as u64));
        }
        jouait = joue;
        if !joue {
            continue;
        }
        let courante = m.note_courante();
        if (courante - note).abs() > 0.5 {
            suite.push((note, m.cpu.cycles - depuis));
            note = courante;
            depuis = m.cpu.cycles;
        }
    }

    if suite.is_empty() {
        println!("aucune melodie en {:.0} s de temps console", pas as f64 / SECONDE);
        return;
    }

    println!("== suite relevee, {} segments", suite.len());
    let mut horloge = 0.0f64;
    for (i, &(f, c)) in suite.iter().enumerate() {
        let ms = c as f64 / SECONDE * 1000.0;
        println!(
            "  {:>3}  a {:>8.1} ms  pendant {:>7.1} ms  {}",
            i,
            horloge,
            ms,
            if f > 0.0 { format!("{f:.0} Hz") } else { "silence".to_string() }
        );
        horloge += ms;
    }

    // Rendu au temps de la console : un signal carre, phase gardee.
    let mut echantillons: Vec<i16> = Vec::new();
    let mut phase = 0.0f32;
    for &(f, cycles) in &suite {
        let compte = (cycles as f64 / SECONDE * TAUX as f64) as usize;
        if f <= 0.0 {
            echantillons.extend(std::iter::repeat(0).take(compte));
            phase = 0.0;
            continue;
        }
        let avance = f / TAUX as f32;
        for _ in 0..compte {
            phase += avance;
            if phase >= 1.0 {
                phase -= 1.0;
            }
            echantillons.push(if phase < 0.5 { 6000 } else { -6000 });
        }
    }
    ecrire_wav(&wav_path, &echantillons);
    println!(
        "== {} ecrit, {:.2} s",
        wav_path,
        echantillons.len() as f64 / TAUX as f64
    );
}

fn ecrire_wav(chemin: &str, echantillons: &[i16]) {
    let octets = echantillons.len() * 2;
    let mut f: Vec<u8> = Vec::with_capacity(44 + octets);
    f.extend(b"RIFF");
    f.extend(((36 + octets) as u32).to_le_bytes());
    f.extend(b"WAVEfmt ");
    f.extend(16u32.to_le_bytes());
    f.extend(1u16.to_le_bytes()); // PCM
    f.extend(1u16.to_le_bytes()); // mono
    f.extend(TAUX.to_le_bytes());
    f.extend((TAUX * 2).to_le_bytes());
    f.extend(2u16.to_le_bytes());
    f.extend(16u16.to_le_bytes());
    f.extend(b"data");
    f.extend((octets as u32).to_le_bytes());
    for e in echantillons {
        f.extend(e.to_le_bytes());
    }
    std::fs::write(chemin, f).expect("ecriture du wav");
}
