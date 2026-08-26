use std::collections::VecDeque;

pub struct UartController {
    pub baud_rate: u32,
    pub tx_buffer: VecDeque<u8>,
    pub rx_buffer: VecDeque<u8>,
    pub console_history: String,
}

impl Default for UartController {
    fn default() -> Self {
        Self {
            baud_rate: 460800,
            tx_buffer: VecDeque::new(),
            rx_buffer: VecDeque::new(),
            console_history: String::new(),
        }
    }
}

impl UartController {
    pub fn read_reg(&mut self, offset: u32) -> u32 {
        match offset {
            0x00 => {
                // Data register (RX)
                self.rx_buffer.pop_front().unwrap_or(0) as u32
            }
            0x04 => {
                // Status register: bit 0 = RX ready, bit 1 = TX ready
                let mut status = 0x02; // TX always ready
                if !self.rx_buffer.is_empty() {
                    status |= 0x01;
                }
                status
            }
            0x08 => self.baud_rate,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            0x00 => {
                // Data register (TX)
                let byte = (val & 0xFF) as u8;
                self.tx_buffer.push_back(byte);
                if byte == b'\n' || byte == b'\r' {
                    self.console_history.push('\n');
                } else if byte.is_ascii_graphic() || byte == b' ' {
                    self.console_history.push(byte as char);
                }
                // Cap history length
                if self.console_history.len() > 10_000 {
                    self.console_history = self.console_history.split_off(2_000);
                }
            }
            0x08 => self.baud_rate = val,
            _ => {}
        }
    }

    pub fn inject_rx_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.rx_buffer.push_back(b);
        }
    }
}
