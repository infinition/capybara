//! Sonde pour surveiller l'atteinte de la fonction d'émission 0x00005888.

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

    let o = (SCENE - 0x1800_0000) as usize;
    let scene = m.bus.sram.data[o] as u16 | ((m.bus.sram.data[o + 1] as u16) << 8);
    println!("== Instantané restauré : scène {} ==", scene);

    let cycles_depart = m.cpu.cycles;
    println!("== Cycles au départ : {} ==", cycles_depart);

    let mut etape = 0;
    let fin = cycles_depart + (8.0 * SECONDE) as u64;

    // Boucle de 300 frames (5 secondes)
    let mut nb_4ec0 = 0;
    let mut nb_5888 = 0;
    let mut nb_4e4c = 0;
    let mut octets_emis: Vec<u8> = Vec::new();
    let mut callers_4e4c: Vec<u32> = Vec::new();
    let mut scene_prec = scene;
    for frame in 0..300 {
        let scene = lire16(&m, SCENE);
        if scene != scene_prec {
            println!(
                "  [frame {:03}] Transition scène : {} -> {}",
                frame, scene_prec, scene
            );
            scene_prec = scene;
        }

        // Appui 1 à la frame 20 pendant 6 frames (100 ms) pour valider le menu de comm
        if frame == 20 {
            println!("  [frame {:03}] Appui Bouton B (scène {})", frame, scene);
            m.appuyer(0x0B);
        }
        if frame == 26 {
            println!("  [frame {:03}] Relâche Bouton B", frame);
            m.relacher(0x0B);
        }

        // Appui 2 à la frame 80 pendant 6 frames pour lancer l'attente / émission
        if frame == 80 {
            println!(
                "  [frame {:03}] Second appui Bouton B (scène {})",
                frame, scene
            );
            m.appuyer(0x0B);
        }
        if frame == 86 {
            println!("  [frame {:03}] Relâche second Bouton B", frame);
            m.relacher(0x0B);
        }

        // Avance de 1 frame (1_600_000 cycles = 16.66 ms)
        let cible = m.cpu.cycles + 1_600_000;
        while m.cpu.cycles < cible {
            let pc = m.cpu.regs.pc;

            if pc == 0x0000_4E4C {
                nb_4e4c += 1;
                callers_4e4c.push(m.cpu.regs.lr);
                if nb_4e4c <= 5 {
                    println!("  >>> [0x00004E4C] UART_SendBuffer : r0={:#010x}, r1={:#010x}, len(r2)={}, flags(r3)={}, LR={:#010x}",
                        m.cpu.regs.r[0], m.cpu.regs.r[1], m.cpu.regs.r[2], m.cpu.regs.r[3], m.cpu.regs.lr
                    );
                }
            }

            if pc == 0x0000_4EC0 {
                nb_4ec0 += 1;
            }

            if pc == 0x0000_5888 {
                nb_5888 += 1;
                let octet = (m.cpu.regs.r[1] & 0xFF) as u8;
                octets_emis.push(octet);
                println!(
                    "  >>> [0x00005888] ÉMISSION OCTET #{:03} : {:#04x} ('{}'), LR={:#010x}",
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
    println!("Total appels émission (0x00005888)       : {}", nb_5888);
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
