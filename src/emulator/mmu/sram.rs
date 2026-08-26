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
            data: vec![0; 128 * 1024],    // 128 KB SRAM / PRAM
            mailbox: vec![0; 16 * 1024],  // 16 KB Mailbox RAM
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
