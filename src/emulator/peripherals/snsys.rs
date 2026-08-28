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
    /// Pose quand l'echeance d'alarme vient d'etre atteinte. C'est le reveil
    /// materiel, qui sur la puce remet le coeur a zero.
    #[serde(default)]
    pub reveil_demande: bool,
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

    /// Comparateur d'alarme. Le firmware y pose l'echeance de son prochain
    /// reveil avant de s'endormir, en `0x00002196`, et la compare au compteur
    /// en `0x00005CAC`.
    pub const ALARME: u32 = 0x230;

    /// Statut de l'alarme. Le firmware y lit les bits 9 et 11 en `0x00005CD2`
    /// pour savoir si c'est elle qui a reveille la console, puis les efface en
    /// `0x00005CE4`.
    pub const STATUT_ALARME: u32 = 0x234;
    /// Bits que le materiel pose quand l'echeance est atteinte.
    pub const ALARME_ECHUE: u32 = 0xA00;
    /// Temoin d'alarme armee, pose par le firmware en `0x00002468` juste apres
    /// avoir ecrit l'echeance. Le materiel l'efface en sonnant, et le firmware
    /// exige de le trouver efface en `0x00005CC2` pour croire au reveil.
    pub const ALARME_ARMEE: u32 = 0x100;

    /// Avance le compteur de secondes du nombre de cycles ecoules, et pose le
    /// statut d'alarme quand l'echeance est atteinte.
    ///
    /// Sans elle, la console programme son reveil en `0x00002196` puis s'endort
    /// pour toujours, et le personnage ne vieillit plus.
    pub fn tick(&mut self, cycles: u32) {
        let avant = self.secondes;
        self.reste += cycles as u64;
        while self.reste >= CYCLES_PAR_SECONDE {
            self.reste -= CYCLES_PAR_SECONDE;
            self.secondes = self.secondes.wrapping_add(1);
        }
        if self.secondes == avant {
            return;
        }
        // Le firmware pose `echeance - 1` dans le comparateur, en `0x0000218C`,
        // et teste `comparateur < compteur` en `0x00005CAC`. L'alarme ne sonne
        // donc qu'une fois le comparateur depasse, pas atteint.
        let echeance = self.regs.get(&Self::ALARME).copied().unwrap_or(0);
        if echeance != 0 && avant <= echeance && self.secondes > echeance {
            self.declencher_reveil();
        }
    }

    /// Marque le reveil : le temoin d'armement retombe, les deux bits de sonnerie
    /// se posent, et le coeur est a rallumer.
    ///
    /// L'appui sur un bouton passe par la meme porte. Le materiel a
    /// vraisemblablement un temoin distinct pour la broche de reveil, qu'on n'a
    /// pas identifie ; mais le firmware n'a qu'un seul chemin de reprise apres
    /// veille profonde, celui de `0x00005CDA`, et il y recalcule la date depuis
    /// le compteur. Rien n'y est fausse.
    pub fn declencher_reveil(&mut self) {
        // Le firmware exige d'abord, en `0x00005CAC`, que le compteur ait
        // depasse le comparateur. Un reveil par bouton arrive avant l'echeance
        // prevue : on ramene donc celle ci a l'instant, ce qui revient a dire
        // que le reveil a bien eu lieu maintenant.
        let echeance = self.regs.entry(Self::ALARME).or_default();
        if *echeance >= self.secondes {
            *echeance = self.secondes.saturating_sub(1);
        }
        let statut = self.regs.entry(Self::STATUT_ALARME).or_default();
        *statut = (*statut & !Self::ALARME_ARMEE) | Self::ALARME_ECHUE;
        self.reveil_demande = true;
    }
}
