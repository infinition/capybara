/// Modele de timer herite, branche sur 0x40040000..0x40046000 (`LEGACY_TIMERS`).
///
/// ATTENTION : cette plage n'a aucun
/// peripherique dans le SVD et le firmware n'y accede jamais. Les vrais timers
/// sont CT32B0..7 en 0x40000000..0x40007000 (non modelises) ; leur IRQ est
/// 38 + n, alors que ce modele leve l'IRQ 16 (SDIO_DMA_IRQn dans le SDK).
/// Les champs `wdt_*` ne sont pas le chien de garde (WDT0/WDT1 en
/// 0x40008000/0x40009000). Code mort garde tel quel pour ne rien changer.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Timers {
    pub timer0_count: u32,
    pub timer0_reload: u32,
    pub timer0_ctrl: u32,
    pub wdt_count: u32,
    pub wdt_enabled: bool,
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            timer0_count: 0,
            timer0_reload: 1000,
            timer0_ctrl: 0,
            wdt_count: 0,
            wdt_enabled: false,
        }
    }
}

impl Timers {
    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            0x00 => self.timer0_count,
            0x04 => self.timer0_reload,
            0x08 => self.timer0_ctrl,
            0x10 => self.wdt_count,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            0x00 => self.timer0_count = val,
            0x04 => self.timer0_reload = val,
            0x08 => self.timer0_ctrl = val,
            0x10 => {
                self.wdt_count = val;
                self.wdt_enabled = val != 0;
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, cycles: u32) -> bool {
        let mut irq = false;
        if (self.timer0_ctrl & 1) != 0 {
            if self.timer0_count <= cycles {
                self.timer0_count = self.timer0_reload;
                irq = true;
            } else {
                self.timer0_count -= cycles;
            }
        }
        irq
    }
}
