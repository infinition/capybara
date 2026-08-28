//! Temporaire : etat complet des huit voix pendant une melodie.
use tamagotchi_paradise_rs::emulator::etat::Instantane;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let key = u32::from_str_radix(a.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().unwrap();

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).unwrap());

    let touches = [Machine::BOUTON_A, Machine::BOUTON_B, Machine::BOUTON_MOLETTE, Machine::BOUTON_C];
    let periode = (2.5 * SECONDE) as u64;
    let duree = (0.3 * SECONDE) as u64;
    let mut tenue: Option<(u64, u32)> = None;
    let mut pas = 0u64;
    let mut precedent = String::new();
    let mut melodies = 0;
    let mut jouait = false;
    let debut_cycles = m.cpu.cycles;

    while pas < (600.0 * SECONDE) as u64 && melodies < 2 {
        if pas % periode == 0 {
            let b = touches[(pas / periode) as usize % touches.len()];
            m.appuyer(b);
            tenue = Some((pas + duree, b));
        }
        if let Some((fin, b)) = tenue {
            if pas >= fin { m.relacher(b); tenue = None; }
        }
        if !matches!(m.step(), StepResult::Ok(_)) { break; }
        pas += 1;
        if pas % 500 != 0 { continue; }

        let joue = m.son_en_cours();
        if joue && !jouait { println!("--- debut de melodie ---"); }
        if !joue && jouait { println!("--- fin de melodie ---"); melodies += 1; }
        jouait = joue;
        if !joue { continue; }

        let mut ligne = String::new();
        for i in 0..Machine::NOMBRE_VOIX {
            let base = Machine::VOIX_AUDIO + i * Machine::TAILLE_VOIX;
            let actif = m.bus.sram.read_u8((base + 8 - 0x1800_0000) as usize);
            let f = lire32(&m, base + 4);
            let v = lire32(&m, base + 0xC);
            let d = lire32(&m, base);
            let _ = (d, v);
            if actif != 0 {
                ligne.push_str(&format!("voix {i} f={f} :"));
                for mot in 0..13 {
                    ligne.push_str(&format!(" {:08X}", lire32(&m, base + mot * 4)));
                }
            }
        }
        if ligne != precedent {
            println!("{:>8.1} ms  {}", (m.cpu.cycles - debut_cycles) as f64 / SECONDE * 1000.0, ligne);
            precedent = ligne;
        }
    }
}

fn lire32(m: &Machine, adresse: u32) -> u32 {
    let o = (adresse - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    if o + 4 > d.len() { return 0; }
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}
