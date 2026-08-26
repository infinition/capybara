pub struct BootRom {
    pub data: Vec<u8>,
    pub is_hidden: bool,
}

impl Default for BootRom {
    fn default() -> Self {
        Self::new()
    }
}

impl BootRom {
    pub fn new() -> Self {
        let mut rom = vec![0; 64 * 1024]; // 64 KB Sonix Boot ROM

        // Setup default Cortex-M reset vector at 0x00000000 / 0x08000000:
        // Word 0: Initial SP = 0x20010000 (Top of SRAM)
        // Word 1: Reset Vector = 0x08000041 (Thumb mode address)
        let sp_bytes = 0x2001_0000_u32.to_le_bytes();
        let reset_bytes = 0x0800_0041_u32.to_le_bytes();
        rom[0..4].copy_from_slice(&sp_bytes);
        rom[4..8].copy_from_slice(&reset_bytes);

        // Entry point instructions at 0x40 (Reset Handler):
        // 0x08000040: NOP (0xBF00)
        // 0x08000042: MOVS r0, #1 (0x2001)
        // 0x08000044: B 0x08000044 (0xE7FE - loop)
        rom[0x40] = 0x00;
        rom[0x41] = 0xBF;
        rom[0x42] = 0x01;
        rom[0x43] = 0x20;
        rom[0x44] = 0xFE;
        rom[0x45] = 0xE7;

        Self {
            data: rom,
            is_hidden: false,
        }
    }

    pub fn load_binary(&mut self, bytes: &[u8]) {
        let len = bytes.len().min(self.data.len());
        self.data[..len].copy_from_slice(&bytes[..len]);
    }

    pub fn read_u8(&self, offset: usize) -> u8 {
        if self.is_hidden && offset >= 0x8000 && offset <= 0xBFFF {
            // Mirror start of ROM when hidden
            return self.data[offset & 0x3FFF];
        }

        if offset < self.data.len() {
            self.data[offset]
        } else {
            0
        }
    }
}
