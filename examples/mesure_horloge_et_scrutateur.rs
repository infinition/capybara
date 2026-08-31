//! Mesure exacte de l'horloge périphérique 0x000047C4 et de l'appelant de 0x00004EC0.

use capybara::emulator::etat::Instantane;
use capybara::emulator::Machine;

const SECONDE: f64 = 96_000_000.0;

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

    println!("== Surveillance de 0x000047C4 (horloge) et 0x00004EC0 (LSR) ==");
    let mut vu_47c4 = false;
    let mut vu_retour_47c4 = false;
    let mut retour_pc_47c4 = 0u32;

    // Surveillons 0x000047C4 et 0x00004EC0
    let fin = m.cpu.cycles + (5.0 * SECONDE) as u64;
    let mut nb_4ec0 = 0;

    while m.cpu.cycles < fin {
        let pc = m.cpu.regs.pc;

        if pc == 0x0000_47C4 && !vu_47c4 {
            let reg_0c = m.bus.read_u32(0x4500_000C, &mut m.periph, &m.cpu.nvic);
            let reg_10 = m.bus.read_u32(0x4500_0010, &mut m.periph, &m.cpu.nvic);
            println!("Entrée en 0x000047C4 :");
            println!(
                "  0x4500000C = {:#010x} (bits 2:0 = {})",
                reg_0c,
                reg_0c & 7
            );
            println!("  0x45000010 = {:#010x}", reg_10);
            println!("  LR = {:#010x}", m.cpu.regs.lr);
            retour_pc_47c4 = m.cpu.regs.lr & !1;
            vu_47c4 = true;
        }

        if vu_47c4 && !vu_retour_47c4 && pc == retour_pc_47c4 {
            println!("Sortie de 0x000047C4 :");
            println!(
                "  Valeur rendue r0 = {} Hz ({:#x})",
                m.cpu.regs.r[0], m.cpu.regs.r[0]
            );
            vu_retour_47c4 = true;
        }

        if pc == 0x0000_4EC0 {
            nb_4ec0 += 1;
            if nb_4ec0 <= 3 {
                println!(
                    "\n== Appel #{} à 0x00004EC0 (lecture 0x4000B014) ==",
                    nb_4ec0
                );
                println!("  LR = {:#010x}", m.cpu.regs.lr);
                println!(
                    "  r0={:#010x}, r1={:#010x}, r2={:#010x}, r3={:#010x}",
                    m.cpu.regs.r[0], m.cpu.regs.r[1], m.cpu.regs.r[2], m.cpu.regs.r[3]
                );
                let msp = m.cpu.regs.msp;
                println!("  MSP = {:#010x}", msp);
                print!("  Contenu pile (MSP) : ");
                for i in 0..10 {
                    let adr = msp.wrapping_add(i * 4);
                    if (0x1800_0000..0x1802_0000).contains(&adr) {
                        let o = (adr - 0x1800_0000) as usize;
                        let d = &m.bus.sram.data;
                        if o + 3 < d.len() {
                            let val = u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
                            print!("{:#010x} ", val);
                        }
                    }
                }
                println!();
            }
        }

        m.step();
    }

    println!("\nTotal lectures 0x00004EC0 : {}", nb_4ec0);
}
