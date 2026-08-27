/// Port d'entrees-sorties, registre de donnees a l'offset 0x00.
///
/// Le firmware adresse ses broches par la fenetre bit-band. Un identifiant
/// encode `port = id >> 4` et `pin = id & 15`, cinq ports etant resolus par une
/// table en SRAM. Le port 2 a son registre de donnees en 0x4001A000.
///
/// Les entrees sont a resistance de tirage : au repos elles se lisent hautes,
/// et un appui les tire vers le bas. C'est pour cela que l'etat par defaut est
/// tout a un. Le firmware lit les broches 0x20 et 0x21, les combine en
/// `pin0 | (pin1 << 1)` et attend la valeur 3, donc les deux au repos.
///
/// Seul le registre de donnees est modelise : les autres offsets de la page
/// restent visibles dans la trace MMIO, leur role n'etant pas etabli.
pub struct GpioPort {
    /// Etat lu sur les broches. Un bit a un correspond a une entree au repos.
    pub entrees: u32,
}

pub const DATA: u32 = 0x00;

impl Default for GpioPort {
    fn default() -> Self {
        Self { entrees: 0xFFFF_FFFF }
    }
}

impl GpioPort {
    pub fn handles(offset: u32) -> bool {
        offset == DATA
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            DATA => self.entrees,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, _offset: u32, _val: u32) {}

    /// Tire une broche vers le bas, ce que fait un appui sur un bouton.
    pub fn appuyer(&mut self, pin: u32) {
        self.entrees &= !(1 << pin);
    }

    /// Relache une broche, qui remonte par sa resistance de tirage.
    pub fn relacher(&mut self, pin: u32) {
        self.entrees |= 1 << pin;
    }
}
