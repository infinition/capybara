//! Surveillance précise de l'émission UART (0x00005888) sur la scène 116 (connexion).

use capybara::emulator::etat::Instantane;
use capybara::emulator::Machine;

const SCENE: u32 = 0x1800_1BF4;

fn lire16(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn main() {
    let snap_path = "C:\\Users\\infinition\\AppData\\Roaming\\TamagotchiParadise\\data\\sauvegardes\\Tamagotchi_Paradise_Water_MX25L12835F-bad089cd\\reprises\\20260829-003215.tamastate";
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();

    let etat = Instantane::lire(std::path::Path::new(snap_path)).expect("lecture instantane");
    m.restaurer(&etat);

    let s_init = lire16(&m, SCENE);
    println!("== Démarrage probe : scène initiale = {} ==", s_init);

    let mut s_prec = s_init;
    let mut nb_4e4c = 0;
    let mut nb_4ec0 = 0;
    let mut nb_5888 = 0;
    let mut octets_emis: Vec<u8> = Vec::new();

    // 600 frames = 10 secondes
    for frame in 0..600 {
        let scene = lire16(&m, SCENE);
        if scene != s_prec {
            println!(
                "  [frame {:03}] Transition scène : {} -> {}",
                frame, s_prec, scene
            );
            s_prec = scene;
        }

        // Appui 1 à la frame 30 : sélection dans le menu 113
        if frame == 30 {
            println!("  [frame {:03}] Appui Bouton B sur scène {}", frame, scene);
            m.appuyer(0x0B);
        }
        if frame == 36 {
            println!("  [frame {:03}] Relâche Bouton B", frame);
            m.relacher(0x0B);
        }

        // Appui 2 à la frame 120 : validation sur l'écran d'attente 116
        if frame == 120 {
            println!(
                "  [frame {:03}] Second appui Bouton B sur scène {}",
                frame, scene
            );
            m.appuyer(0x0B);
        }
        if frame == 126 {
            println!("  [frame {:03}] Relâche second Bouton B", frame);
            m.relacher(0x0B);
        }

        // Exécution de 1 frame (1_600_000 cycles)
        let cible = m.cpu.cycles + 1_600_000;
        while m.cpu.cycles < cible {
            let pc = m.cpu.regs.pc;

            if pc == 0x0000_4E4C {
                nb_4e4c += 1;
                println!("  >>> [0x00004E4C] UART_SendBuffer #{:03} : r0={:#010x}, buf(r1)={:#010x}, len(r2)={}, flags(r3)={}, LR={:#010x}",
                    nb_4e4c, m.cpu.regs.r[0], m.cpu.regs.r[1], m.cpu.regs.r[2], m.cpu.regs.r[3], m.cpu.regs.lr
                );
            }

            if pc == 0x0000_4EC0 {
                nb_4ec0 += 1;
                if nb_4ec0 <= 5 || nb_4ec0 % 50_000 == 0 {
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
                println!("  >>>>>> [0x00005888] ÉMISSION OCTET #{:03} : {:#04x} ('{}'), handle={:#010x}, LR={:#010x} <<<<<<",
                    nb_5888, octet, if octet.is_ascii_graphic() { octet as char } else { '.' }, m.cpu.regs.r[0], m.cpu.regs.lr
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
        "Total octets émis                         : {}",
        octets_emis.len()
    );
    if !octets_emis.is_empty() {
        print!("Octets émis (hex) : ");
        for b in &octets_emis {
            print!("{:02X} ", b);
        }
        println!();
    }
}
