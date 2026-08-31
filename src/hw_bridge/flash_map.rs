use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FlashSectionInfo {
    pub name: String,
    pub offset_start: usize,
    pub offset_end: usize,
    pub size_bytes: usize,
    pub description: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct FlashInspector {
    pub file_loaded: bool,
    pub file_size: usize,
    pub detected_edition: String,
    pub header_magic: String,
    pub xip_present: bool,
    pub arc2_assets_count: usize,
    pub arc2_total_bytes: usize,
    pub sections: Vec<FlashSectionInfo>,
}

impl Default for FlashInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl FlashInspector {
    pub fn new() -> Self {
        let sections = vec![
            FlashSectionInfo {
                name: "Firmware Header".to_string(),
                offset_start: 0x000000,
                offset_end: 0x000FFF,
                size_bytes: 0x1000,
                description: "Sonix SNC73410 boot descriptor & entry vectors".to_string(),
                status: "Ready".to_string(),
            },
            FlashSectionInfo {
                name: "Encrypted PRAM".to_string(),
                offset_start: 0x001000,
                offset_end: 0x010FFF,
                size_bytes: 0x10000,
                description: "Protected RAM firmware payload (AES encrypted)".to_string(),
                status: "Encrypted".to_string(),
            },
            FlashSectionInfo {
                name: "XIP Firmware".to_string(),
                offset_start: 0x011000,
                offset_end: 0x10FFFF,
                size_bytes: 0xFF000,
                description: "ARM Cortex-M3 game code (0x60011000)".to_string(),
                status: "XIP Mapped".to_string(),
            },
            FlashSectionInfo {
                name: "DPD Firmware".to_string(),
                offset_start: 0x110000,
                offset_end: 0x110FFF,
                size_bytes: 0x1000,
                description: "Deep Power Down low-power wake routines".to_string(),
                status: "Standby".to_string(),
            },
            FlashSectionInfo {
                name: "ARC2 Assets".to_string(),
                offset_start: 0x111000,
                offset_end: 0x8286C3,
                size_bytes: 0x7176C4,
                description: "ARC2 asset container: sprites, sounds, island maps".to_string(),
                status: "Asset Pack OK".to_string(),
            },
            FlashSectionInfo {
                name: "User Save Data".to_string(),
                offset_start: 0xD49000,
                offset_end: 0xFFFFFF,
                size_bytes: 0x2B7000,
                description: "Paradise pet state & diary memory".to_string(),
                status: "Persistent".to_string(),
            },
        ];

        Self {
            file_loaded: false,
            file_size: 16 * 1024 * 1024,
            detected_edition: "Paradise".to_string(),
            header_magic: "SONIXDEV".to_string(),
            xip_present: true,
            arc2_assets_count: 3,
            arc2_total_bytes: 0x7176B4,
            sections,
        }
    }

    pub fn inspect_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let mut file =
            File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        self.file_loaded = true;
        self.file_size = buffer.len();

        if buffer.len() >= 8 && &buffer[0..8] == b"SONIXDEV" {
            self.header_magic = "SONIXDEV (Sonix SNC73410)".to_string();
            
            // Check Edition from CRC at offset 0x24 or filename
            let crc = if buffer.len() >= 0x28 {
                u32::from_le_bytes([buffer[0x24], buffer[0x25], buffer[0x26], buffer[0x27]])
            } else {
                0
            };

            let path_str = path.as_ref().to_string_lossy().to_lowercase();
            if path_str.contains("jade") || crc == 0x2AD40D77 {
                self.detected_edition = "Paradise - Jade Forest Edition".to_string();
            } else if path_str.contains("water") {
                self.detected_edition = "Paradise - Water Edition".to_string();
            } else if path_str.contains("earth") {
                self.detected_edition = "Paradise - Earth Edition".to_string();
            } else if path_str.contains("land") {
                self.detected_edition = "Paradise - Land Edition".to_string();
            } else if path_str.contains("sky") {
                self.detected_edition = "Paradise - Sky Edition".to_string();
            } else {
                self.detected_edition = format!("Paradise (CRC: 0x{:08X})", crc);
            }

            // Inspect ARC2 header at 0x111000
            if buffer.len() >= 0x111020 && &buffer[0x111000..0x111004] == b"ARC2" {
                let arc_size = u32::from_le_bytes([
                    buffer[0x111008], buffer[0x111009], buffer[0x11100A], buffer[0x11100B],
                ]) as usize;
                let arc_tables = u32::from_le_bytes([
                    buffer[0x11100C], buffer[0x11100D], buffer[0x11100E], buffer[0x11100F],
                ]) as usize;
                self.arc2_total_bytes = arc_size;
                self.arc2_assets_count = arc_tables;
            }
        } else if buffer.len() >= 16 {
            self.header_magic = buffer[0..16]
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            self.detected_edition = "Generic Raw ARM Firmware".to_string();
        }

        self.xip_present = buffer.len() > 0x011000;
        Ok(())
    }
}
