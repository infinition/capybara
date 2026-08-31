//! Sonde d'exploration de la navigation depuis la vue Space (scene 29).

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
    }

    // 1. Laisser la scene 29 s'etablir (1.5s)
    let fin_boot = m.cpu.cycles + (1.5 * SECONDE) as u64;
    while m.cpu.cycles < fin_boot {
        m.step();
    }
    println!("Scene initiale : {}", lire16(&m, SCENE));

    // Test 1 : Appui A pour voir s'il deplace le curseur d'icones dans Space
    println!("== Test appui Bouton A ==");
    m.appuyer(0x09);
    let fin_a = m.cpu.cycles + (0.2 * SECONDE) as u64;
    while m.cpu.cycles < fin_a {
        m.step();
    }
    m.relacher(0x09);
    let fin_a2 = m.cpu.cycles + (1.0 * SECONDE) as u64;
    while m.cpu.cycles < fin_a2 {
        m.step();
    }
    println!("Scene apres A : {}", lire16(&m, SCENE));

    // Test 2 : Appui B pour valider l'icone selectionnee
    println!("== Test appui Bouton B ==");
    m.appuyer(0x0B);
    let fin_b = m.cpu.cycles + (0.2 * SECONDE) as u64;
    while m.cpu.cycles < fin_b {
        m.step();
    }
    m.relacher(0x0B);
    let fin_b2 = m.cpu.cycles + (1.5 * SECONDE) as u64;
    while m.cpu.cycles < fin_b2 {
        m.step();
    }
    println!("Scene apres B : {}", lire16(&m, SCENE));

    // Test 3 : Second appui B
    println!("== Test second appui Bouton B ==");
    m.appuyer(0x0B);
    let fin_b3 = m.cpu.cycles + (0.2 * SECONDE) as u64;
    while m.cpu.cycles < fin_b3 {
        m.step();
    }
    m.relacher(0x0B);
    let fin_b4 = m.cpu.cycles + (1.5 * SECONDE) as u64;
    while m.cpu.cycles < fin_b4 {
        m.step();
    }
    println!("Scene apres second B : {}", lire16(&m, SCENE));
}
