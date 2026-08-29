use std::collections::VecDeque;

/// Capacite standard des files FIFO de l'UART (16 octets).
pub const UART_FIFO_DEPTH: usize = 16;

/// Controleur UART pour la page 0x4000B000 (UART1).
///
/// Banque de registres :
/// - `+0x00` : RBR (donnee recue en lecture), THR (donnee a emettre en ecriture),
///             DLL (diviseur bas si DLAB=1 dans LCR)
/// - `+0x04` : IER (autorisations d'interruption), DLM (diviseur haut si DLAB=1)
/// - `+0x08` : IIR (identification d'interruption en lecture), FCR (controle FIFO en ecriture)
/// - `+0x0C` : LCR (controle de ligne : bits 0..1 longueur mot, bit 2 stop, bit 7 DLAB)
/// - `+0x10` : MCR (controle modem : bit 4 loopback)
/// - `+0x14` : LSR (etat de ligne : bits 0 DR, 5 THRE, 6 TEMT, 9 TXE, 10 TXF, 11 RXE, 12 RXF)
/// - `+0x18` : MSR (etat modem)
/// - `+0x1C` : SCR (registre de travail)
/// - `+0x28` : FDR (diviseur fractionnaire)
/// - `+0x30` : CTRL (validation generale)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UartController {
    pub dll: u8,
    pub dlm: u8,
    pub ier: u32,
    pub fcr: u32,
    pub lcr: u32,
    pub mcr: u32,
    pub msr: u32,
    pub scr: u32,
    pub fdr: u32,
    pub ctrl: u32,

    pub tx_fifo: VecDeque<u8>,
    pub rx_fifo: VecDeque<u8>,
    pub console_history: String,

    pub loopback: bool,
    pub irq_pending: bool,
}

impl Default for UartController {
    fn default() -> Self {
        Self {
            dll: 13,
            dlm: 0,
            ier: 0,
            fcr: 0,
            lcr: 0x03, // 8 bits, 1 stop bit, pas de parite
            mcr: 0,
            msr: 0,
            scr: 0,
            fdr: 0x100, // OVER8 = 1 par defaut
            ctrl: 0x07, // UARTEN | TXEN | RXEN
            tx_fifo: VecDeque::with_capacity(UART_FIFO_DEPTH),
            rx_fifo: VecDeque::with_capacity(UART_FIFO_DEPTH),
            console_history: String::new(),
            loopback: false,
            irq_pending: false,
        }
    }
}

