/// Accelerateur materiel de somme de controle, base 0x40038000.
///
/// La figure 4-1 du datasheet place UART0 a cette page. Le firmware y fait tout
/// autre chose : il y programme une adresse source, une longueur et un polynome,
/// puis lit un resultat. La sequence relevee est sans ambiguite :
///
/// ```text
///   STR 0xA001,     [base, #0x18]   ; polynome, forme reflechie
///   STR 0xF0,       [base, #0x14]   ; configuration
///   STR 0x18005DA0, [base, #0x04]   ; adresse source
///   STR 0x00000FFC, [base, #0x08]   ; longueur
///   STR 1,          [base, #0x0C]
///   STR 1,          [base, #0x10]
///   STR ctrl|0x10,  [base, #0x00]   ; depart
///   LDR resultat,   [base, #0x1C]
/// ```
///
/// `0xA001` est la forme reflechie de `0x8005`, donc CRC-16/ARC : initialisation
/// a zero, entree et sortie reflechies, sans ou exclusif final. Verifie sur les
/// pages de sauvegarde des cinq editions, dont l'en-tete porte la somme attendue.
///
/// C'est ce calcul qui valide la sauvegarde. Tant que le resultat restait a
/// zero, le firmware rejetait ses deux emplacements et affichait la chaine de
/// repli, "unsupport chip, please check your flash vender", qui n'a rien
/// a voir avec le fabricant de la flash.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ChecksumUnit {
    pub ctrl: u32,
    pub source: u32,
    pub length: u32,
    pub config: u32,
    pub polynome: u32,
    pub resultat: u32,
    pub mode_a: u32,
    pub mode_b: u32,
    /// Nombre de calculs effectues, pour le diagnostic.
    pub calculs: u64,
    /// compteurs (non sauvegardes).
    #[serde(skip)]
    pub com: StatsCrc,
}

/// Compteurs du modele generalise .
#[derive(Clone, Copy, Debug, Default)]
pub struct StatsCrc {
    /// Calculs par largeur (Order + 1) : 16 bits, 32 bits, autre.
    pub crc16: u64,
    pub crc32: u64,
    pub autres: u64,
    pub additions: u64,
    /// Calculs avec R_LINE_NUM > 1 (balayage en lignes non modelise).
    pub lignes_multiples: u64,
    /// Longueurs non multiples de 4 (traitement materiel inconnu).
    pub longueurs_non_alignees: u64,
}

/// Parametres du calcul generalise, d'apres CRC_SET et CTRL :
/// SDK snc_drv_crc.c CRC_Start (Init / XOR a 0 ou tout a 1, Order = largeur
/// - 1, polynome sous forme reflechie comme les six de crc_poly[]) et
/// CRC_Adder_Start (Mode = 1 : somme des octets).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Generalise {
    pub largeur: u32,
    pub init_uns: bool,
    pub xor_uns: bool,
    pub additionneur: bool,
    pub polynome32: u32,
}

/// Bits de CTRL : Mode (b0), CRC_EN (b4), CRC_OK (b9, lu par CRC_GetStatus).
pub const CTRL_MODE: u32 = 0x01;
pub const CTRL_OK: u32 = 1 << 9;

pub const CTRL: u32 = 0x00;
pub const SOURCE: u32 = 0x04;
pub const LENGTH: u32 = 0x08;
pub const MODE_A: u32 = 0x0C;
pub const MODE_B: u32 = 0x10;
pub const CONFIG: u32 = 0x14;
pub const POLY: u32 = 0x18;
pub const RESULT: u32 = 0x1C;
/// Bit de lancement dans le registre de controle.
pub const CTRL_START: u32 = 0x10;
/// Polynome par defaut tant que le firmware n'en a pas programme.
pub const POLY_ARC_REFLECHI: u16 = 0xA001;

impl Default for ChecksumUnit {
    fn default() -> Self {
        Self {
            ctrl: 0,
            source: 0,
            length: 0,
            config: 0,
            polynome: POLY_ARC_REFLECHI as u32,
            resultat: 0,
            mode_a: 0,
            mode_b: 0,
            calculs: 0,
            com: StatsCrc::default(),
        }
    }
}

/// Calcul demande, a executer par le bus qui seul voit la memoire.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Calcul {
    pub source: u32,
    pub length: u32,
    pub polynome: u16,
    /// Parametres du calcul ; `None` = calcul historique CRC-16 reflechi
    /// (ancien format de sauvegarde).
    #[serde(default)]
    pub generalise: Option<Generalise>,
}

