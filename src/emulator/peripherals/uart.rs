use std::collections::VecDeque;

/// Capacite des files materielles d'emission et de reception.
pub const UART_FIFO_DEPTH: usize = 16;

/// Controleur UART1 de la page 0x4000B000.
///
/// Les files `tx_fifo` et `rx_fifo` representent les files materielles. Les
/// files `tx_out` et `rx_in` representent les octets deja sortis sur la ligne,
/// ou arrives depuis l'hote mais pas encore recus par le controleur.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UartController {
    pub dll: u8,
    pub dlm: u8,
    pub ier: u32,
    pub fcr: u32,
    pub lcr: u32,
    pub scr: u32,
    pub abctrl: u32,
    pub fdr: u32,
    pub ctrl: u32,
    pub hden: u32,

    pub tx_fifo: VecDeque<u8>,
    pub tx_out: VecDeque<u8>,
    pub rx_fifo: VecDeque<u8>,
    pub rx_in: VecDeque<u8>,
    pub console_history: String,

    pub irq_pending: bool,
    /// Octets perdus faute de place dans la file d'emission. Non nul, cela
    /// signale que le firmware ecrit sans consulter l'etat de ligne, ou que la
    /// file se vide trop lentement.
    pub tx_perdus: u64,
    /// Octets ecartes par une remise a zero de la file de reception. Ceux qui
    /// etaient deja dans la file sont perdus sur le materiel aussi ; ce
    /// compteur sert a mesurer l'ampleur du phenomene pendant un transfert.
    pub rx_jetes: u64,
    tx_phase: u64,
    rx_phase: u64,
}

impl Default for UartController {
    fn default() -> Self {
        Self {
            // Configuration prete a 460800 bauds sous 96 MHz. Le firmware la
            // remplace quand il ouvre le lien, mais cet etat garde les anciens
            // instantanes et les tests de transport utilisables.
            dll: 26,
            dlm: 0,
            ier: 0,
            fcr: 0x01,
            lcr: 0x03,
            scr: 0,
            abctrl: 0,
            fdr: 0x100,
            ctrl: 0xC1,
            hden: 0,
            tx_fifo: VecDeque::with_capacity(UART_FIFO_DEPTH),
            tx_out: VecDeque::new(),
            rx_fifo: VecDeque::with_capacity(UART_FIFO_DEPTH),
            rx_in: VecDeque::new(),
            console_history: String::new(),
            irq_pending: false,
            tx_perdus: 0,
            rx_jetes: 0,
            tx_phase: 0,
            rx_phase: 0,
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
            0x00 | 0x04 | 0x08 | 0x0C | 0x14 | 0x1C | 0x20 | 0x28 | 0x30 | 0x34
        )
    }

