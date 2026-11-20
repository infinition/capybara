#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Timers {
    pub timer0_count: u32,
    pub timer0_reload: u32,
    pub timer0_ctrl: u32,
    pub wdt_count: u32,
    pub wdt_enabled: bool,
    #[serde(default)]
    pub buzzer_regs: [u32; 16],
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            timer0_count: 0,
            timer0_reload: 1000,
            timer0_ctrl: 0,
            wdt_count: 0,
            wdt_enabled: false,
            buzzer_regs: [0; 16],
        }
    }
}

impl Timers {
    pub fn read_buzzer_reg(&mut self, offset: u32) -> u32 {
        // Also accept old snapshots captured while the reset bit was stuck.
        if offset == 0 && self.buzzer_regs[0] & 2 != 0 {
            self.buzzer_regs[0] &= !2;
            self.buzzer_regs[0x08 / 4] = 0;
        }
        self.buzzer_regs[(offset / 4) as usize]
    }

    pub fn write_buzzer_reg(&mut self, offset: u32, value: u32) {
        if offset == 0 && value & 2 != 0 {
            // CT32B7 acknowledges a counter reset immediately. The native
            // sound driver polls this bit before programming every tone.
            self.buzzer_regs[0] = value & !2;
            self.buzzer_regs[0x08 / 4] = 0;
        } else {
            self.buzzer_regs[(offset / 4) as usize] = value;
        }
    }

    /// CT32B7 PWM used by Paramix. The guest explicitly starts/stops output.
    pub fn buzzer_frequency(&self) -> f32 {
        let r = &self.buzzer_regs;
        let period = r[0x24 / 4];
        let match0 = r[0x18 / 4];
        if r[0] & 3 != 1 || r[0x34 / 4] == 0 || period == 0
            || match0 == 0 || match0 >= period {
            return 0.0;
        }
        72_000_000.0 / (r[0x0C / 4] as f32 + 1.0) / period as f32
    }

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