impl ChecksumUnit {
    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            // CRC_EN reste tel qu'ecrit ; CRC_OK (b9) dit le calcul termine
            // (instantane ici). CRC_GetStatus remet lui-meme EN a 0.
            CTRL => self.ctrl,
            SOURCE => self.source,
            LENGTH => self.length,
            MODE_A => self.mode_a,
            MODE_B => self.mode_b,
            CONFIG => self.config,
            POLY => self.polynome,
            RESULT => self.resultat,
            _ => 0,
        }
    }

    #[must_use]
    pub fn write_reg(&mut self, offset: u32, val: u32) -> Option<Calcul> {
        match offset {
            SOURCE => self.source = val,
            LENGTH => self.length = val,
            MODE_A => self.mode_a = val,
            MODE_B => self.mode_b = val,
            CONFIG => self.config = val,
            POLY => self.polynome = val,
            CTRL => {
                // SVD CRC_EN : "Need to reset to 0 before setting 1" : le
                // calcul part sur le front montant de EN. CRC_OK retombe au
                // depart et se pose a la fin (calcul instantane).
                let avant = self.ctrl;
                self.ctrl = val & !CTRL_OK | (avant & CTRL_OK);
                if val & CTRL_START != 0 && avant & CTRL_START == 0 {
                    self.ctrl &= !CTRL_OK;
                    if self.length != 0 {
                        self.calculs += 1;
                        let ordre = (self.config >> 4) & 0x1F;
                        let g = Generalise {
                            largeur: ordre + 1,
                            init_uns: self.config & 1 != 0,
                            xor_uns: self.config & 2 != 0,
                            additionneur: val & CTRL_MODE != 0,
                            polynome32: self.polynome,
                        };
                        let s = &mut self.com;
                        if g.additionneur {
                            s.additions += 1;
                        } else {
                            match g.largeur {
                                16 => s.crc16 += 1,
                                32 => s.crc32 += 1,
                                _ => s.autres += 1,
                            }
                        }
                        if self.mode_a > 1 {
                            s.lignes_multiples += 1;
                        }
                        if self.length % 4 != 0 {
                            s.longueurs_non_alignees += 1;
                        }
                        self.ctrl |= CTRL_OK;
                        return Some(Calcul {
                            source: self.source,
                            length: self.length,
                            polynome: self.polynome as u16,
                            generalise: Some(g),
                        });
                    }
                    self.ctrl |= CTRL_OK;
                }
            }
            _ => {}
        }
        None
    }

    /// CRC reflechi de largeur quelconque (1..=32), ou somme des octets en
    /// mode additionneur .
    pub fn calculer_generalise(octets: impl Iterator<Item = u8>, g: &Generalise) -> u32 {
        if g.additionneur {
            return octets.fold(0u32, |s, b| s.wrapping_add(b as u32));
        }
        let largeur = g.largeur.clamp(1, 32);
        let masque: u32 = if largeur == 32 { u32::MAX } else { (1u32 << largeur) - 1 };
        let poly = g.polynome32 & masque;
        let mut crc: u32 = if g.init_uns { masque } else { 0 };
        for b in octets {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ poly } else { crc >> 1 };
            }
            crc &= masque;
        }
        if g.xor_uns {
            crc ^= masque;
        }
        crc
    }

    /// CRC 16 bits a decalage vers la droite, le polynome etant deja reflechi.
    pub fn crc16(octets: impl Iterator<Item = u8>, polynome: u16) -> u16 {
        let mut crc: u16 = 0;
        for b in octets {
            crc ^= b as u16;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ polynome } else { crc >> 1 };
            }
        }
        crc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_arc_sur_un_vecteur_connu() {
        // CRC-16/ARC de "123456789" vaut 0xBB3D.
        let v = ChecksumUnit::crc16(b"123456789".iter().copied(), POLY_ARC_REFLECHI);
        assert_eq!(v, 0xBB3D);
    }

    /// les six types de crc_poly[] du SDK (snc_drv_crc.h), Order =
    /// largeur - 1 comme CRC_Start, sur le vecteur de controle "123456789".
    #[test]
    fn crc_generalise_types_du_sdk() {
        let g = |poly: u32, init: bool, xor: bool| Generalise {
            largeur: 32 - poly.leading_zeros(),
            init_uns: init,
            xor_uns: xor,
            additionneur: false,
            polynome32: poly,
        };
        let v = |g: Generalise| ChecksumUnit::calculer_generalise(b"123456789".iter().copied(), &g);
        // 0 CRC-16/ARC : identique au chemin historique.
        assert_eq!(v(g(0xA001, false, false)), 0xBB3D);
        assert_eq!(
            v(g(0xA001, false, false)),
            ChecksumUnit::crc16(b"123456789".iter().copied(), 0xA001) as u32
        );
        // 1 init tout a 1 : CRC-16/MODBUS.
        assert_eq!(v(g(0xA001, true, false)), 0x4B37);
        // 2 DNP sans XOR final : complement du controle CRC-16/DNP 0xEA82.
        assert_eq!(v(g(0xA6BC, false, false)), 0xEA82 ^ 0xFFFF);
        // 3 KERMIT.
        assert_eq!(v(g(0x8408, false, false)), 0x2189);
        // 5 CRC-32.
        assert_eq!(v(g(0xEDB8_8320, true, true)), 0xCBF4_3926);
        // Additionneur : somme des octets.
        let mut a = g(0xA001, false, false);
        a.additionneur = true;
        assert_eq!(v(a), (b'1'..=b'9').map(|b| b as u32).sum::<u32>());
    }
}
