/// Controleur de transferts, base 0x4000F000, deux canaux en 0x100 et 0x120.
///
/// Etabli par le pilote en PRAM 0x000044B8 a 0x00004578, qui programme un canal
/// puis pose le bit 0 de son registre de controle :
///
/// ```text
///   canal = descripteur[+8]          ; 0x4000F100
///   canal[0x04] |= 4 | 2 | 0x2000 | 0x80   ; configuration
///   canal[0x14]  = nombre d'unites, champ de 22 bits
///   canal[0x0C]  = destination, ici 0x4000E01C
///   canal[0x08]  = source
///   canal[0x00] |= 1                 ; depart
/// ```
///
/// Le pilote n'attend pas : il met son etat a 3 et rend la main. La fin du
/// transfert arrive par l'IRQ 58, dont le vecteur en 0x00000128 pointe sur
/// 0x10014050, qui charge le descripteur 0x1801C9C0 et le remet a l'etat 1.
/// Sans ce transfert ni son interruption, la boucle de demarrage en 0x967E
/// attend indefiniment.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DmaController {
    pub canaux: [Canal; NB_CANAUX],
    /// Drapeaux de fin, un bit par canal.
    pub status: u32,
    /// Registre de commande commun, relu tel qu'ecrit.
    pub commande: u32,
    /// Interruption de fin a signaler au controleur. Le bus realise la copie
    /// mais ne voit pas le NVIC : c'est le coeur qui la releve apres son pas.
    pub irq_a_lever: bool,
}

#[derive(Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Canal {
    pub ctrl: u32,
    pub config: u32,
    pub source: u32,
    pub destination: u32,
    pub reserve: u32,
    pub compte: u32,
}

pub const NB_CANAUX: usize = 2;
/// Offset du premier canal dans la page.
pub const CANAL0: u32 = 0x100;
/// Ecart entre deux canaux.
pub const PAS_CANAL: u32 = 0x20;

/// Registres communs a la page, releves dans le gestionnaire d'interruption
/// 0x00003E44 : il lit les drapeaux en 0x00, un bit par canal, acquitte en
/// 0x08 puis repose deux bits de commande en 0x10.
pub const STATUS: u32 = 0x00;
pub const ACQUIT: u32 = 0x08;
pub const COMMANDE: u32 = 0x10;

pub const CTRL: u32 = 0x00;
pub const CONFIG: u32 = 0x04;
pub const SOURCE: u32 = 0x08;
pub const DESTINATION: u32 = 0x0C;
pub const RESERVE: u32 = 0x10;
pub const COMPTE: u32 = 0x14;
/// Bit de depart du transfert, dans le registre de controle du canal.
pub const DEPART: u32 = 0x1;
/// Masque du nombre d'unites, un champ de 22 bits pose par BFI en 0x000044F6.
pub const MASQUE_COMPTE: u32 = 0x003F_FFFF;
/// Taille d'une unite transferee, en octets.
///
/// Le seul transfert observe pousse 16384 unites depuis 0x180142A6 vers le
/// registre de donnees de l'afficheur. Deux faits imposent le demi-mot : la
/// source n'est alignee que sur deux octets, et 16384 demi-mots font exactement
/// les 32768 octets d'une image de 128 x 128 en RGB565. La largeur est
/// probablement codee dans le controle ou la configuration du canal, mais aucun
/// autre transfert n'a encore permis de le verifier.
pub const LARGEUR_UNITE: u32 = 2;

/// Interruption de fin de transfert. Le vecteur 58 est le seul active au dela
/// de 32, et son gestionnaire s'adresse au descripteur de ce pilote.
pub const IRQ: u32 = 58;

/// Copie demandee par un canal.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Transfert {
    pub canal: usize,
    pub source: u32,
    pub destination: u32,
    /// Nombre d'unites de `LARGEUR_UNITE` octets.
    pub unites: u32,
}

impl Default for DmaController {
    fn default() -> Self {
        Self {
            canaux: [Canal::default(); NB_CANAUX],
            status: 0,
            commande: 0,
            irq_a_lever: false,
        }
    }
}

impl DmaController {
    /// Rend l'index du canal vise par un offset de la page, s'il y en a un.
    fn canal_de(offset: u32) -> Option<(usize, u32)> {
        if offset < CANAL0 {
            return None;
        }
        let index = ((offset - CANAL0) / PAS_CANAL) as usize;
        if index >= NB_CANAUX {
            return None;
        }
        Some((index, (offset - CANAL0) % PAS_CANAL))
    }

    pub fn handles(offset: u32) -> bool {
        matches!(offset, STATUS | ACQUIT | COMMANDE)
            || Self::canal_de(offset).is_some_and(|(_, r)| r <= COMPTE)
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            STATUS => return self.status,
            COMMANDE => return self.commande,
            ACQUIT => return 0,
            _ => {}
        }
        let Some((i, reg)) = Self::canal_de(offset) else {
            return 0;
        };
        let c = &self.canaux[i];
        match reg {
            // Le bit de depart retombe des la fin du transfert, instantanee ici.
            CTRL => c.ctrl & !DEPART,
            CONFIG => c.config,
            SOURCE => c.source,
            DESTINATION => c.destination,
            RESERVE => c.reserve,
            COMPTE => c.compte,
            _ => 0,
        }
    }

    /// Rend la description d'une copie quand l'ecriture lance un canal.
    #[must_use]
    pub fn write_reg(&mut self, offset: u32, val: u32) -> Option<Transfert> {
        match offset {
            STATUS => return None,
            // Un bit a un efface le drapeau du canal correspondant.
            ACQUIT => {
                self.status &= !val;
                return None;
            }
            COMMANDE => {
                self.commande = val;
                return None;
            }
            _ => {}
        }
        let (i, reg) = Self::canal_de(offset)?;
        let c = &mut self.canaux[i];
        match reg {
            CTRL => {
                c.ctrl = val;
                if val & DEPART != 0 {
                    self.status |= 1 << i;
                    let c = self.canaux[i];
                    return Some(Transfert {
                        canal: i,
                        source: c.source,
                        destination: c.destination,
                        unites: c.compte & MASQUE_COMPTE,
                    });
                }
            }
            CONFIG => c.config = val,
            SOURCE => c.source = val,
            DESTINATION => c.destination = val,
            RESERVE => c.reserve = val,
            COMPTE => c.compte = val,
            _ => {}
        }
        None
    }
}
