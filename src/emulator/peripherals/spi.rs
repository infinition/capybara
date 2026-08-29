use std::collections::VecDeque;

/// Controleur SPI0 officiel Sonix `SN_SPI_Type` (`0x4000E000`).
///
/// Registres materiels :
/// - `0x00` : CTRL (validation SPI, mode maitre, polarite CPOL, phase CPHA, format 8/16 bits)
/// - `0x04` : CLKDIV (diviseur de frequence d'horloge SPI)
/// - `0x08` : STAT (etat : bit 0 TXEMPTY, bit 1 TXFULL, bit 2 RXEMPTY, bit 3 RXFULL, bit 4 BUSY)
/// - `0x10` : IE (autorisations d'interruptions)
/// - `0x14` : RIS (etat brut des interruptions)
/// - `0x18` : IC (acquittement d'interruptions)
/// - `0x1C` : DATA (registre de donnees FIFO TX/RX, destination des trames ecran)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SpiController {
    pub ctrl: u32,
    pub clkdiv: u32,
    pub stat: u32,
    pub ie: u32,
    pub ris: u32,
    pub tx_fifo: VecDeque<u16>,
    pub rx_fifo: VecDeque<u16>,
    pub total_octets_emis: u64,
}

impl Default for SpiController {
    fn default() -> Self {
        Self {
            ctrl: 0,
            clkdiv: 2,
            // Par defaut : TXEMPTY (0x01) et RXEMPTY (0x04) poses
            stat: 0x05,
            ie: 0,
            ris: 0,
            tx_fifo: VecDeque::new(),
            rx_fifo: VecDeque::new(),
            total_octets_emis: 0,
        }
    }
}

impl SpiController {
    pub fn handles(offset: u32) -> bool {
        matches!(offset, 0x00 | 0x04 | 0x08 | 0x10 | 0x14 | 0x18 | 0x1C)
    }

    pub fn read_reg(&mut self, offset: u32) -> u32 {
        match offset {
            0x00 => self.ctrl,
            0x04 => self.clkdiv,
            0x08 => {
                let mut s = 0x01; // TXEMPTY
                if self.rx_fifo.is_empty() {
                    s |= 0x04; // RXEMPTY
                }
                s
            }
            0x10 => self.ie,
            0x14 => self.ris,
            0x1C => {
                let mot = self.rx_fifo.pop_front().unwrap_or(0);
                mot as u32
            }
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            0x00 => self.ctrl = val,
            0x04 => self.clkdiv = val,
            0x08 => {} // STAT est en lecture seule
            0x10 => self.ie = val,
            0x14 => {}
            0x18 => self.ris &= !val, // Acquittement
            0x1C => {
                let mot = (val & 0xFFFF) as u16;
                self.tx_fifo.push_back(mot);
                if self.tx_fifo.len() > 1024 {
                    self.tx_fifo.pop_front();
                }
                self.total_octets_emis += 2;
            }
            _ => {}
        }
    }
}
