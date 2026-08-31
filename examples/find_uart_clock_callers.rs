//! Trouve tous les appelants de 0x000047C4 (horloge) et 0x00004EC0 (scrutateur UART).

use capybara::emulator::Machine;

fn main() {
    let path = "C:\\Users\\infinition\\Downloads\\Tamagotchi_Paradise_Water_MX25L12835F.bin";
    let key = 0x5AAF34FB;

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(path).unwrap();

    println!("== Appelants de UART_SendBuffer (0x00004E4C) dans la Flash ==");
    let flash = &m.bus.flash.data;
    for i in (0x11000..flash.len().saturating_sub(4)).step_by(2) {
        let h1 = flash[i] as u16 | ((flash[i + 1] as u16) << 8);
        let h2 = flash[i + 2] as u16 | ((flash[i + 3] as u16) << 8);
        if (h1 & 0xF800) == 0xF000 && (h2 & 0xD000) == 0xD000 {
            let s = ((h1 >> 10) & 1) as u32;
            let j1 = ((h2 >> 13) & 1) as u32;
            let j2 = ((h2 >> 11) & 1) as u32;
            let imm10 = (h1 & 0x03FF) as u32;
            let imm11 = (h2 & 0x07FF) as u32;
            let i1 = !(j1 ^ s) & 1;
            let i2 = !(j2 ^ s) & 1;
            let sign = if s == 1 { 0xFF00_0000 } else { 0 };
            let imm32 = sign | (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);
            let pc = (0x1000_0000 + (i - 0x11000)) as u32 + 4;
            let target = pc.wrapping_add(imm32);
            if target == 0x0000_4E4C {
                println!(
                    "  Flash PC {:#010x} appelle UART_SendBuffer (0x00004E4C)",
                    pc - 4
                );
            }
        }
    }
}
