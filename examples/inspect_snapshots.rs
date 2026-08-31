//! Inspecte la scene de chaque instantane dans le dossier water.

use capybara::emulator::etat::Instantane;
use capybara::emulator::Machine;
use std::fs;
use std::path::Path;

const SCENE: u32 = 0x1800_1BF4;

fn visiter(dir: &Path, m: &mut Machine) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                visiter(&p, m);
            } else if p.extension().and_then(|s| s.to_str()) == Some("tamastate") {
                if let Ok(etat) = Instantane::lire(&p) {
                    m.restaurer(&etat);
                    let o = (SCENE - 0x1800_0000) as usize;
                    let scene = m.bus.sram.data[o] as u16 | ((m.bus.sram.data[o + 1] as u16) << 8);
                    println!(
                        "  {} : scene {}, cycles {}",
                        p.display(),
                        scene,
                        etat.cycles
                    );
                }
            }
        }
    }
}

fn main() {
    let base_dir =
        Path::new("C:\\Users\\infinition\\AppData\\Roaming\\TamagotchiParadise\\data\\sauvegardes");
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();

    println!("== Recherche récursive des instantanés ==");
    visiter(base_dir, &mut m);
}
