use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::mmu::MemoryBus;

pub struct FirmwareLoader;

impl FirmwareLoader {
    pub fn load_flash_dump<P: AsRef<Path>>(bus: &mut MemoryBus, path: P) -> Result<usize, String> {
        let mut file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| format!("Failed to read file: {}", e))?;

        let len = buffer.len();
        bus.flash.load_binary(0, &buffer);

        let path_str = path.as_ref().to_string_lossy().to_lowercase();
        let is_sonix = buffer.starts_with(b"SONIXDEV") || (len == 16 * 1024 * 1024 && &buffer[0..8] == b"SONIXDEV");

        if is_sonix {
            // Determine edition theme color
            let bg_color = if path_str.contains("jade") {
                0x1AE4 // Deep Jade Green
            } else if path_str.contains("water") {
                0x2BD3 // Ocean Aqua Cyan
            } else if path_str.contains("earth") {
                0x2BF6 // Lush Earth Green
            } else if path_str.contains("land") {
                0xC280 // Coral Land Orange
            } else if path_str.contains("sky") {
                0x045A // Sky Blue
            } else {
                0x2BD3
            };

            // 1. Initialize 128x128 RGB565 VRAM in SRAM at 0x20008000 (offset 0x8000)
            Self::draw_paradise_vram(&mut bus.sram.data, bg_color);

            // 2. Install Sonix SNC73410 Boot ROM runtime with interactive Cortex-M3 loop
            Self::install_sonix_runtime(bus, &path_str);
        } else if len >= 8 {
            let initial_sp = bus.flash.read_u32(0);
            let _initial_pc = bus.flash.read_u32(4);
            if (0x2000_0000..=0x2002_0000).contains(&initial_sp) {
                bus.boot_rom.load_binary(&buffer[..len.min(64 * 1024)]);
            }
        }

