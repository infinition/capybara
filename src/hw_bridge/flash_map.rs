use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FlashSectionInfo {
    pub name: &'static str,
    pub offset_start: usize,
    pub offset_end: usize,
    pub size_bytes: usize,
    pub description: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Clone)]
pub struct FlashInspector {
    pub file_loaded: bool,
    pub file_size: usize,
    pub header_magic: String,
    pub xip_present: bool,
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
                name: "Firmware Header",
                offset_start: 0x000000,
                offset_end: 0x000FFF,
                size_bytes: 0x1000,
                description: "Boot configuration & entry point vectors",
                status: "Ready",
            },
            FlashSectionInfo {
                name: "Encrypted PRAM",
                offset_start: 0x001000,
                offset_end: 0x010FFF,
                size_bytes: 0x10000,
                description: "Protected RAM firmware payload (AES encrypted)",
                status: "Encrypted",
            },
            FlashSectionInfo {
                name: "XIP Firmware",
                offset_start: 0x011000,
                offset_end: 0x10FFFF,
                size_bytes: 0xFF000,
                description: "Execute-In-Place ARM Cortex-M3 game code",
                status: "XIP Mapped",
            },
            FlashSectionInfo {
                name: "DPD Firmware",
                offset_start: 0x110000,
                offset_end: 0x110FFF,
                size_bytes: 0x1000,
                description: "Deep Power Down low-power wake routines",
                status: "Standby",
            },
            FlashSectionInfo {
                name: "Assets Region",
                offset_start: 0x111000,
                offset_end: 0x8286C3,
                size_bytes: 0x7176C4,
                description: "Sprites, animation sheets, audio PCM & island data",
                status: "Asset Pack OK",
            },
        ];

        Self {
            file_loaded: false,
            file_size: 16 * 1024 * 1024, // 128 Mbit (16 MB)
            header_magic: "SONIX_SNC73410_OK".to_string(),
            xip_present: true,
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

        if buffer.len() >= 16 {
            self.header_magic = buffer[0..16]
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
        }

        self.xip_present = buffer.len() > 0x011000;
        Ok(())
    }
}
