use std::collections::VecDeque;

/// Controleur USB officiel Sonix SNC73410 (`0x40007000`).
///
/// Permet l'emulation du peripherique USB CDC/HID integre, facilitant la
/// communication directe avec les logiciels tiers de transfert.
///
/// Registres principaux :
/// - `0x00` : USB_CTRL (validation du peripherique USB, signal de connexion)
/// - `0x04` : USB_INT_EN (masques d'interruption USB)
/// - `0x08` : USB_INT_STAT (etat d'interruption : reset bus, resume, transactions EP)
/// - `0x0C` : USB_DEV_ADDR (adresse USB attribuee par l'hote)
/// - `0x10` : EP0_CSR (controle et etat du point d'acces 0 de controle)
/// - `0x14` : EP0_COUNT (nombre d'octets disponibles sur EP0)
/// - `0x20` : EP1_CSR (controle et etat du point d'acces 1 CDC/donnees)
/// - `0x24` : EP1_COUNT (nombre d'octets sur EP1)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UsbController {
    pub ctrl: u32,
    pub int_en: u32,
    pub int_stat: u32,
    pub dev_addr: u32,
    pub ep0_csr: u32,
    pub ep0_count: u32,
    pub ep1_csr: u32,
    pub ep1_count: u32,
    pub ep0_rx: VecDeque<u8>,
    pub ep0_tx: VecDeque<u8>,
    pub ep1_rx: VecDeque<u8>,
    pub ep1_tx: VecDeque<u8>,
    pub est_connecte: bool,
}

impl Default for UsbController {
    fn default() -> Self {
        Self {
            ctrl: 0,
            int_en: 0,
            int_stat: 0,
            dev_addr: 0,
            ep0_csr: 0,
            ep0_count: 0,
            ep1_csr: 0,
            ep1_count: 0,
            ep0_rx: VecDeque::new(),
            ep0_tx: VecDeque::new(),
            ep1_rx: VecDeque::new(),
            ep1_tx: VecDeque::new(),
            est_connecte: false,
        }
    }
}

impl UsbController {
    pub fn handles(offset: u32) -> bool {
        matches!(
            offset,
            0x00 | 0x04 | 0x08 | 0x0C | 0x10 | 0x14 | 0x20 | 0x24 | 0x30 | 0x34
        )
    }

    pub fn read_reg(&mut self, offset: u32) -> u32 {
        match offset {
            0x00 => self.ctrl,
            0x04 => self.int_en,
            0x08 => self.int_stat,
            0x0C => self.dev_addr,
            0x10 => self.ep0_csr,
            0x14 => self.ep0_rx.len() as u32,
            0x20 => self.ep1_csr,
            0x24 => self.ep1_rx.len() as u32,
            0x30 => {
                // Lecture FIFO EP0
                self.ep0_rx.pop_front().unwrap_or(0) as u32
            }
            0x34 => {
                // Lecture FIFO EP1 (donnees de l'outil de transfert)
                self.ep1_rx.pop_front().unwrap_or(0) as u32
            }
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            0x00 => {
                self.ctrl = val;
                self.est_connecte = (val & 0x01) != 0;
            }
            0x04 => self.int_en = val,
            0x08 => self.int_stat &= !val, // Acquittement
            0x0C => self.dev_addr = val & 0x7F,
            0x10 => self.ep0_csr = val,
            0x20 => self.ep1_csr = val,
            0x30 => {
                // Ecriture FIFO EP0
                self.ep0_tx.push_back((val & 0xFF) as u8);
            }
            0x34 => {
                // Ecriture FIFO EP1
                self.ep1_tx.push_back((val & 0xFF) as u8);
                if self.ep1_tx.len() > 4096 {
                    self.ep1_tx.pop_front();
                }
            }
            _ => {}
        }
    }

    /// Injecte un paquet CDC recu d'un logiciel externe vers le firmware.
    pub fn injecter_paquet_externe(&mut self, donnees: &[u8]) {
        for &octet in donnees {
            self.ep1_rx.push_back(octet);
        }
        self.int_stat |= 1 << 2; // Drapeau EP1 RX
    }

    /// Extrait les paquets de donnees emis par le firmware.
    pub fn extraire_paquets_emis(&mut self) -> Vec<u8> {
        self.ep1_tx.drain(..).collect()
    }
}
