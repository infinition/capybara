#[derive(Debug, Clone)]
pub struct UartPacket {
    pub opcode: u8,
    pub payload: Vec<u8>,
    pub checksum: u16,
}

pub struct UartBridge {
    pub baud_rate: u32,
    pub is_connected: bool,
    pub bytes_sent: usize,
    pub bytes_received: usize,
}

impl Default for UartBridge {
    fn default() -> Self {
        Self {
            baud_rate: 460800,
            is_connected: true,
            bytes_sent: 1024,
            bytes_received: 4096,
        }
    }
}

impl UartBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn encode_packet(opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mut packet = vec![0xAB, 0x5D, 0xEB, 0xEF]; // Sync magic pattern
        packet.push(opcode);
        let len = payload.len() as u16;
        packet.extend_from_slice(&len.to_le_bytes());
        packet.extend_from_slice(payload);

        // Simple checksum
        let mut sum: u16 = 0;
        for &b in payload {
            sum = sum.wrapping_add(b as u16);
        }
        packet.extend_from_slice(&sum.to_le_bytes());
        packet
    }
}
