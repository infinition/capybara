/// Controleur de la fenetre XIP cachee, base 0x4002F000.
///
/// La fenetre de 1 Mo a 0x10000000 n'est pas figee sur le debut de la flash :
/// le firmware y installe la base de son choix. La sequence observee dans le
/// dump ecrit l'adresse issue du bloc boot-info (+0x818) dans BASE, puis 3 dans
/// CTRL. C'est ce qui explique qu'un saut vers 0x1006D1C4 vise en realite
/// l'offset flash 0x11000 + 0x6D1C4.
pub struct XipController {
    pub ctrl: u32,
    /// Adresse flash mappee au debut de la fenetre, dans l'espace 0x60000000.
    pub base: u32,
}

pub const CTRL: u32 = 0x00;
pub const BASE: u32 = 0x04;

/// Base retenue tant que le firmware n'en a pas programme une.
pub const DEFAULT_BASE: u32 = 0x6000_0000;

impl Default for XipController {
    fn default() -> Self {
        Self { ctrl: 0, base: DEFAULT_BASE }
    }
}

impl XipController {
    /// Offset flash correspondant a une adresse de la fenetre cachee.
    pub fn flash_offset(&self, window_offset: u32) -> usize {
        ((self.base & 0x00FF_FFFF) + window_offset) as usize
    }

    pub fn is_enabled(&self) -> bool {
        self.ctrl != 0
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            CTRL => self.ctrl,
            BASE => self.base,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        match offset {
            CTRL => self.ctrl = val,
            BASE => self.base = val,
            _ => {}
        }
    }
}
