//! Releve l'appelant de 0x1005B4AC (panic / halt handler).

use capybara::emulator::Machine;

const SECONDE: f64 = 96_000_000.0;
const SCENE: u32 = 0x1800_1BF4;
const ETAT_MACHINE: u32 = 0x1800_1BFA;

fn forcer_scene(m: &mut Machine, scene: u16) {
    let o = (SCENE - 0x1800_0000) as usize;
    m.bus.sram.data[o] = (scene & 0xFF) as u8;
    m.bus.sram.data[o + 1] = (scene >> 8) as u8;
    let eo = (ETAT_MACHINE - 0x1800_0000) as usize;
    m.bus.sram.data[eo] &= !0x07;
}

fn main() {
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();
    m.remplacer_la_pile();

    let fin_totale = m.cpu.cycles + (10.0 * SECONDE) as u64;
    let mut etapes = 0;
    while m.cpu.cycles < fin_totale {
        if m.cpu.regs.pc == 0x1005_B4AC || m.cpu.regs.pc == 0x1005_B4C4 {
            println!(
                "== ENTREE DANS LE POINT DE BLOCAGE {:#010x} ==",
                m.cpu.regs.pc
            );
            println!("  Cycles : {}", m.cpu.cycles);
            println!("  LR (retour appelant) : {:#010x}", m.cpu.regs.lr);
            println!(
                "  r0: {:#010x}, r1: {:#010x}, r2: {:#010x}, r3: {:#010x}",
                m.cpu.regs.r[0], m.cpu.regs.r[1], m.cpu.regs.r[2], m.cpu.regs.r[3]
            );
            println!(
                "  MSP: {:#010x}, PSP: {:#010x}",
                m.cpu.regs.msp, m.cpu.regs.psp
            );
            return;
        }

        // Transitions séquentielles
        if m.cpu.cycles >= (3.0 * SECONDE) as u64 && etapes == 0 {
            forcer_scene(&mut m, 112);
            etapes = 1;
        } else if m.cpu.cycles >= (4.0 * SECONDE) as u64 && etapes == 1 {
            forcer_scene(&mut m, 113);
            etapes = 2;
        } else if m.cpu.cycles >= (5.0 * SECONDE) as u64 && etapes == 2 {
            m.appuyer(capybara::emulator::Machine::BOUTON_B);
            etapes = 3;
        } else if m.cpu.cycles >= (5.3 * SECONDE) as u64 && etapes == 3 {
            m.relacher(capybara::emulator::Machine::BOUTON_B);
            etapes = 4;
        }

        m.step();
    }
    println!("Jamais atteint en 10 secondes.");
}
