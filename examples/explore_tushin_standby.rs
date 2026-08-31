//! Sonde pour explorer l'écran de communication (scène 113 -> 116) et l'émission UART.

use capybara::emulator::etat::Instantane;
use capybara::emulator::Machine;

const SECONDE: f64 = 96_000_000.0;
const SCENE: u32 = 0x1800_1BF4;

fn lire16(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn main() {
    let snap_path = "C:\\Users\\infinition\\AppData\\Roaming\\TamagotchiParadise\\data\\sauvegardes\\Tamagotchi_Paradise_Jade_Forest-786fc58c\\reprises\\jadee\\20260829-182309.tamastate";
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Jade_Forest.BIN";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();

    let etat = Instantane::lire(std::path::Path::new(snap_path)).expect("lecture instantane");
    m.restaurer(&etat);

    let scene_init = lire16(&m, SCENE);
    println!("== Démarrage probe : scène initiale = {} ==", scene_init);

    let mut scene_prec = scene_init;
    let mut nb_4ec0 = 0;
    let mut nb_5888 = 0;
    let mut nb_4e4c = 0;
    let mut octets_emis: Vec<u8> = Vec::new();

    // Boucle d'exécution avec gestion explicite des touches
    // Scène 113 (Menu Comm) -> appui B -> Scène 116 (Attente / Connexion)
    for frame in 0..1200 {
        // ~20 secondes à 60 fps
        let scene = lire16(&m, SCENE);
        if scene != scene_prec {
            println!(
                "  [frame {:04}] Changement de scène : {} -> {}",
                frame, scene_prec, scene
            );
            scene_prec = scene;
        }

        // Sur la scène 113, appui sur B à la frame 30
        if scene == 113 && frame == 30 {
            println!("  [frame {:04}] Appui B sur scène 113", frame);
            m.periph.gpio.btn_b = true;
        }
        if frame == 40 {
            m.periph.gpio.btn_b = false;
        }

        // Sur la scène 116 (Standby), appui sur B à la frame 100
        if scene == 116 && frame == 100 {
            println!(
                "  [frame {:04}] Appui B sur scène 116 (lancement comm)",
                frame
            );
            m.periph.gpio.btn_b = true;
        }
        if frame == 110 {
            m.periph.gpio.btn_b = false;
        }

        // Exécution de 1/60e de seconde (1_600_000 cycles)
        let cycles_cible = m.cpu.cycles + 1_600_000;
        while m.cpu.cycles < cycles_cible {
            let pc = m.cpu.regs.pc;

            if pc == 0x0000_4E4C {
                nb_4e4c += 1;
                println!("  [0x00004E4C] UART_SendBuffer : r0={:#010x}, r1={:#010x}, r2={:#010x}, r3={:#010x}, LR={:#010x}",
                    m.cpu.regs.r[0], m.cpu.regs.r[1], m.cpu.regs.r[2], m.cpu.regs.r[3], m.cpu.regs.lr
                );
            }

            if pc == 0x0000_4EC0 {
                nb_4ec0 += 1;
                if nb_4ec0 <= 10 || nb_4ec0 % 100_000 == 0 {
                    println!(
                        "  [0x00004EC0] LSR read #{:06} : r0={:#010x}, LR={:#010x}",
                        nb_4ec0, m.cpu.regs.r[0], m.cpu.regs.lr
                    );
                }
            }

            if pc == 0x0000_5888 {
                nb_5888 += 1;
                let octet = (m.cpu.regs.r[1] & 0xFF) as u8;
                octets_emis.push(octet);
                println!(
                    "  >>> [0x00005888] ÉMISSION OCTET #{:03} : {:#04x} ('{}') (LR={:#010x})",
                    nb_5888,
                    octet,
                    if octet.is_ascii_graphic() {
                        octet as char
                    } else {
                        '.'
                    },
                    m.cpu.regs.lr
                );
            }

            m.step();
        }
    }

    println!("\n==================================================");
    println!("Total appels UART_SendBuffer (0x00004E4C) : {}", nb_4e4c);
    println!("Total lectures LSR (0x00004EC0)          : {}", nb_4ec0);
    println!("Total émissions d'octets (0x00005888)    : {}", nb_5888);
    println!(
        "Octets émis au total                     : {}",
        octets_emis.len()
    );
    if !octets_emis.is_empty() {
        print!("Hex: ");
        for b in &octets_emis {
            print!("{:02X} ", b);
        }
        println!();
    }
}
