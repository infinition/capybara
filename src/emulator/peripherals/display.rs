use egui::Color32;

pub const LCD_WIDTH: usize = 128;
pub const LCD_HEIGHT: usize = 128;

/// Registre de donnees du panneau, destination des trames poussees par le
/// controleur de transferts.
///
/// Le pilote garde ses adresses dans un descripteur en 0x1801C9C0 : le premier
/// mot vaut 0x4000E000, et il programme la destination du canal a cette base
/// plus 0x1C, en 0x00004506.
pub const PANNEAU_DONNEES: u32 = 0x4000_E01C;

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
struct PanelStream {
    command: u8,
    params: Vec<u8>,
    columns: [u16; 2],
    rows: [u16; 2],
    x: u16,
    y: u16,
    high: Option<u8>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DisplayController {
    pub ctrl: u32,
    pub fb_base_addr: u32,
    pub width: usize,
    pub height: usize,
    pub vram: Vec<u16>, // RGB565 framebuffer
    pub is_enabled: bool,
    pub dirty: bool,
    /// Nombre de trames recues du controleur de transferts. Un compteur fige
    /// est le signe le plus direct d'un blocage.
    pub trames: u64,
    #[serde(default)]
    stream: PanelStream,
    #[serde(default)]
    pub my_lab: bool,
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
            fb_base_addr: 0x1800_8000,
            width: LCD_WIDTH,
            height: LCD_HEIGHT,
            vram: vec![0x1084; LCD_WIDTH * LCD_HEIGHT], // Initial retro green-grey tint
            is_enabled: true,
            dirty: true,
            trames: 0,
            stream: PanelStream::default(),
            my_lab: false,
        }
    }

    pub fn for_my_lab() -> Self {
        let mut lcd = Self { my_lab: true, ..Self::default() };
        lcd.resize(160, 128);
        lcd
    }

    fn resize(&mut self, width: usize, height: usize) {
        if width != 0 && height != 0 && width <= 512 && height <= 512 {
            self.width = width;
            self.height = height;
            self.vram.resize(width * height, 0);
            self.dirty = true;
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
                self.resize((val & 0xFFFF) as usize, (val >> 16) as usize);
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

    /// Direct SPI writes used by homebrew, alongside the factory DMA path.
    /// P0.5 selects command/data; panel coordinates include a 2,1 border.
    pub fn write_panel_byte(&mut self, data: bool, byte: u8) {
        if !data {
            self.stream.command = byte;
            self.stream.params.clear();
            self.stream.high = None;
            match byte {
                0x2C => {
                    self.stream.x = self.stream.columns[0];
                    self.stream.y = self.stream.rows[0];
                }
                0x28 | 0x10 => self.is_enabled = false,
                0x29 => self.is_enabled = true,
                _ => {}
            }
            return;
        }
        match self.stream.command {
            0x2A | 0x2B => {
                if self.stream.params.len() == 4 { return; }
                self.stream.params.push(byte);
                if self.stream.params.len() == 4 {
                    let p = &self.stream.params;
                    let range = [u16::from_be_bytes([p[0], p[1]]), u16::from_be_bytes([p[2], p[3]])];
                    if self.stream.command == 0x2A { self.stream.columns = range; }
                    else { self.stream.rows = range; }
                }
            }
            0x2C => {
                if let Some(high) = self.stream.high.take() {
                    self.write_panel_pixel(u16::from_be_bytes([high, byte]));
                } else { self.stream.high = Some(byte); }
            }
            _ => {}
        }
    }

    fn write_panel_pixel(&mut self, pixel: u16) {
        let (x, y) = (self.stream.x, self.stream.y);
        if self.my_lab {
            // The LCD controller uses portrait coordinates in the landscape shell.
            if let Some(column) = (self.width - 1).checked_sub(y as usize) {
                self.write_vram_pixel(column, x as usize, pixel);
            }
        } else if x >= 2 && y >= 1 {
            self.write_vram_pixel((x - 2) as usize, (y - 1) as usize, pixel);
        }
        if x >= self.stream.columns[1] {
            self.stream.x = self.stream.columns[0];
            if y >= self.stream.rows[1] {
                self.stream.y = self.stream.rows[0];
                self.trames += 1;
            } else {
                self.stream.y = y.saturating_add(1);
            }
        } else {
            self.stream.x = x.saturating_add(1);
        }
    }

    /// Recoit une trame entiere poussee par le controleur de transferts.
    ///
    /// C'est le chemin reel de l'afficheur : le firmware ne fait jamais ecrire
    /// le panneau par le coeur, il programme un canal qui deverse le tampon
    /// d'image dans le registre de donnees. Une trame plus courte que l'ecran
    /// n'ecrase que son debut, une trame plus longue est tronquee.
    pub fn recevoir_trame(&mut self, pixels: &[u16]) {
        if self.my_lab && self.stream.command == 0x2c {
            for &pixel in pixels {
                self.write_panel_pixel(pixel);
            }
            return;
        }
        let n = pixels.len().min(self.vram.len());
        self.vram[..n].copy_from_slice(&pixels[..n]);
        self.dirty = true;
        self.trames += 1;
    }

    pub fn sync_from_sram(&mut self, sram: &[u8]) {
        let vram_offset = (self.fb_base_addr.saturating_sub(0x1800_0000)) as usize;
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
