//! Test de la combinaison Molette + B pour ouvrir le menu de debug (PSID_DEBUGMENU).

use capybara::emulator::Machine;

const SECONDE: f64 = 96_000_000.0;
const SCENE: u32 = 0x1800_1BF4;

fn lire16(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn main() {
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();
    m.remplacer_la_pile();

    let empreinte = m.empreinte.clone().unwrap();
    let chemin = capybara::emulator::sauvegarde::chemin(&empreinte, "Water");
    if let Ok(partie) = capybara::emulator::sauvegarde::Sauvegarde::lire(&chemin) {
        partie.appliquer(&mut m);
        println!("== Partie Water chargee ==");
    }

    println!("== Demarrage a froid jusqu'a l'ecran de jeu (4s) ==");
    let fin_boot = m.cpu.cycles + (4.0 * SECONDE) as u64;
    let mut scene_prec = 999;
    while m.cpu.cycles < fin_boot {
        let s = lire16(&m, SCENE);
        if s != scene_prec {
            println!(
                "  a {:.1} s : scene {} -> {}",
                m.cpu.cycles as f64 / SECONDE,
                scene_prec,
                s
            );
            scene_prec = s;
        }
        m.step();
    }

    println!("== Maintien Molette (0x08) + Bouton B (0x0B) pendant 2.0s ==");
    m.appuyer(0x08);
    m.appuyer(0x0B);

    let fin_maintien = m.cpu.cycles + (2.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_maintien {
        let s = lire16(&m, SCENE);
        if s != scene_prec {
            println!(
                "  a {:.1} s : scene {} -> {}",
                m.cpu.cycles as f64 / SECONDE,
                scene_prec,
                s
            );
            scene_prec = s;
        }
        m.step();
    }
    m.relacher(0x08);
    m.relacher(0x0B);

    let fin_apres = m.cpu.cycles + (2.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_apres {
        let s = lire16(&m, SCENE);
        if s != scene_prec {
            println!(
                "  a {:.1} s : scene {} -> {}",
                m.cpu.cycles as f64 / SECONDE,
                scene_prec,
                s
            );
            scene_prec = s;
        }
        m.step();
    }

    println!("== Scene finale atteinte : {} ==", lire16(&m, SCENE));
}
