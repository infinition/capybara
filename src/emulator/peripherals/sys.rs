pub struct SysRegisters {
    pub osc_ctrl: u32,
    pub pinctrl: u32,
    pub clk_ctrl: u32,
}

impl Default for SysRegisters {
    fn default() -> Self {
        Self {
            osc_ctrl: 0x00,
            pinctrl: 0x00,
            clk_ctrl: 0x01,
        }
    }
}

impl SysRegisters {
    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            0x00 => self.osc_ctrl,
            0x20 => self.pinctrl,
            0x24 => self.clk_ctrl,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) -> bool {
        let mut rom_hidden_changed = false;
        match offset {
            0x00 => {
                // Bit 3 hides the Boot ROM
                if (val & 0x08) != 0 && (self.osc_ctrl & 0x08) == 0 {
                    rom_hidden_changed = true;
                }
                self.osc_ctrl |= val & 0x08; // Set-only bit
            }
            0x20 => self.pinctrl = val,
            0x24 => self.clk_ctrl = val,
            _ => {}
        }
        rom_hidden_changed
    }
}