    /// Calcule dynamiquement le registre d'etat de ligne LS (+0x14).
    pub fn lsr(&self) -> u32 {
        let rx_empty = self.rx_fifo.is_empty();
        let rx_full = self.rx_fifo.len() >= UART_FIFO_DEPTH;
        let tx_empty = self.tx_fifo.is_empty();
        let tx_full = self.tx_fifo.len() >= UART_FIFO_DEPTH;

        let mut val = 0u32;
        if !rx_empty {
            val |= 1 << 0; // RDR
        }
        if !tx_full {
            val |= 1 << 5; // THRE
        }
        if tx_empty {
            val |= 1 << 6; // TEMT
            val |= 1 << 9; // TX_EMPTY
        }
        if tx_full {
            val |= 1 << 10; // TX_FULL
        }
        if rx_empty {
            val |= 1 << 11; // RX_EMPTY
        }
        if rx_full {
            val |= 1 << 12; // RX_FULL
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
            0x08 => self.iir(),
            0x0C => self.lcr,
            0x14 => self.lsr(),
            0x1C => self.scr,
            0x20 => self.abctrl,
            0x28 => self.fdr,
            0x30 => self.ctrl,
            0x34 => self.hden,
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
                    self.reinitialiser_cadence();
                } else {
                    self.ecrire_octet((val & 0xFF) as u8);
                }
            }
            0x04 => {
                if dlab {
                    self.dlm = (val & 0xFF) as u8;
                    self.reinitialiser_cadence();
                } else {
                    self.ier = val & 0x317;
                }
            }
            0x08 => {
                if (val & 0x02) != 0 {
                    // Seule la file materielle est videe. `rx_in` represente ce
                    // qui circule encore sur la ligne : le materiel ne peut pas
                    // le reprendre, et le jeter tronquait chaque rafale de
                    // l'hote au premier vidage de file.
                    self.rx_jetes += self.rx_fifo.len() as u64;
                    self.rx_fifo.clear();
                    self.rx_phase = 0;
                }
                if (val & 0x04) != 0 {
                    self.tx_fifo.clear();
                    self.tx_phase = 0;
                }
                // Les deux bits de remise a zero sont des impulsions.
                self.fcr = val & !0x06;
            }
            0x0C => {
                self.lcr = val & 0xFF;
                self.reinitialiser_cadence();
            }
            0x1C => self.scr = val,
            0x20 => {
                // Les acquittements d'auto-debit sont des impulsions. Le mode
                // et le redemarrage restent relisibles.
                self.abctrl = val & 0x07;
            }
            0x28 => {
                self.fdr = val & 0x1FF;
                self.reinitialiser_cadence();
            }
            0x30 => self.ctrl = val & 0xC1,
            0x34 => self.hden = val & 0x01,
            _ => {}
        }
        self.evaluer_irq();
    }

    fn ecrire_octet(&mut self, octet: u8) {
        if octet == b'\n' || octet == b'\r' {
            self.console_history.push('\n');
        } else if octet.is_ascii_graphic() || octet == b' ' {
            self.console_history.push(octet as char);
        }
        if self.console_history.len() > 10_000 {
            self.console_history = self.console_history.split_off(2_000);
        }

        if self.tx_fifo.len() < UART_FIFO_DEPTH {
            self.tx_fifo.push_back(octet);
        } else {
            self.tx_perdus += 1;
        }
    }

    /// Met des octets sur la ligne d'entree. Ils atteignent la FIFO RX au
    /// rythme configure par le firmware et ne sont jamais jetes si elle est
    /// momentanement pleine.
    pub fn inject_rx_bytes(&mut self, bytes: &[u8]) {
        self.rx_in.extend(bytes.iter().copied());
    }

    /// Fait avancer les deux lignes serie d'un nombre de cycles du coeur.
    pub fn tick(&mut self, cycles: u32, sys_clock: u32) {
        let baud = self.baud_rate(sys_clock);
        if baud == 0 || sys_clock == 0 {
            self.evaluer_irq();
            return;
        }

        let seuil = sys_clock as u64 * self.bits_par_octet() as u64;
        let credit = cycles as u64 * baud as u64;

        let tx_actif = (self.ctrl & 0x81) == 0x81;
        if self.tx_fifo.is_empty() || !tx_actif {
            self.tx_phase = 0;
        } else {
            self.tx_phase = self.tx_phase.saturating_add(credit);
            while self.tx_phase >= seuil {
                let Some(b) = self.tx_fifo.pop_front() else {
                    self.tx_phase = 0;
                    break;
                };
                self.tx_out.push_back(b);
                self.tx_phase -= seuil;
            }
        }

        let rx_actif = (self.ctrl & 0x41) == 0x41;
        if self.rx_in.is_empty() || !rx_actif {
            self.rx_phase = 0;
        } else {
            self.rx_phase = self.rx_phase.saturating_add(credit);
            while self.rx_phase >= seuil && self.rx_fifo.len() < UART_FIFO_DEPTH {
                let Some(b) = self.rx_in.pop_front() else {
                    self.rx_phase = 0;
                    break;
                };
                self.rx_fifo.push_back(b);
                self.rx_phase -= seuil;
            }
            if self.rx_fifo.len() >= UART_FIFO_DEPTH {
                // Le tampon hote fournit la contre-pression que la ligne
                // physique n'a pas. On ne cumule pas un retard infini pendant
                // que le firmware vide la FIFO.
                self.rx_phase = self.rx_phase.min(seuil);
            }
        }

        self.evaluer_irq();
    }

    /// Vide les deux sens de la ligne, sans toucher aux registres.
    ///
    /// A appeler quand un hote se branche. La console imprime son journal de
    /// demarrage bien avant, et ces octets attendent dans la file de sortie :
    /// sans ce vidage, le premier chose que recoit l'outil de transfert est un
    /// message de demarrage, et sa conversation commence desynchronisee.
    pub fn vider_la_ligne(&mut self) {
        self.tx_out.clear();
        self.rx_in.clear();
        self.rx_fifo.clear();
        self.tx_phase = 0;
        self.rx_phase = 0;
    }

    /// Rend les octets dont la transmission sur la ligne est terminee.
    pub fn drain_hote(&mut self) -> Vec<u8> {
        self.tx_out.drain(..).collect()
    }

    /// Replace des octets devant la sortie, notamment apres une ecriture hote
    /// partielle.
    pub fn remettre_sortie(&mut self, bytes: &[u8]) {
        for &b in bytes.iter().rev() {
            self.tx_out.push_front(b);
        }
    }

    /// Registre d'identification d'interruption, complete par l'etat des FIFO.
    pub fn iir(&self) -> u32 {
        let mut valeur = self.etat_files_iir();
        if (self.ier & 0x04) != 0 && (self.lsr() & 0x9E) != 0 {
            valeur |= 0x06; // RLS
        } else if self.rda_active() {
            valeur |= 0x04; // RDA
        } else if (self.ier & 0x10) != 0 && self.tx_fifo.is_empty() {
            valeur |= 0x0E; // TEMT
        } else if (self.ier & 0x02) != 0 && self.tx_fifo.len() < UART_FIFO_DEPTH {
            valeur |= 0x02; // THRE
        } else {
            valeur |= 0x01; // aucune interruption en attente
        }
        valeur
    }

    fn etat_files_iir(&self) -> u32 {
        let mut valeur = if (self.fcr & 0x01) != 0 { 0xC0 } else { 0 };
        if self.tx_fifo.is_empty() {
            valeur |= 1 << 11;
        }
        if self.tx_fifo.len() >= UART_FIFO_DEPTH {
            valeur |= 1 << 12;
        }
        if self.rx_fifo.is_empty() {
            valeur |= 1 << 13;
        }
        if self.rx_fifo.len() >= UART_FIFO_DEPTH {
            valeur |= 1 << 14;
        }
        valeur
    }

    fn rda_active(&self) -> bool {
        if (self.ier & 0x01) == 0 {
            return false;
        }
        let seuil = match (self.fcr >> 6) & 0x03 {
            0 => 1,
            1 => 4,
            2 => 8,
            _ => 14,
        };
        self.rx_fifo.len() >= seuil
    }

    fn evaluer_irq(&mut self) {
        let rls = (self.ier & 0x04) != 0 && (self.lsr() & 0x9E) != 0;
        let thre = (self.ier & 0x02) != 0 && self.tx_fifo.len() < UART_FIFO_DEPTH;
        let temt = (self.ier & 0x10) != 0 && self.tx_fifo.is_empty();
        self.irq_pending = rls || self.rda_active() || thre || temt;
    }

    fn reinitialiser_cadence(&mut self) {
        self.tx_phase = 0;
        self.rx_phase = 0;
    }

    fn bits_par_octet(&self) -> u32 {
        let donnees = 5 + (self.lcr & 0x03);
        let parite = ((self.lcr >> 3) & 1) as u32;
        let arret = if (self.lcr & 0x04) != 0 { 2 } else { 1 };
        1 + donnees + parite + arret
    }

    /// Calcule le debit effectif d'apres le diviseur, la fraction et le mode
    /// de surechantillonnage programmes.
    pub fn baud_rate(&self, sys_clock: u32) -> u32 {
        let divisor = ((self.dlm as u64) << 8) | self.dll as u64;
        if divisor == 0 {
            return 0;
        }
        let oversampling = if (self.fdr & 0x100) != 0 { 8 } else { 16 };
        let div = (self.fdr & 0x0F) as u64;
        let mul = ((self.fdr >> 4) & 0x0F) as u64;
        let (numerateur, fraction) = if mul == 0 {
            (sys_clock as u64, 1)
        } else {
            (sys_clock as u64 * mul, mul + div)
        };
        (numerateur / (oversampling * divisor * fraction)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HORLOGE: u32 = 96_000_000;

    #[test]
    fn statut_et_cadence_d_emission() {
        let mut uart = UartController::new();
        assert_eq!(uart.baud_rate(HORLOGE), 461_538);
        assert_ne!(uart.lsr() & (1 << 6), 0);

        uart.write_reg(0x00, 0x42);
        assert_eq!(uart.lsr() & (1 << 6), 0);
        uart.tick(2_000, HORLOGE);
        assert!(uart.drain_hote().is_empty());
        uart.tick(100, HORLOGE);
        assert_eq!(uart.drain_hote(), vec![0x42]);
        assert_ne!(uart.lsr() & (1 << 6), 0);
    }

    #[test]
    fn reception_ne_perd_pas_ce_qui_depasse_la_fifo() {
        let mut uart = UartController::new();
        let entree: Vec<u8> = (0..32).collect();
        uart.inject_rx_bytes(&entree);
        uart.tick(100_000, HORLOGE);
        assert_eq!(uart.rx_fifo.len(), UART_FIFO_DEPTH);
        assert_eq!(uart.rx_in.len(), UART_FIFO_DEPTH);

        for attendu in 0..16u32 {
            assert_eq!(uart.read_reg(0x00), attendu);
        }
        uart.tick(2_100, HORLOGE);
        assert_eq!(uart.read_reg(0x00), 16);
    }

    #[test]
    fn le_vidage_de_file_ne_reprend_pas_ce_qui_circule() {
        let mut uart = UartController::new();
        // Une rafale bien plus grande que la file materielle, comme celle d'un
        // outil hote qui envoie un objet en quelques milliers d'octets.
        let rafale: Vec<u8> = (0..200u32).map(|i| i as u8).collect();
        uart.inject_rx_bytes(&rafale);
        uart.tick(100_000, HORLOGE);
        assert_eq!(uart.rx_fifo.len(), UART_FIFO_DEPTH);

        // Remise a zero de la file de reception, bit 1 du registre 0x08.
        uart.write_reg(0x08, 0x02);
        assert!(uart.rx_fifo.is_empty());
        assert_eq!(uart.rx_jetes, UART_FIFO_DEPTH as u64);

        // Le reste de la rafale doit continuer d'arriver : elle est encore sur
        // la ligne, le materiel ne peut pas la reprendre.
        uart.tick(100_000, HORLOGE);
        assert_eq!(uart.rx_fifo.len(), UART_FIFO_DEPTH);
        assert_eq!(uart.read_reg(0x00), UART_FIFO_DEPTH as u32);
    }

    #[test]
    fn registres_dlab_et_interruptions() {
        let mut uart = UartController::new();
        uart.write_reg(0x0C, 0x83);
        uart.write_reg(0x00, 13);
        uart.write_reg(0x04, 0);
        assert_eq!(uart.read_reg(0x00), 13);
        uart.write_reg(0x0C, 0x03);
        uart.write_reg(0x04, 0x01);
        uart.inject_rx_bytes(&[0xA5]);
        uart.tick(2_100, HORLOGE);
        assert!(uart.irq_pending);
        assert_eq!(uart.iir() & 0x0F, 0x04);
        assert_eq!(uart.read_reg(0x00), 0xA5);
        assert!(!uart.irq_pending);
    }

    #[test]
    fn activation_separee_des_lignes_tx_et_rx() {
        let mut uart = UartController::new();
        uart.write_reg(0x30, 0);
        uart.write_reg(0x00, 0x5A);
        uart.inject_rx_bytes(&[0xA5]);
        uart.tick(10_000, HORLOGE);
        assert!(uart.drain_hote().is_empty());
        assert!(uart.rx_fifo.is_empty());

        uart.write_reg(0x30, 0xC1);
        uart.tick(2_100, HORLOGE);
        assert_eq!(uart.drain_hote(), vec![0x5A]);
        assert_eq!(uart.read_reg(0x00), 0xA5);
    }
}
