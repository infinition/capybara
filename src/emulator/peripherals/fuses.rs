/// Zone systeme SN_SYS0, base 0x45000000.
///
/// Contient les fusibles de la puce. `FEUSE2` porte la cle AES sur ses 16 bits
/// de poids fort, `FEUSE3` la porte en mot complet selon la variante. C'est la
/// que le bootrom va chercher la deviceKey pour deriver l'IV du code chiffre,
/// et c'est pour cela qu'elle est absente du dump de flash.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FuseRegisters {
    pub device_key: Option<u32>,
    pub feuse0: u32,
    pub feuse1: u32,
}

pub const FEUSE0: u32 = 0x30;
pub const FEUSE1: u32 = 0x34;
pub const FEUSE2: u32 = 0x38;
pub const FEUSE3: u32 = 0x3c;

impl Default for FuseRegisters {
    fn default() -> Self {
        Self { device_key: None, feuse0: 0, feuse1: 0 }
    }
}

impl FuseRegisters {
    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            FEUSE0 => self.feuse0,
            FEUSE1 => self.feuse1,
            // La cle occupe les 16 bits de poids fort de FEUSE2.
            FEUSE2 => self.device_key.map(|k| k & 0xFFFF_0000).unwrap_or(0),
            FEUSE3 => self.device_key.unwrap_or(0),
            _ => 0,
        }
    }

    /// Les fusibles sont graves, aucune ecriture n'est repercutee.
    pub fn write_reg(&mut self, _offset: u32, _val: u32) {}
}
