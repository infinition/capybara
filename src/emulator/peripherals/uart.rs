use std::collections::VecDeque;

/// Controleur UART du SNC73410, un 16550 ordinaire.
///
/// Implantation des registres materiels :
/// - `0x00` : RB (reception), TH (emission), DLL (diviseur LSB si DLAB=1)
/// - `0x04` : IE (autorisations d'interruption), DLM (diviseur MSB si DLAB=1)
/// - `0x08` : II (identification d'interruption en lecture), FIFOCTRL (controle FIFO en ecriture)
/// - `0x0C` : LC (controle de ligne : format, parite, bit DLAB)
/// - `0x10` : MC (controle modem)
/// - `0x14` : LS (etat de ligne : donnees pretes bit 0, THRE bit 5, TEMT bit 6)
/// - `0x18` : MS (etat modem)
/// - `0x1C` : SCR (registre temporaire de travail)
/// - `0x20` : ABCTRL (controle du calcul automatique de debit)
/// - `0x24` : ABRES (resultat du calcul automatique de debit)
/// - `0x28` : FD (diviseur fractionnaire : DIVADDVAL, MULVAL, OVER8)
/// - `0x30` : CTRL (validation generale de l'UART, TX et RX)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UartController {
    pub dll: u8,
    pub dlm: u8,
    pub ie: u32,
    pub iir: u32,
    pub fifoctrl: u32,
    pub lc: u32,
    pub mc: u32,
    pub ls: u32,
    pub ms: u32,
    pub scr: u32,
    pub abctrl: u32,
    pub abres: u32,
    pub fd: u32,
    pub ctrl: u32,

    pub tx_buffer: VecDeque<u8>,
    pub rx_buffer: VecDeque<u8>,
    pub console_history: String,
    pub irq_pending: bool,
}

impl Default for UartController {
    fn default() -> Self {
        Self {
            dll: 13,
            dlm: 0,
            ie: 0,
            iir: 0x01, // 0x01 = aucune interruption en attente (bit 0 = 1)
            fifoctrl: 0,
            lc: 0x03, // 8 bits de donnees (WLS = 3)
            mc: 0,
            ls: 0, // Initialement a 0 avant transmission
            ms: 0,
            scr: 0,
            abctrl: 0,
            abres: 0,
            fd: 0x100, // OVER8 = 1 par defaut
            ctrl: 0x07, // UARTEN | TXEN | RXEN
            tx_buffer: VecDeque::new(),
            rx_buffer: VecDeque::new(),
            console_history: String::new(),
            irq_pending: false,
        }
    }
}

impl UartController {
    /// Indique si ce controleur prend en charge un offset donne.
    pub fn handles(offset: u32) -> bool {
        matches!(
            offset,
            0x00 | 0x04 | 0x08 | 0x0C | 0x10 | 0x14 | 0x18 | 0x1C | 0x20 | 0x24 | 0x28 | 0x30
        )
    }

    /// Lecture d'un registre du peripherique UART.
    pub fn read_reg(&mut self, offset: u32) -> u32 {
        let dlab = (self.lc & 0x80) != 0;
        match offset {
            0x00 => {
                if dlab {
                    self.dll as u32
                } else {
                    let octet = self.rx_buffer.pop_front().unwrap_or(0);
                    self.mettre_a_jour_etat_rx();
                    octet as u32
                }
            }
            0x04 => {
                if dlab {
                    self.dlm as u32
                } else {
                    self.ie
                }
            }
            0x08 => self.calculer_iir(),
            0x0C => self.lc,
            0x10 => self.mc,
            0x14 => {
                let mut status = self.ls;
                if !self.rx_buffer.is_empty() {
                    status |= 0x01; // RDR : donnees pretes en reception
                }
                status
            }
            0x18 => self.ms,
            0x1C => self.scr,
            0x20 => self.abctrl,
            0x24 => self.abres,
            0x28 => self.fd,
            0x30 => self.ctrl,
            _ => 0,
        }
    }

    /// Ecriture dans un registre du peripherique UART.
    pub fn write_reg(&mut self, offset: u32, val: u32) {
        let dlab = (self.lc & 0x80) != 0;
        match offset {
            0x00 => {
                if dlab {
                    self.dll = (val & 0xFF) as u8;
                } else {
                    let octet = (val & 0xFF) as u8;
                    self.tx_buffer.push_back(octet);
                    if octet == b'\n' || octet == b'\r' {
                        self.console_history.push('\n');
                    } else if octet.is_ascii_graphic() || octet == b' ' {
                        self.console_history.push(octet as char);
                    }
                    if self.console_history.len() > 10_000 {
                        self.console_history = self.console_history.split_off(2_000);
                    }
                    // THRE et TEMT restent poses pour autoriser les envois successifs immediats
                    self.ls |= 0x60;
                    self.evaluer_irq();
                }
            }
            0x04 => {
                if dlab {
                    self.dlm = (val & 0xFF) as u8;
                } else {
                    self.ie = val;
                    self.evaluer_irq();
                }
            }
            0x08 => {
                self.fifoctrl = val;
                if (val & 0x02) != 0 {
                    self.rx_buffer.clear();
                    self.mettre_a_jour_etat_rx();
                }
                if (val & 0x04) != 0 {
                    self.tx_buffer.clear();
                }
            }
            0x0C => self.lc = val,
            0x10 => self.mc = val,
            0x14 => self.ls = val,
            0x18 => self.ms = val,
            0x1C => self.scr = val,
            0x20 => self.abctrl = val,
            0x24 => self.abres = val,
            0x28 => self.fd = val,
            0x30 => self.ctrl = val,
            _ => {}
        }
    }

    /// Injecte des octets recus depuis l'exterieur vers le tampon de reception.
    pub fn inject_rx_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.rx_buffer.push_back(b);
        }
        self.mettre_a_jour_etat_rx();
    }

    fn mettre_a_jour_etat_rx(&mut self) {
        if !self.rx_buffer.is_empty() {
            self.ls |= 0x01; // RDR
        } else {
            self.ls &= !0x01;
        }
        self.evaluer_irq();
    }

    fn evaluer_irq(&mut self) {
        let rda_active = (self.ie & 0x01) != 0 && !self.rx_buffer.is_empty();
        let thre_active = (self.ie & 0x02) != 0 && (self.ls & 0x20) != 0;
        self.irq_pending = rda_active || thre_active;
    }

    fn calculer_iir(&self) -> u32 {
        if (self.ie & 0x01) != 0 && !self.rx_buffer.is_empty() {
            // Interruption reception (RDA, ID = 2, bit 0 = 0 car pending)
            0x04
        } else if (self.ie & 0x02) != 0 && (self.ls & 0x20) != 0 {
            // Interruption emission prete (THRE, ID = 1, bit 0 = 0 car pending)
            0x02
        } else {
            // Pas d'interruption en attente (bit 0 = 1)
            0x01
        }
    }

    /// Calcule le debit en bauds effectif d'apres la configuration des registres.
    pub fn baud_rate(&self, sys_clock: u32) -> u32 {
        let divisor = ((self.dlm as u32) << 8) | (self.dll as u32);
        if divisor == 0 {
            return 460800;
        }
        let over8 = (self.fd >> 8) & 1;
        let oversampling = if over8 == 1 { 8 } else { 16 };
        let div = self.fd & 0x0F;
        let mul = (self.fd >> 4) & 0x0F;
        let frac = if mul > 0 {
            1.0 + (div as f64 / mul as f64)
        } else {
            1.0
        };
        let rate = sys_clock as f64 / (oversampling as f64 * divisor as f64 * frac);
        rate as u32
    }
}
