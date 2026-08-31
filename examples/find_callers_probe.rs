//! Trouve les appelants de 0x1005B4AC dans le firmware.

use capybara::emulator::Machine;

fn main() {
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();

    let cible = 0x1005_B4ACu32;
    let base = 0x6001_1000u32;
    let decalage = (base & 0x00FF_FFFF) as usize;
    let flash = &m.bus.flash.data;

    println!(
        "== Recherche des BL vers {:#010x} dans la fenetre XIP ==",
        cible
    );
    for off in (0..flash.len() - 4).step_by(2) {
        let h1 = flash[off] as u16 | ((flash[off + 1] as u16) << 8);
        let h2 = flash[off + 2] as u16 | ((flash[off + 3] as u16) << 8);

        // Instruction BL : 11110s imm10 | 11j1 1 j2 imm11
        if (h1 & 0xF800) == 0xF000 && (h2 & 0xD000) == 0xD000 {
            let s = ((h1 >> 10) & 1) as u32;
            let imm10 = (h1 & 0x03FF) as u32;
            let j1 = ((h2 >> 13) & 1) as u32;
            let j2 = ((h2 >> 11) & 1) as u32;
            let imm11 = (h2 & 0x07FF) as u32;

            let i1 = !(j1 ^ s) & 1;
            let i2 = !(j2 ^ s) & 1;

            let imm32 = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);
            let imm32 = if s == 1 { imm32 | 0xFE00_0000 } else { imm32 };

            let pc = 0x1000_0000u32
                .wrapping_add((off - decalage) as u32)
                .wrapping_add(4);
            let dest = pc.wrapping_add(imm32);

            if dest == cible {
                println!(
                    "  Appel BL trouve a l'adresse {:#010x} (off {:#08x})",
                    pc - 4,
                    off
                );
            }
        }
    }
}
