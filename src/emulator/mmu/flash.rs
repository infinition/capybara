pub struct SpiFlash {
    pub data: Vec<u8>,
    pub size: usize,
    /// Image telle qu'elle a ete chargee, avant toute ecriture du jeu.
    ///
    /// Elle sert de fond aux instantanes : eux ne retiennent que les pages
    /// modifiees, et une restauration remet les autres a cette reference.
    pub reference: Vec<u8>,
    /// Pages de 4 Ko ecrites depuis le chargement. Le jeu n'en salit qu'une
    /// poignee, celles de sa sauvegarde.
    pub pages_salies: std::collections::BTreeSet<usize>,
    /// Compteur d'ecritures. Il ne sert pas au modele : il permet a l'interface
    /// de savoir que la sauvegarde du jeu a bouge, et de la recopier sur le
    /// disque, sans comparer seize mega-octets a chaque image.
    pub revision: u64,
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
            reference: Vec::new(),
            pages_salies: std::collections::BTreeSet::new(),
            revision: 0,
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
            if self.data[offset] != val {
                self.revision = self.revision.wrapping_add(1);
            }
            self.data[offset] = val;
            // Suivre la page permet aux instantanes de ne retenir que ce qui a
            // vraiment change, au lieu de recopier seize mega-octets.
            self.pages_salies.insert(offset / crate::emulator::etat::PAGE_FLASH);
        }
    }

    /// Remet la flash dans l'etat ou le dump a ete charge.
    ///
    /// Seules les pages salies sont a refaire : le jeu n'en ecrit qu'une
    /// poignee, celles de sa sauvegarde.
    pub fn revenir_a_la_reference(&mut self) {
        if self.reference.len() != self.data.len() {
            return;
        }
        for &page in &self.pages_salies {
            let debut = page * 0x1000;
            let fin = (debut + 0x1000).min(self.data.len());
            if debut < fin {
                self.data[debut..fin].copy_from_slice(&self.reference[debut..fin]);
            }
        }
        self.pages_salies.clear();
        self.revision = self.revision.wrapping_add(1);
    }

    /// Fige l'image courante comme reference des instantanes.
    pub fn figer_reference(&mut self) {
        self.reference.clone_from(&self.data);
        self.pages_salies.clear();
    }
}
