/// Convertisseur de la mesure de pile, page 0x4003A000.
///
/// La figure 4-1 annonce le chien de garde a cette adresse, mais le firmware y
/// lance des conversions. Le gestionnaire de l'IRQ 9, en 0x10078774, en extrait
/// l'echantillon par `UBFX r0, r0, #6, #10` sur le registre 0x08 : dix bits
/// utiles, cales a partir du rang 6.
///
/// Sequence de depart relevee en 0x000051F6 a 0x000052D8, en
/// lecture-modification-ecriture :
///
/// ```text
///   ctrl   |= 0x8000        ; sur 0x00
///   cmd    |= 0x8000        ; sur 0x04
///   cmd    |= 1             ; depart
/// ```
///
/// La pile est mesuree sur P0.6. Sans ce convertisseur ni son interruption, le
/// firmware ne voit aucune tension, pose son drapeau de pile faible dans l'etat
/// sauvegarde et s'eteint apres avoir affiche son message.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AdcPile {
    pub ctrl: u32,
    pub commande: u32,
    /// Echantillon courant, sur dix bits.
    pub echantillon: u32,
    /// Fin de conversion a signaler au controleur d'interruptions.
    pub irq_a_lever: bool,
    /// Le convertisseur a ete lance et enchaine ses conversions.
    pub en_marche: bool,
    cycles: u64,
}

pub const CTRL: u32 = 0x00;
pub const COMMANDE: u32 = 0x04;
pub const RESULTAT: u32 = 0x08;
/// Bit de depart de conversion, dans le registre de commande.
pub const DEPART: u32 = 0x1;
/// Rang du premier bit utile dans le registre de resultat.
pub const DECALAGE_RESULTAT: u32 = 6;
/// Interruption de fin de conversion.
pub const IRQ: u32 = 9;
/// Duree d'une conversion, en cycles du coeur.
///
/// Le firmware ne relance pas le convertisseur : il scrute un drapeau que son
/// gestionnaire d'interruption entretient, et accumule une dizaine
/// d'echantillons avant de conclure. Le convertisseur enchaine donc ses
/// conversions de lui-meme.
pub const DUREE_CONVERSION: u64 = 20_000;

/// Echantillon d'une pile pleine.
///
/// Le firmware compare la tension calculee au seuil 0x23332 en 0x10003754 et
/// se declare en fin de pile en dessous. Cette valeur est celle qui passe le
/// seuil avec de la marge ; elle est surchargeable par SONIX_PILE.
pub const PILE_PLEINE: u32 = 0x03FF;

impl Default for AdcPile {
    fn default() -> Self {
        Self {
            ctrl: 0,
            commande: 0,
            echantillon: std::env::var("SONIX_PILE")
                .ok()
                .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                .unwrap_or(PILE_PLEINE),
            irq_a_lever: false,
            en_marche: false,
            cycles: 0,
        }
    }
}

impl AdcPile {
    pub fn handles(offset: u32) -> bool {
        matches!(offset, CTRL | COMMANDE | RESULTAT)
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            CTRL => self.ctrl,
            // Le bit de depart retombe des la fin de conversion, instantanee ici.
            COMMANDE => self.commande & !DEPART,
            RESULTAT => (self.echantillon & 0x3FF) << DECALAGE_RESULTAT,
            _ => 0,
        }
    }

    /// Fait avancer la conversion. Rend vrai a chaque echantillon acheve.
    pub fn tick(&mut self, cycles: u32) -> bool {
        if !self.en_marche {
            return false;
        }
        self.cycles += cycles as u64;
        if self.cycles < DUREE_CONVERSION {
            return false;
        }
        self.cycles -= DUREE_CONVERSION;
        true
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            CTRL => self.ctrl = val,
            COMMANDE => {
                self.commande = val;
                if val & DEPART != 0 {
                    self.en_marche = true;
                    self.cycles = 0;
                    self.irq_a_lever = true;
                }
            }
            _ => {}
        }
    }
}
