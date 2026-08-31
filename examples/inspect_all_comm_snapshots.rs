//! Teste la stabilité des scènes de tous les instantanés de communication.

use capybara::emulator::etat::Instantane;
use capybara::emulator::Machine;

const SCENE: u32 = 0x1800_1BF4;

fn lire16(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn test_snap(path_rom: &str, snap: &str) {
    let mut m = Machine::new();
    m.device_key = Some(0x5AAF34FB);
    m.load_firmware_file(path_rom).unwrap();

    let etat = match Instantane::lire(std::path::Path::new(snap)) {
        Ok(e) => e,
        Err(_) => return,
    };
    m.restaurer(&etat);

    let s_init = lire16(&m, SCENE);
    let mut transitions = Vec::new();
    let mut s_prec = s_init;

    for frame in 0..60 {
        for _ in 0..1_600_000 {
            m.step();
        }
        let s = lire16(&m, SCENE);
        if s != s_prec {
            transitions.push((frame, s_prec, s));
            s_prec = s;
        }
    }

    let snap_name = std::path::Path::new(snap)
        .file_name()
        .unwrap()
        .to_string_lossy();
    let parent = std::path::Path::new(snap)
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy();
    print!("Snap {}/{}: init={}", parent, snap_name, s_init);
    if transitions.is_empty() {
        println!(" -> RESTE STABLE en scène {}", s_init);
    } else {
        print!(" -> Transitions: ");
        for (f, a, b) in &transitions {
            print!("[f{:02}: {}->{}] ", f, a, b);
        }
        println!();
    }
}

fn main() {
    let base = "C:\\Users\\infinition\\AppData\\Roaming\\TamagotchiParadise\\data\\sauvegardes";
    let water_rom = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin";
    let jade_rom = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Jade_Forest.BIN";

    println!("=== Instantanés Water ===");
    for p in &[
        format!(
            "{}\\{}\\reprises\\20260829-003215.tamastate",
            base, "Tamagotchi_Paradise_Water_MX25L12835F-bad089cd"
        ),
        format!(
            "{}\\{}\\reprises\\20260829-003315.tamastate",
            base, "Tamagotchi_Paradise_Water_MX25L12835F-bad089cd"
        ),
    ] {
        test_snap(water_rom, p);
    }

    println!("\n=== Instantanés Jade ===");
    for p in &[
        format!(
            "{}\\{}\\reprises\\20260828-232857.tamastate",
            base, "Tamagotchi_Paradise_Jade_Forest-786fc58c"
        ),
        format!(
            "{}\\{}\\reprises\\20260828-232957.tamastate",
            base, "Tamagotchi_Paradise_Jade_Forest-786fc58c"
        ),
        format!(
            "{}\\{}\\reprises\\jaddef\\20260829-133839.tamastate",
            base, "Tamagotchi_Paradise_Jade_Forest-786fc58c"
        ),
        format!(
            "{}\\{}\\reprises\\jadee\\20260829-182109.tamastate",
            base, "Tamagotchi_Paradise_Jade_Forest-786fc58c"
        ),
    ] {
        test_snap(jade_rom, p);
    }
}
