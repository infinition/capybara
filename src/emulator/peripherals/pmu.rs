/// Gestionnaire d'alimentation (Power Management Unit) pour Sonix SNC73410.
///
/// Registres materiels :
/// - `0x00` : PMU_CTRL (mode veille, deep power down, deep sleep)
/// - `0x04` : PMU_STATUS (drapeaux de reveil : bit 4 RTC, bit 6 broche IO, bit 7 interruption)
/// - `0x08` : PMU_WAKEUP_EN (validation des sources de reveil)
/// - `0x0C` : PMU_IO_LATCH_P0 (masque de maintien et detection de broches port 0)
/// - `0x10` : PMU_IO_LATCH_P1 (masque de maintien et detection de broches port 1)
/// - `0x14` : PMU_IO_LATCH_P2 (masque de maintien et detection de broches port 2)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PmuController {
    pub ctrl: u32,
    pub status: u32,
    pub wakeup_en: u32,
    pub latch_p0: u32,
    pub latch_p1: u32,
    pub latch_p2: u32,
    pub deep_sleep_active: bool,
}

impl Default for PmuController {
    fn default() -> Self {
        Self {
            ctrl: 0,
            status: 0,
            wakeup_en: 0x50, // RTC et broches actives par defaut
            latch_p0: 0,
            latch_p1: 0,
            latch_p2: 0,
            deep_sleep_active: false,
        }
    }
}

impl PmuController {
    pub const FLAG_RTC: u32 = 1 << 4;
    pub const FLAG_WAKEUP_PIN: u32 = 1 << 6;
    pub const FLAG_WAKEUP_INT: u32 = 1 << 7;

    pub fn handles(offset: u32) -> bool {
        matches!(offset, 0x00 | 0x04 | 0x08 | 0x0C | 0x10 | 0x14)
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            0x00 => self.ctrl,
            0x04 => self.status,
            0x08 => self.wakeup_en,
            0x0C => self.latch_p0,
            0x10 => self.latch_p1,
            0x14 => self.latch_p2,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            0x00 => {
                self.ctrl = val;
                // Mode 1: Deep Power Down, Mode 2: Deep Sleep
                if (val & 0x03) != 0 {
                    self.deep_sleep_active = true;
                }
            }
            0x04 => {
                // Ecrire 1 acquitte le drapeau
                self.status &= !val;
            }
            0x08 => self.wakeup_en = val,
            0x0C => self.latch_p0 = val,
            0x10 => self.latch_p1 = val,
            0x14 => self.latch_p2 = val,
            _ => {}
        }
    }

    /// Signale un reveil provoque par une broche utilisateur (bouton, molette).
    pub fn declencher_reveil_broche(&mut self) {
        self.status |= Self::FLAG_WAKEUP_PIN | Self::FLAG_WAKEUP_INT;
        self.deep_sleep_active = false;
    }

    /// Signale un reveil provoque par l'alarme RTC du calendrier.
    pub fn declencher_reveil_rtc(&mut self) {
        self.status |= Self::FLAG_RTC | Self::FLAG_WAKEUP_INT;
        self.deep_sleep_active = false;
    }
}
