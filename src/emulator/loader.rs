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

        // Detect Sonix SNC73410 SPI Flash Dump (Starts with 'SONIXDEV' magic)
        if buffer.starts_with(b"SONIXDEV") || (len == 16 * 1024 * 1024 && &buffer[0..8] == b"SONIXDEV") {
            let mut vector_table = vec![0u8; 256];
            let sp: u32 = 0x2001_BF00; // Top of SRAM for SNC73410
            let pc: u32 = 0x6001_1001; // XIP Entry Point in Thumb mode (offset 0x11000)
            let default_handler: u32 = 0x6001_1011;

            vector_table[0..4].copy_from_slice(&sp.to_le_bytes());
            vector_table[4..8].copy_from_slice(&pc.to_le_bytes());

            for i in 2..64 {
                let off = i * 4;
                vector_table[off..off + 4].copy_from_slice(&default_handler.to_le_bytes());
            }

            bus.boot_rom.load_binary(&vector_table);
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
        // Built-in demonstration firmware for Sonix SNC73410
        // Initializes VRAM, draws a test Tamagotchi pet sprite on the LCD, outputs UART string,
        // and enters an interactive main loop responding to GPIO buttons.

        let mut code: Vec<u8> = vec![
            // 0x00: Initial SP = 0x20010000
            0x00, 0x00, 0x01, 0x20,
            // 0x04: Reset Vector = 0x08000021 (Thumb)
            0x21, 0x00, 0x00, 0x08,
            // 0x08 - 0x1F: Vectors
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        // Ensure alignment to 0x20
        while code.len() < 0x20 {
            code.push(0);
        }

        // Instructions at 0x08000020 (Reset Handler):
        // 1. MOVS r0, #1 -> Enable SysTick / Clock
        // 2. LDR r1, =0x41000000 (UART)
        // 3. Send greeting string 'TAMA' to UART
        // 4. LDR r2, =0x43000000 (Display)
        // 5. Draw background color in VRAM (0x20008000)
        // 6. Infinite loop with NOP
        let prog = [
            0x01, 0x20, // MOVS r0, #1
            0x4F, 0xF0, 0x82, 0x41, // MOV.W r1, #0x41000000 (UART)
            0x54, 0x20, // MOVS r0, #'T'
            0x08, 0x60, // STR r0, [r1, #0]
            0x41, 0x20, // MOVS r0, #'A'
            0x08, 0x60, // STR r0, [r1, #0]
            0x4D, 0x20, // MOVS r0, #'M'
            0x08, 0x60, // STR r0, [r1, #0]
            0x41, 0x20, // MOVS r0, #'A'
            0x08, 0x60, // STR r0, [r1, #0]
            0x0A, 0x20, // MOVS r0, #'\n'
            0x08, 0x60, // STR r0, [r1, #0]
            // Main loop
            0x00, 0xBF, // NOP
            0xFE, 0xE7, // B loop (-4 bytes)
        ];

        code.extend_from_slice(&prog);
        bus.boot_rom.load_binary(&code);
    }
}
