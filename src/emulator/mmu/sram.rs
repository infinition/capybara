pub struct InternalSram {
    pub data: Vec<u8>,
    pub mailbox: Vec<u8>,
}

impl Default for InternalSram {
    fn default() -> Self {
        Self::new()
    }
}

impl InternalSram {
    pub fn new() -> Self {
        Self {
            data: vec![0; super::map::SRAM_SIZE],       // SRAM AHB, 128 Ko
            mailbox: vec![0; super::map::MAILBOX_SIZE], // Mailbox RAM, 4 Ko
        }
    }

    pub fn read_u8(&self, offset: usize) -> u8 {
        if offset < self.data.len() {
            self.data[offset]
        } else {
            0
        }
    }

    pub fn write_u8(&mut self, offset: usize, val: u8) {
        if offset < self.data.len() {
            self.data[offset] = val;
        }
    }

    pub fn read_mailbox_u8(&self, offset: usize) -> u8 {
        if offset < self.mailbox.len() {
            self.mailbox[offset]
        } else {
            0
        }
    }

    pub fn write_mailbox_u8(&mut self, offset: usize, val: u8) {
        if offset < self.mailbox.len() {
            self.mailbox[offset] = val;
        }
    }
}
