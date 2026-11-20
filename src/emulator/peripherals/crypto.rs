//! Sonix AES memory-to-memory engine. My Lab uses AES-256 ECB to load code.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Crypto {
    ctrl: u32,
    source: u32,
    destination: u32,
    length_minus_block: u32,
    key: [u32; 8],
}

impl Crypto {
    fn key_index(offset: u32) -> Option<usize> {
        match offset {
            0x1c..=0x28 => Some(((offset - 0x1c) / 4) as usize),
            0x40..=0x44 => Some(4 + ((offset - 0x40) / 4) as usize),
            0x80..=0x84 => Some(6 + ((offset - 0x80) / 4) as usize),
            _ => None,
        }
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            0 => self.ctrl,
            0x10 => self.source,
            0x14 => self.destination,
            0x18 => self.length_minus_block,
            _ => Self::key_index(offset).map_or(0, |index| self.key[index]),
        }
    }

    pub fn write_reg(&mut self, offset: u32, value: u32) -> Option<(u32, u32, u32, [u8; 32])> {
        match offset {
            0 => {
                self.ctrl = value & !1;
                // The driver preserves the AES-256 select bit while starting ECB.
                if value == 0x6000_0001 {
                    let mut key = [0; 32];
                    for (bytes, word) in key.chunks_exact_mut(4).zip(self.key) {
                        bytes.copy_from_slice(&word.to_le_bytes());
                    }
                    return Some((self.source, self.destination, self.length_minus_block.saturating_add(16), key));
                }
            }
            0x10 => self.source = value,
            0x14 => self.destination = value,
            0x18 => self.length_minus_block = value,
            _ => {
                if let Some(index) = Self::key_index(offset) {
                    self.key[index] = value;
                }
            }
        }
        None
    }
}
