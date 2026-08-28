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
#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
    /// Masque d'autorisation d'interruption, une broche par bit.
    pub irq_enable: u32,
    /// Drapeaux d'interruption en attente. Le firmware les lit en 0x1C et les
    /// efface en ecrivant un a la meme position en 0x20.
    pub irq_status: u32,
    cycles: u64,
}

pub const DATA: u32 = 0x00;
pub const DIRECTION: u32 = 0x04;
pub const MODE: u32 = 0x08;
/// Autorisation d'interruption, posee broche par broche par la fenetre
/// bit-band depuis 0x000028C8.
pub const IRQ_ENABLE: u32 = 0x18;
/// Drapeaux d'interruption, lus en 0x0000C132 par le gestionnaire.
pub const IRQ_STATUS: u32 = 0x1C;
/// Effacement des drapeaux, ecrit en 0x0000C160. Un bit a un efface le sien.
pub const IRQ_CLEAR: u32 = 0x20;

/// Broche du signal TE sur le port 1.
pub const TE_PIN: u32 = 10;
/// Interruption du port 1 dans le controleur d'interruptions. Le vecteur 27,
/// en 0x000000AC, pointe sur 0x0000C120, qui lit les drapeaux du port 1, les
/// efface et incremente le compteur de trames en 0x1801C2C0.
pub const PORT1_IRQ: u32 = 27;
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
            irq_enable: 0,
            irq_status: 0,
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
        matches!(offset, DATA | DIRECTION | MODE | IRQ_ENABLE | IRQ_STATUS | IRQ_CLEAR)
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            DATA => (self.sorties & self.direction) | (self.entrees & !self.direction),
            DIRECTION => self.direction,
            MODE => self.mode,
            IRQ_ENABLE => self.irq_enable,
            IRQ_STATUS => self.irq_status,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            DATA => self.sorties = val,
            DIRECTION => self.direction = val,
            MODE => self.mode = val,
            IRQ_ENABLE => self.irq_enable = val,
            IRQ_CLEAR => self.irq_status &= !val,
            _ => {}
        }
    }

    /// Fait avancer le signal periodique du port. Sans lui, le firmware attend
    /// indefiniment un front sur le TE et n'affiche jamais rien.
    ///
    /// Rend vrai quand un front montant autorise vient de poser un drapeau
    /// d'interruption : c'est a l'appelant de la signaler au controleur.
    pub fn tick(&mut self, cycles: u32) -> bool {
        let Some((pin, demi)) = self.periodique else {
            return false;
        };
        // Le compteur est ramene dans la periode plutot que divise : la
        // division en soixante quatre bits revenait a chaque entretien des
        // peripheriques, soit une fois toutes les deux cent cinquante six
        // instructions, pour un signal qui ne bascule qu'a 120 Hz.
        self.cycles = self.cycles.wrapping_add(cycles as u64);
        let periode = demi.saturating_mul(2).max(1);
        if self.cycles >= periode {
            self.cycles %= periode;
        }
        let haut = self.cycles < demi;
        let etait_haut = self.entrees & (1 << pin) != 0;
        if haut {
            self.entrees |= 1 << pin;
        } else {
            self.entrees &= !(1 << pin);
        }
        // Le front montant du TE marque le debut d'une trame. C'est lui que le
        // gestionnaire compte, et lui que la boucle de demarrage attend.
        if haut && !etait_haut && self.irq_enable & (1 << pin) != 0 {
            self.irq_status |= 1 << pin;
            return true;
        }
        false
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
