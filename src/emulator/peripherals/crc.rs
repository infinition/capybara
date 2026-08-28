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
/// repli du SDK, "unsupport chip, please check your flash vender", qui n'a rien
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
}

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
        }
    }
}

/// Calcul demande, a executer par le bus qui seul voit la memoire.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Calcul {
    pub source: u32,
    pub length: u32,
    pub polynome: u16,
}

impl ChecksumUnit {
    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            // Le bit de lancement retombe des la fin du calcul, et celui-ci est
            // instantane : le registre se lit donc toujours au repos.
            CTRL => self.ctrl & !CTRL_START,
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
                self.ctrl = val;
                if val & CTRL_START != 0 && self.length != 0 {
                    self.calculs += 1;
                    return Some(Calcul {
                        source: self.source,
                        length: self.length,
                        polynome: self.polynome as u16,
                    });
                }
            }
            _ => {}
        }
        None
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
}
