use std::collections::VecDeque;
use std::io::{Read, Write};
use crate::emulator::peripherals::UartController;

/// Pont serie entre l'emulateur (UartController) et l'ordinateur hote.
pub struct UartHostBridge {
    pub port_name: String,
    pub baud_rate: u32,
    pub is_connected: bool,
    pub bytes_sent: usize,
    pub bytes_received: usize,

    // File d'attente interne hote pour les tests ou les transports personnalises
    host_rx_stream: VecDeque<u8>,
    host_tx_stream: VecDeque<u8>,
}

impl Default for UartHostBridge {
    fn default() -> Self {
        Self {
            port_name: "COM10".to_string(),
            baud_rate: 460800,
            is_connected: false,
            bytes_sent: 0,
            bytes_received: 0,
            host_rx_stream: VecDeque::new(),
            host_tx_stream: VecDeque::new(),
        }
    }
}

impl UartHostBridge {
    pub fn new(port_name: &str, baud_rate: u32) -> Self {
        Self {
            port_name: port_name.to_string(),
            baud_rate,
            is_connected: true,
            bytes_sent: 0,
            bytes_received: 0,
            host_rx_stream: VecDeque::new(),
            host_tx_stream: VecDeque::new(),
        }
    }

    /// Connecte le pont serie vers le port hote designe.
    pub fn connect(&mut self, port: &str) {
        self.port_name = port.to_string();
        self.is_connected = true;
    }

    /// Deconnecte le pont serie.
    pub fn disconnect(&mut self) {
        self.is_connected = false;
    }

    /// Ecrit des octets du cote de l'hote (simulant l'application externe type TamaHome).
    pub fn host_write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.host_rx_stream.push_back(b);
        }
    }

    /// Lit les octets recus par l'hote (emis par l'emulateur).
    pub fn host_read(&mut self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.host_tx_stream.len());
        while let Some(b) = self.host_tx_stream.pop_front() {
            out.push(b);
        }
        out
    }

    /// Synchronise les files d'emission et de reception entre le controleur UART et l'hote.
    pub fn sync(&mut self, uart: &mut UartController) {
        if !self.is_connected {
            return;
        }

        // 1. Recupere ce que l'emulateur a emis -> envoie vers l'hote
        let to_host = uart.drain_tx_bytes();
        if !to_host.is_empty() {
            self.bytes_sent += to_host.len();
            for b in to_host {
                self.host_tx_stream.push_back(b);
            }
        }

        // 2. Transfere ce que l'hote a envoye -> injecte dans la file de reception UART
        if !self.host_rx_stream.is_empty() {
            let mut from_host = Vec::with_capacity(self.host_rx_stream.len());
            while let Some(b) = self.host_rx_stream.pop_front() {
                from_host.push(b);
            }
            self.bytes_received += from_host.len();
            uart.inject_rx_bytes(&from_host);
        }
    }

    /// Transfert direct avec un flux implementant Read + Write.
    pub fn sync_with_stream<S: Read + Write>(&mut self, uart: &mut UartController, stream: &mut S) {
        // Envoi vers le flux
        let to_send = uart.drain_tx_bytes();
        if !to_send.is_empty() {
            if let Ok(n) = stream.write(&to_send) {
                self.bytes_sent += n;
            }
        }

        // Reception depuis le flux
        let mut buf = [0u8; 64];
        if let Ok(n) = stream.read(&mut buf) {
            if n > 0 {
                self.bytes_received += n;
                uart.inject_rx_bytes(&buf[..n]);
            }
        }
    }
}

// Alias de compatibilite
pub type UartBridge = UartHostBridge;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_bridge_bidirectional_transfer() {
        let mut bridge = UartHostBridge::new("COM10", 460800);
        let mut uart = UartController::new();

        // 1. L'hote ecrit un octet (0x55) a destination de la console
        bridge.host_write(&[0x55, 0xAA]);
        bridge.sync(&mut uart);

        // Verifie que l'UART a bien recu les 2 octets
        assert_eq!(uart.rx_fifo.len(), 2);
        assert_eq!(uart.read_reg(0x00), 0x55);
        assert_eq!(uart.read_reg(0x00), 0xAA);

        // 2. La console emet deux octets (0x12, 0x34)
        uart.write_reg(0x00, 0x12);
        uart.write_reg(0x00, 0x34);
        bridge.sync(&mut uart);

        // Verifie que l'hote les a bien recuperes
        let host_received = bridge.host_read();
        assert_eq!(host_received, vec![0x12, 0x34]);
    }
}
