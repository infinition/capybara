//! Test de navigation sur la scène de communication 113.

use capybara::emulator::etat::Instantane;
use capybara::emulator::Machine;

const SCENE: u32 = 0x1800_1BF4;

fn lire16(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn tester_action(nom: &str, action: impl Fn(&mut Machine)) {
    let snap_path = "C:\\Users\\infinition\\AppData\\Roaming\\TamagotchiParadise\\data\\sauvegardes\\Tamagotchi_Paradise_Jade_Forest-786fc58c\\reprises\\jadee\\20260829-182309.tamastate";
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Jade_Forest.BIN";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();

    let etat = Instantane::lire(std::path::Path::new(snap_path)).expect("lecture instantane");
    m.restaurer(&etat);

    println!("\n=== Test action : {} ===", nom);
    println!("  Scène initiale : {}", lire16(&m, SCENE));

    // Laisse tourner 10 frames
    for _ in 0..10 {
        for _ in 0..1_600_000 {
            m.step();
        }
    }

    action(&mut m);

    // Laisse tourner 60 frames (1 seconde)
    let mut scene_prec = lire16(&m, SCENE);
    for frame in 0..60 {
        for _ in 0..1_600_000 {
            m.step();
        }
        let s = lire16(&m, SCENE);
        if s != scene_prec {
            println!(
                "  [frame +{:02}] Transition scène : {} -> {}",
                frame, scene_prec, s
            );
            scene_prec = s;
        }
    }
    println!("  Scène finale : {}", scene_prec);
}

fn main() {
    // Test 1 : Appui A (0x0A)
    tester_action("Appui A (0x0A)", |m| {
        m.appuyer(0x0A);
        for _ in 0..10 {
            for _ in 0..1_600_000 {
                m.step();
            }
        }
        m.relacher(0x0A);
    });

    // Test 2 : Appui B (0x0B)
    tester_action("Appui B (0x0B)", |m| {
        m.appuyer(0x0B);
        for _ in 0..10 {
            for _ in 0..1_600_000 {
                m.step();
            }
        }
        m.relacher(0x0B);
    });

    // Test 3 : Appui C (0x0C)
    tester_action("Appui C (0x0C)", |m| {
        m.appuyer(0x0C);
        for _ in 0..10 {
            for _ in 0..1_600_000 {
                m.step();
            }
        }
        m.relacher(0x0C);
    });

    // Test 4 : Tourner molette +1 puis appui B
    tester_action("Molette +1 puis Appui B", |m| {
        m.periph.gpio.step_dial(1);
        for _ in 0..10 {
            for _ in 0..1_600_000 {
                m.step();
            }
        }
        m.appuyer(0x0B);
        for _ in 0..10 {
            for _ in 0..1_600_000 {
                m.step();
            }
        }
        m.relacher(0x0B);
    });

    // Test 5 : Appui A puis appui B
    tester_action("Appui A puis Appui B", |m| {
        m.appuyer(0x0A);
        for _ in 0..6 {
            for _ in 0..1_600_000 {
                m.step();
            }
        }
        m.relacher(0x0A);
        for _ in 0..10 {
            for _ in 0..1_600_000 {
                m.step();
            }
        }
        m.appuyer(0x0B);
        for _ in 0..6 {
            for _ in 0..1_600_000 {
                m.step();
            }
        }
        m.relacher(0x0B);
    });
}
