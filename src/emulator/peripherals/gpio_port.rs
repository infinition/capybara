/// Port d'entrees-sorties. Trois registres sont etablis par la trace :
/// donnees en 0x00, direction en 0x04, mode en 0x08.
///
/// Le firmware adresse ses broches par la fenetre bit-band. Un identifiant
/// encode `port = id >> 4` et `pin = id & 15`, cinq ports etant resolus par une
/// table en SRAM. Les ports 0 a 2 occupent 0x40018000, 0x40019000 et
/// 0x4001A000.
///
/// Brochage releve sur la console (tama-para-research, hardware/testpads.txt) :
///
/// ```text
///   P0.4  RESET de l'ecran        P1.0 a P1.4  flash SPI
///   P0.5  commande/donnee ecran   P1.5  CS de l'ecran
///   P0.7  retroeclairage          P1.6  SCLK de l'ecran
///   P0.8  bouton molette          P1.8  MOSI de l'ecran
///   P0.9  bouton A                P1.9  alimentation de l'ecran
///   P0.10 bouton C                P1.10 TE de l'ecran
///   P0.11 bouton B                P2.0 et P2.1  encodeur
/// ```
///
/// La direction decide de ce que rend une lecture. Une broche en sortie relit
/// son verrou, ce qui est indispensable : le firmware pilote ses sorties par
/// bit-band, et le bus traduit cela en lecture puis ecriture du mot entier.
/// Une broche en entree rend le niveau exterieur, haut au repos par sa
/// resistance de tirage, et tire bas par un appui.
pub struct GpioPort {
    /// Niveau impose de l'exterieur. Un bit a un correspond a une entree au
    /// repos.
    pub entrees: u32,
    /// Verrou de sortie, dernier mot ecrit sur le registre de donnees.
    pub sorties: u32,
    /// Registre de direction. Un bit a un designe une sortie.
    pub direction: u32,
    /// Registre de mode, deux bits par broche. Son role exact n'est pas etabli,
    /// mais il doit se relire tel qu'ecrit : le firmware le construit broche par
    /// broche en lecture-modification-ecriture.
    pub mode: u32,
    /// Broche entretenue par un signal periodique exterieur, et sa demi-periode
    /// en cycles. C'est ainsi qu'arrive le TE de l'ecran, que le firmware attend
    /// pour se synchroniser sur les trames.
    pub periodique: Option<(u32, u64)>,
    cycles: u64,
}

pub const DATA: u32 = 0x00;
pub const DIRECTION: u32 = 0x04;
pub const MODE: u32 = 0x08;

/// Broche du signal TE sur le port 1.
pub const TE_PIN: u32 = 10;
/// Demi-periode du TE, en cycles du coeur.
///
/// Le SysTick est arme a 95999, soit une milliseconde a 96 MHz. Une trame a
/// 60 Hz vaut donc 1 600 000 cycles, et le creneau en fait la moitie.
pub const TE_DEMI_PERIODE: u64 = 800_000;

impl Default for GpioPort {
    fn default() -> Self {
        Self {
            entrees: 0xFFFF_FFFF,
            sorties: 0xFFFF_FFFF,
            direction: 0,
            mode: 0,
            periodique: None,
            cycles: 0,
        }
    }
}

impl GpioPort {
    /// Port 1, qui porte le TE de l'ecran en plus de ses sorties.
    pub fn port1() -> Self {
        Self { periodique: Some((TE_PIN, TE_DEMI_PERIODE)), ..Self::default() }
    }

    pub fn handles(offset: u32) -> bool {
        matches!(offset, DATA | DIRECTION | MODE)
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            DATA => (self.sorties & self.direction) | (self.entrees & !self.direction),
            DIRECTION => self.direction,
            MODE => self.mode,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            DATA => self.sorties = val,
            DIRECTION => self.direction = val,
            MODE => self.mode = val,
            _ => {}
        }
    }

    /// Fait avancer le signal periodique du port. Sans lui, le firmware attend
    /// indefiniment un front sur le TE et n'affiche jamais rien.
    pub fn tick(&mut self, cycles: u32) {
        let Some((pin, demi)) = self.periodique else {
            return;
        };
        self.cycles = self.cycles.wrapping_add(cycles as u64);
        if (self.cycles / demi) % 2 == 0 {
            self.entrees |= 1 << pin;
        } else {
            self.entrees &= !(1 << pin);
        }
    }

    /// Tire une broche vers le bas, ce que fait un appui sur un bouton.
    pub fn appuyer(&mut self, pin: u32) {
        self.entrees &= !(1 << pin);
    }

    /// Relache une broche, qui remonte par sa resistance de tirage.
    pub fn relacher(&mut self, pin: u32) {
        self.entrees |= 1 << pin;
    }
}
