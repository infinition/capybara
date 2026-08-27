/// RAM programme (PRAM) mappee a 0x00000000.
///
/// Le bootrom y recopie le code utilisateur dechiffre depuis la flash, puis
/// saute dessus. C'est donc la que vit la table de vecteurs du firmware, ce qui
/// explique les adresses de handlers tres basses vues dans le dump.
pub struct Pram {
    pub data: Vec<u8>,
    pub loaded: bool,
}

pub const PRAM_SIZE: usize = 64 * 1024;

impl Default for Pram {
    fn default() -> Self {
        Self::new()
    }
}

impl Pram {
    pub fn new() -> Self {
        Self { data: vec![0; PRAM_SIZE], loaded: false }
    }

    pub fn load(&mut self, bytes: &[u8]) {
        let len = bytes.len().min(self.data.len());
        self.data[..len].copy_from_slice(&bytes[..len]);
        self.data[len..].fill(0);
        self.loaded = true;
    }

    pub fn clear(&mut self) {
        self.data.fill(0);
        self.loaded = false;
    }

    pub fn read_u8(&self, offset: usize) -> u8 {
        self.data.get(offset).copied().unwrap_or(0)
    }

    pub fn write_u8(&mut self, offset: usize, val: u8) {
        if let Some(b) = self.data.get_mut(offset) {
            *b = val;
        }
    }

    pub fn read_u32(&self, offset: usize) -> u32 {
        if offset + 4 <= self.data.len() {
            u32::from_le_bytes(self.data[offset..offset + 4].try_into().unwrap())
        } else {
            0
        }
    }
}
