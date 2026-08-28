use std::collections::BTreeMap;

/// Nombre de cycles du coeur pour une seconde reelle.
///
/// Le firmware programme lui-meme son SysTick a 95999 pour obtenir une
/// milliseconde : le coeur tourne donc a 96 MHz, et c'est cette cadence qui
/// donne la seconde du compteur d'horloge.
pub const CYCLES_PAR_SECONDE: u64 = 96_000_000;

/// Registres d'horloge et de PLL de la zone systeme SN_SYS0 (0x45000000),
/// hors fusibles FEUSE qui ont leur propre modele.
///
/// Le firmware ecrit une sequence de verrouillage PLL puis attend le bit 8 du
/// statut (offset 0x08). Les valeurs ecrites sont memorisees pour que le
/// read-modify-write sur l'offset 0x00 retrouve l'etat precedent.
///
/// La page porte aussi le compteur de temps de la console, en `0x304`. C'est
/// un compteur de secondes libre : le firmware ne l'ecrit jamais, il lui ajoute
/// un decalage garde en memoire pour obtenir la date affichee. Tant qu'il reste
/// fige, le calendrier du jeu ne bouge pas d'une seconde, l'oeuf n'eclot pas et
/// les jauges ne descendent pas.
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnSysRegisters {
    regs: BTreeMap<u32, u32>,
    pll_locked: bool,
    /// Secondes ecoulees, telles que le firmware les lit en `0x45000304`.
    #[serde(default)]
    pub secondes: u32,
    /// Cycles accumules depuis la derniere seconde entiere.
    #[serde(default)]
    reste: u64,
}

impl SnSysRegisters {
    /// Compteur de secondes de la console, lu par la couche date du firmware en
    /// `0x00003754`, qui n'est qu'un `ldr r0, [0x45000304]`.
    pub const COMPTEUR_SECONDES: u32 = 0x304;

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            // Statut PLL : bit 4 = verrouillage initial (poll LSR #4), bit 6 =
            // verrouillage final apres reconfiguration (poll LSR #6).
            0x08 => {
                if self.pll_locked {
                    0x10 | 0x40
                } else {
                    0
                }
            }
            // Selection de la source d'horloge. Le firmware ecrit la source
            // demandee dans les bits 2:0, puis verifie que le materiel la
            // recopie dans les bits 6:4 avant de continuer. Sans cet echo il
            // appelle panic(4).
            0x0C => {
                let v = self.regs.get(&0x0C).copied().unwrap_or(0);
                (v & !0x70) | ((v & 0x07) << 4)
            }
            Self::COMPTEUR_SECONDES => self.secondes,
            _ => self.regs.get(&offset).copied().unwrap_or(0),
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        // 0x04 recoit la valeur magique de deblocage avant la configuration.
        if offset == 0x04 && val == 0xA55A_0000 {
            self.pll_locked = true;
        }
        // Le firmware ne le fait pas, mais un compteur qu'on peut poser reste
        // le seul moyen de rejouer une console qui a deja vecu.
        if offset == Self::COMPTEUR_SECONDES {
            self.secondes = val;
            self.reste = 0;
        }
        self.regs.insert(offset, val);
    }

    /// Avance le compteur de secondes du nombre de cycles ecoules.
    pub fn tick(&mut self, cycles: u32) {
        self.reste += cycles as u64;
        while self.reste >= CYCLES_PAR_SECONDE {
            self.reste -= CYCLES_PAR_SECONDE;
            self.secondes = self.secondes.wrapping_add(1);
        }
    }
}
