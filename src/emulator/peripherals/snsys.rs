use std::collections::BTreeMap;

/// Registres d'horloge et de PLL de la zone systeme SN_SYS0 (0x45000000),
/// hors fusibles FEUSE qui ont leur propre modele.
///
/// Le firmware ecrit une sequence de verrouillage PLL puis attend le bit 8 du
/// statut (offset 0x08). Les valeurs ecrites sont memorisees pour que le
/// read-modify-write sur l'offset 0x00 retrouve l'etat precedent.
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnSysRegisters {
    regs: BTreeMap<u32, u32>,
    pll_locked: bool,
}

impl SnSysRegisters {
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
            _ => self.regs.get(&offset).copied().unwrap_or(0),
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        // 0x04 recoit la valeur magique de deblocage avant la configuration.
        if offset == 0x04 && val == 0xA55A_0000 {
            self.pll_locked = true;
        }
        self.regs.insert(offset, val);
    }
}
