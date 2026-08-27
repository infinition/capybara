use egui::Color32;

pub const LCD_WIDTH: usize = 128;
pub const LCD_HEIGHT: usize = 128;

pub struct DisplayController {
    pub ctrl: u32,
    pub fb_base_addr: u32,
    pub width: usize,
    pub height: usize,
    pub vram: Vec<u16>, // 128x128 RGB565 buffer
    pub is_enabled: bool,
    pub dirty: bool,
}

impl Default for DisplayController {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayController {
    pub fn new() -> Self {
        Self {
            ctrl: 0x01, // Enabled
            fb_base_addr: 0x2000_8000,
            width: LCD_WIDTH,
            height: LCD_HEIGHT,
            vram: vec![0x1084; LCD_WIDTH * LCD_HEIGHT], // Initial retro green-grey tint
            is_enabled: true,
            dirty: true,
        }
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            0x00 => self.ctrl,
            0x04 => self.fb_base_addr,
            0x08 => (self.width as u32) | ((self.height as u32) << 16),
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            0x00 => {
                self.ctrl = val;
                self.is_enabled = (val & 1) != 0;
            }
            0x04 => self.fb_base_addr = val,
            0x08 => {
                self.width = (val & 0xFFFF) as usize;
                self.height = (val >> 16) as usize;
            }
            _ => {}
        }
    }

    pub fn write_vram_pixel(&mut self, x: usize, y: usize, rgb565: u16) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            if idx < self.vram.len() {
                self.vram[idx] = rgb565;
                self.dirty = true;
            }
        }
    }

    pub fn sync_from_sram(&mut self, sram: &[u8]) {
        let vram_offset = (self.fb_base_addr.saturating_sub(0x2000_0000)) as usize;
        let pixel_count = self.width * self.height;
        let vram_byte_len = pixel_count * 2;
        if vram_offset + vram_byte_len <= sram.len() {
            for i in 0..pixel_count {
                let off = vram_offset + i * 2;
                let b0 = sram[off] as u16;
                let b1 = sram[off + 1] as u16;
                self.vram[i] = b0 | (b1 << 8);
            }
            self.dirty = true;
        }
    }

    pub fn get_rgba_buffer(&self) -> Vec<Color32> {
        let mut pixels = Vec::with_capacity(self.width * self.height);
        for &raw in &self.vram {
            let r = (((raw >> 11) & 0x1F) * 255 / 31) as u8;
            let g = (((raw >> 5) & 0x3F) * 255 / 63) as u8;
            let b = ((raw & 0x1F) * 255 / 31) as u8;
            pixels.push(Color32::from_rgb(r, g, b));
        }
        pixels
    }
}
