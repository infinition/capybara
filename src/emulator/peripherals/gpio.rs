#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct GpioController {
    pub pin_data: u32,
    pub btn_a: bool,
    pub btn_b: bool,
    pub btn_c: bool,
    pub dial_counter: i32,
}

impl Default for GpioController {
    fn default() -> Self {
        Self {
            pin_data: 0xFFFF_FFFF, // Active-low pull-up
            btn_a: false,
            btn_b: false,
            btn_c: false,
            dial_counter: 0,
        }
    }
}

impl GpioController {
    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            0x00 => {
                let mut data = 0xFFFF_FFFF;
                if self.btn_a {
                    data &= !(1 << 0);
                }
                if self.btn_b {
                    data &= !(1 << 1);
                }
                if self.btn_c {
                    data &= !(1 << 2);
                }
                data
            }
            0x04 => self.dial_counter as u32,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        if offset == 0x04 {
            self.dial_counter = val as i32;
        }
    }

    pub fn set_button_a(&mut self, pressed: bool) {
        self.btn_a = pressed;
    }

    pub fn set_button_b(&mut self, pressed: bool) {
        self.btn_b = pressed;
    }

    pub fn set_button_c(&mut self, pressed: bool) {
        self.btn_c = pressed;
    }

    pub fn step_dial(&mut self, delta: i32) {
        self.dial_counter = self.dial_counter.wrapping_add(delta);
    }
}