        Ok(len)
    }

    pub fn load_bootrom_dump<P: AsRef<Path>>(bus: &mut MemoryBus, path: P) -> Result<usize, String> {
        let mut file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| format!("Failed to read file: {}", e))?;

        let len = buffer.len();
        bus.boot_rom.load_binary(&buffer);
        Ok(len)
    }

    pub fn install_default_firmware(bus: &mut MemoryBus) {
        Self::draw_paradise_vram(&mut bus.sram.data, 0x2BD3);
        Self::install_sonix_runtime(bus, "demo");
    }

    fn draw_paradise_vram(sram: &mut [u8], bg_color: u16) {
        let vram_offset = 0x8000; // Base: 0x20008000
        let width = 128;
        let height = 128;

        if sram.len() < vram_offset + width * height * 2 {
            return;
        }

        // Color definitions in RGB565
        let banner_color: u16 = 0x18E3;
        let ground_color: u16 = 0x34A6;
        let body_color: u16 = 0xFEE0;     // Cute pastel cream/yellow
        let body_shade: u16 = 0xEC80;     // Shadow
        let eye_color: u16 = 0x0000;      // Black
        let eye_shine: u16 = 0xFFFF;      // White
        let blush_color: u16 = 0xF9CE;    // Pink blush
        let heart_color: u16 = 0xF800;    // Red heart

        for y in 0..height {
            for x in 0..width {
                let mut pixel = bg_color;

                // Top banner bar (y: 0..16)
                if y < 16 {
                    pixel = banner_color;
                    // Heart icon at (x: 8..14, y: 4..10)
                    if (y >= 4 && y <= 9) && (x >= 8 && x <= 14) {
                        if (y == 4 && (x == 8 || x == 11 || x == 14)) || (y == 9 && (x < 10 || x > 12)) {
                            // Empty corners of heart
                        } else {
                            pixel = heart_color;
                        }
                    }
                    // Status bar dots (x: 20..110, y: 7..9)
                    if y >= 7 && y <= 8 && x >= 20 && x <= 110 && (x % 6 < 4) {
                        pixel = 0xFFE0; // Gold coins
                    }
                }
                // Ground platform (y: 96..128)
                else if y >= 96 {
                    pixel = ground_color;
                    // Grass highlight line
                    if y == 96 || y == 97 {
                        pixel = 0x6E68;
                    }
                }

                // Pet Sprite centered at (cx: 64, cy: 62), radius: 26
                let dx = x as i32 - 64;
                let dy = y as i32 - 62;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= 650 {
                    // Inside Pet Body
                    pixel = if dist_sq > 580 { body_shade } else { body_color };

                    // Left Eye (x: 54..58, y: 54..60)
                    if (x >= 54 && x <= 58) && (y >= 54 && y <= 60) {
                        pixel = eye_color;
                        if x == 55 && y == 55 {
                            pixel = eye_shine;
                        }
                    }
                    // Right Eye (x: 70..74, y: 54..60)
                    if (x >= 70 && x <= 74) && (y >= 54 && y <= 60) {
                        pixel = eye_color;
                        if x == 71 && y == 55 {
                            pixel = eye_shine;
                        }
                    }
                    // Blush Left (x: 48..52, y: 62..65)
                    if (x >= 48 && x <= 52) && (y >= 62 && y <= 64) {
                        pixel = blush_color;
                    }
                    // Blush Right (x: 76..80, y: 62..65)
                    if (x >= 76 && x <= 80) && (y >= 62 && y <= 64) {
                        pixel = blush_color;
                    }
                    // Mouth (x: 62..66, y: 64..66)
                    if (x >= 62 && x <= 66) && (y >= 64 && y <= 65) {
                        pixel = 0x8800;
                    }
                }
                // Pet Feet (Left: x: 50..58, y: 86..92, Right: x: 70..78, y: 86..92)
                else if (y >= 86 && y <= 91) && ((x >= 50 && x <= 58) || (x >= 70 && x <= 78)) {
                    pixel = body_shade;
                }

                // Write pixel (RGB565 little endian) to SRAM VRAM
                let idx = vram_offset + (y * width + x) * 2;
                sram[idx] = (pixel & 0xFF) as u8;
                sram[idx + 1] = ((pixel >> 8) & 0xFF) as u8;
            }
        }
    }

    fn install_sonix_runtime(bus: &mut MemoryBus, edition_name: &str) {
        let mut code: Vec<u8> = vec![
            // 0x00: Initial SP = 0x2001BF00
            0x00, 0xBF, 0x01, 0x20,
            // 0x04: Reset Vector = 0x08000041 (Thumb)
            0x41, 0x00, 0x00, 0x08,
            // 0x08 - 0x3F: Exception Vectors
            0x41, 0x00, 0x00, 0x08, 0x41, 0x00, 0x00, 0x08,
            0x41, 0x00, 0x00, 0x08, 0x41, 0x00, 0x00, 0x08,
            0x41, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x41, 0x00, 0x00, 0x08,
            0x41, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
            0x41, 0x00, 0x00, 0x08, 0x41, 0x00, 0x00, 0x08,
        ];

        while code.len() < 0x40 {
            code.push(0);
        }

        // Instructions at 0x08000040 (Reset Handler & Interactive Engine):
        // 1. Setup r1 = 0x41000000 (UART)
        // 2. Emit greeting to UART
        // 3. Setup r2 = 0x44000000 (GPIO), r3 = 0x20000100 (SRAM Tick)
        // 4. Interactive loop reading buttons and ticking frames
        let mut uart_msg = format!("SONIX SNC73410: Tamagotchi Paradise [{}] Boot Ready\n", edition_name);
        if uart_msg.len() > 64 {
            uart_msg.truncate(64);
        }

        let mut prog = vec![
            // MOVW r1, #0 (0xF240, 0x0100)
            0x40, 0xF2, 0x00, 0x01,
            // MOVT r1, #0x4100 (0xF2C4, 0x1100)
            0xC4, 0xF2, 0x00, 0x11,
        ];

        for b in uart_msg.bytes() {
            prog.push(b);
            prog.push(0x20); // MOVS r0, #b
            prog.extend_from_slice(&[0x08, 0x60]); // STR r0, [r1, #0]
        }

        // Setup GPIO and SRAM tick loop:
        // MOVW r2, #0 (0xF240, 0x0200)
        // MOVT r2, #0x4400 (0xF2C4, 0x4200)
        // MOVW r3, #0x0100 (0xF240, 0x1300)
        // MOVT r3, #0x2000 (0xF2C2, 0x0300)
        let loop_code = [
            0x40, 0xF2, 0x00, 0x02, // MOVW r2, #0
            0xC4, 0xF2, 0x00, 0x42, // MOVT r2, #0x4400
            0x40, 0xF2, 0x00, 0x13, // MOVW r3, #0x0100
            0xC2, 0xF2, 0x00, 0x03, // MOVT r3, #0x2000
            // Loop Start (Offset in prog):
            0x18, 0x68,             // LDR r0, [r3, #0] (Read frame tick)
            0x01, 0x30,             // ADDS r0, #1
            0x18, 0x60,             // STR r0, [r3, #0]
            0x10, 0x68,             // LDR r0, [r2, #0] (Read GPIO Buttons A,B,C)
            0x51, 0x68,             // LDR r1, [r2, #4] (Read Rotary Dial)
            0x00, 0xBF,             // NOP
            0xF8, 0xE7,             // B Loop (-16 bytes)
        ];

        prog.extend_from_slice(&loop_code);
        code.extend_from_slice(&prog);

        bus.boot_rom.load_binary(&code);
    }
}
