pub struct SpiFlash {
    pub data: Vec<u8>,
    pub size: usize,
}

impl Default for SpiFlash {
    fn default() -> Self {
        Self::new(16 * 1024 * 1024) // 16 MB (128 Mbit)
    }
}

impl SpiFlash {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0xFF; size],
            size,
        }
    }

    pub fn load_binary(&mut self, offset: usize, bytes: &[u8]) {
        let end = (offset + bytes.len()).min(self.size);
        if offset < self.size {
            let copy_len = end - offset;
            self.data[offset..end].copy_from_slice(&bytes[..copy_len]);
        }
    }

    pub fn read_u8(&self, offset: usize) -> u8 {
        if offset < self.size {
            self.data[offset]
        } else {
            0xFF
        }
    }

    pub fn read_u16(&self, offset: usize) -> u16 {
        let b0 = self.read_u8(offset) as u16;
        let b1 = self.read_u8(offset + 1) as u16;
        b0 | (b1 << 8)
    }

    pub fn read_u32(&self, offset: usize) -> u32 {
        let b0 = self.read_u8(offset) as u32;
        let b1 = self.read_u8(offset + 1) as u32;
        let b2 = self.read_u8(offset + 2) as u32;
        let b3 = self.read_u8(offset + 3) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    pub fn write_u8(&mut self, offset: usize, val: u8) {
        if offset < self.size {
            self.data[offset] = val;
        }
    }
}