impl UartController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Indique si un offset appartient a la banque de registres de l'UART.
    pub fn handles(offset: u32) -> bool {
        matches!(
            offset,
            0x00 | 0x04 | 0x08 | 0x0C | 0x10 | 0x14 | 0x18 | 0x1C | 0x28 | 0x30
        )
    }

    /// Calcule dynamiquement le registre d'etat de ligne LSR (+0x14).
    pub fn lsr(&self) -> u32 {
        let rx_empty = self.rx_fifo.is_empty();
        let rx_full = self.rx_fifo.len() >= UART_FIFO_DEPTH;
        let tx_empty = self.tx_fifo.is_empty();
        let tx_full = self.tx_fifo.len() >= UART_FIFO_DEPTH;

        let mut val = 0u32;
        if !rx_empty {
            val |= 1 << 0; // Bit 0 : Data Ready (donnee recue disponible)
        }
        if !tx_full {
            val |= 1 << 5; // Bit 5 : THRE (registre d'emission pret a recevoir)
        }
        if tx_empty {
            val |= 1 << 6; // Bit 6 : TEMT (emetteur completement vide)
            val |= 1 << 9; // Bit 9 : TX FIFO vide
        }
        if tx_full {
            val |= 1 << 10; // Bit 10 : TX FIFO pleine
        }
        if rx_empty {
            val |= 1 << 11; // Bit 11 : RX FIFO vide
        }
        if rx_full {
            val |= 1 << 12; // Bit 12 : RX FIFO pleine
        }
        val
    }

    /// Lecture d'un registre de l'UART.
    pub fn read_reg(&mut self, offset: u32) -> u32 {
        let dlab = (self.lcr & 0x80) != 0;
        match offset {
            0x00 => {
                if dlab {
                    self.dll as u32
                } else {
                    let b = self.rx_fifo.pop_front().unwrap_or(0);
                    self.evaluer_irq();
                    b as u32
                }
            }
            0x04 => {
                if dlab {
                    self.dlm as u32
                } else {
                    self.ier
                }
            }
            0x08 => self.calculer_iir(),
            0x0C => self.lcr,
            0x10 => self.mcr,
            0x14 => self.lsr(),
            0x18 => self.msr,
            0x1C => self.scr,
            0x28 => self.fdr,
            0x30 => self.ctrl,
            _ => 0,
        }
    }

    /// Ecriture dans un registre de l'UART.
    pub fn write_reg(&mut self, offset: u32, val: u32) {
        let dlab = (self.lcr & 0x80) != 0;
        match offset {
            0x00 => {
                if dlab {
                    self.dll = (val & 0xFF) as u8;
                } else {
                    let octet = (val & 0xFF) as u8;
                    if octet == b'\n' || octet == b'\r' {
                        self.console_history.push('\n');
                    } else if octet.is_ascii_graphic() || octet == b' ' {
                        self.console_history.push(octet as char);
                    }
                    if self.console_history.len() > 10_000 {
                        self.console_history = self.console_history.split_off(2_000);
                    }

                    if self.loopback || (self.mcr & 0x10) != 0 {
                        if self.rx_fifo.len() < UART_FIFO_DEPTH {
                            self.rx_fifo.push_back(octet);
                        }
                    } else if self.tx_fifo.len() < UART_FIFO_DEPTH {
                        self.tx_fifo.push_back(octet);
                    }
                    self.evaluer_irq();
                }
            }
            0x04 => {
                if dlab {
                    self.dlm = (val & 0xFF) as u8;
                } else {
                    self.ier = val;
                    self.evaluer_irq();
                }
            }
            0x08 => {
                self.fcr = val;
                if (val & 0x02) != 0 {
                    self.rx_fifo.clear();
                }
                if (val & 0x04) != 0 {
                    self.tx_fifo.clear();
                }
                self.evaluer_irq();
            }
            0x0C => self.lcr = val,
            0x10 => {
                self.mcr = val;
                self.loopback = (val & 0x10) != 0;
            }
            0x18 => self.msr = val,
            0x1C => self.scr = val,
            0x28 => self.fdr = val,
            0x30 => self.ctrl = val,
            _ => {}
        }
    }

    /// Injecte des octets recus depuis l'exterieur vers la file de reception.
    pub fn inject_rx_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if self.rx_fifo.len() < UART_FIFO_DEPTH {
                self.rx_fifo.push_back(b);
            }
        }
        self.evaluer_irq();
    }

    /// Recupere les octets emis par l'emulateur dans la file d'emission.
    pub fn drain_tx_bytes(&mut self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.tx_fifo.len());
        while let Some(b) = self.tx_fifo.pop_front() {
            out.push(b);
        }
        self.evaluer_irq();
        out
    }

    fn evaluer_irq(&mut self) {
        let rda_active = (self.ier & 0x01) != 0 && !self.rx_fifo.is_empty();
        let thre_active = (self.ier & 0x02) != 0 && (self.tx_fifo.len() < UART_FIFO_DEPTH);
        self.irq_pending = rda_active || thre_active;
    }

    fn calculer_iir(&self) -> u32 {
        if (self.ier & 0x01) != 0 && !self.rx_fifo.is_empty() {
            0x04 // RDA (donnees disponibles)
        } else if (self.ier & 0x02) != 0 && self.tx_fifo.is_empty() {
            0x02 // THRE (emetteur pret)
        } else {
            0x01 // Aucune interruption en attente
        }
    }

    /// Calcule le debit en bauds effectif d'apres la configuration des registres.
    pub fn baud_rate(&self, sys_clock: u32) -> u32 {
        let divisor = ((self.dlm as u32) << 8) | (self.dll as u32);
        if divisor == 0 {
            return 460800;
        }
        let over8 = (self.fdr >> 8) & 1;
        let oversampling = if over8 == 1 { 8 } else { 16 };
        let div = self.fdr & 0x0F;
        let mul = (self.fdr >> 4) & 0x0F;
        let frac = if mul > 0 {
            1.0 + (div as f64 / mul as f64)
        } else {
            1.0
        };
        let rate = sys_clock as f64 / (oversampling as f64 * divisor as f64 * frac);
        rate as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uart_lsr_evolution_and_loopback() {
        let mut uart = UartController::new();

        // 1. A l'etat initial : TX vide, RX vide
        let lsr_init = uart.read_reg(0x14);
        assert_eq!(lsr_init & (1 << 0), 0, "Bit 0 (DR) doit etre a 0 a vide");
        assert_ne!(lsr_init & (1 << 5), 0, "Bit 5 (THRE) doit etre a 1 a vide");
        assert_ne!(lsr_init & (1 << 6), 0, "Bit 6 (TEMT) doit etre a 1 a vide");
        assert_ne!(lsr_init & (1 << 9), 0, "Bit 9 (TXE) doit etre a 1 a vide");
        assert_eq!(lsr_init & (1 << 10), 0, "Bit 10 (TXF) doit etre a 0 a vide");
        assert_ne!(lsr_init & (1 << 11), 0, "Bit 11 (RXE) doit etre a 1 a vide");

        // 2. Ecriture d'un octet en mode normal (pas de loopback)
        uart.write_reg(0x00, 0x42);
        let lsr_1 = uart.read_reg(0x14);
        assert_ne!(lsr_1 & (1 << 5), 0, "Bit 5 (THRE) reste a 1 car file non pleine");
        assert_eq!(lsr_1 & (1 << 6), 0, "Bit 6 (TEMT) passe a 0 car 1 octet en file");
        assert_eq!(lsr_1 & (1 << 9), 0, "Bit 9 (TXE) passe a 0 car file non vide");
        assert_eq!(lsr_1 & (1 << 10), 0, "Bit 10 (TXF) est a 0 (1/16 octets)");

        // 3. Remplissage complet de la FIFO TX (15 octets de plus)
        for i in 1..UART_FIFO_DEPTH {
            uart.write_reg(0x00, i as u32);
        }
        let lsr_full = uart.read_reg(0x14);
        assert_eq!(lsr_full & (1 << 5), 0, "Bit 5 (THRE) passe a 0 quand FIFO est pleine");
        assert_eq!(lsr_full & (1 << 6), 0, "Bit 6 (TEMT) est a 0");
        assert_eq!(lsr_full & (1 << 9), 0, "Bit 9 (TXE) est a 0");
        assert_ne!(lsr_full & (1 << 10), 0, "Bit 10 (TXF) passe a 1 quand FIFO pleine");

        // 4. Vidage de la file TX via drain
        let drained = uart.drain_tx_bytes();
        assert_eq!(drained.len(), UART_FIFO_DEPTH);
        assert_eq!(drained[0], 0x42);
        let lsr_empty = uart.read_reg(0x14);
        assert_ne!(lsr_empty & (1 << 9), 0, "Bit 9 (TXE) repasse a 1 apres vidage");
        assert_eq!(lsr_empty & (1 << 10), 0, "Bit 10 (TXF) repasse a 0");

        // 5. Test du bouclage interne (Loopback)
        uart.write_reg(0x10, 0x10); // Active bit 4 (Loopback) dans MCR
        assert!(uart.loopback);

        uart.write_reg(0x00, 0xA5);
        let lsr_lb = uart.read_reg(0x14);
        assert_ne!(lsr_lb & (1 << 0), 0, "Bit 0 (DR) passe a 1 en loopback");
        assert_eq!(lsr_lb & (1 << 11), 0, "Bit 11 (RXE) passe a 0 en loopback");

        // Lecture de l'octet en reception
        let received = uart.read_reg(0x00);
        assert_eq!(received, 0xA5, "L'octet relu en loopback doit valoir 0xA5");

        let lsr_after_read = uart.read_reg(0x14);
        assert_eq!(lsr_after_read & (1 << 0), 0, "Bit 0 (DR) repasse a 0 apres lecture");
        assert_ne!(lsr_after_read & (1 << 11), 0, "Bit 11 (RXE) repasse a 1");
    }
}
