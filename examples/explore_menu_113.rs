//! Exploration précise des touches sur la scène 113.

use capybara::emulator::etat::Instantane;
use capybara::emulator::Machine;

const SCENE: u32 = 0x1800_1BF4;

fn lire16(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn tester_touche(nom: &str, broche: u32) {
    let snap_path = "C:\\Users\\infinition\\AppData\\Roaming\\TamagotchiParadise\\data\\sauvegardes\\Tamagotchi_Paradise_Water_MX25L12835F-bad089cd\\reprises\\20260829-003215.tamastate";
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin";
    let mut m = Machine::new();
    m.device_key = Some(0x5AAF34FB);
    m.load_firmware_file(path).unwrap();

    let etat = Instantane::lire(std::path::Path::new(snap_path)).expect("lecture instantane");
    m.restaurer(&etat);

    println!("\n=== Test touche : {} (broche {:#x}) ===", nom, broche);
    let s_init = lire16(&m, SCENE);

    for frame in 0..120 {
        if frame == 20 {
            println!("  [frame 20] Appui {}", nom);
            m.appuyer(broche);
        }
        if frame == 26 {
            println!("  [frame 26] Relâche {}", nom);
            m.relacher(broche);
        }

        for _ in 0..1_600_000 {
            m.step();
        }
        let s = lire16(&m, SCENE);
        if s != s_init && frame < 40 {
            println!("  [frame {:03}] Transition : {} -> {}", frame, s_init, s);
            return;
        }
    }
    println!("  Fin : scène = {}", lire16(&m, SCENE));
}

fn main() {
    tester_touche("Bouton A (0x0A)", 0x0A);
    tester_touche("Bouton C (0x0C)", 0x0C);
    tester_touche("Molette clic (0x08)", 0x08);
}
