/// Convertisseur analogique-numerique a approximations successives.
///
/// Deux instances existent, 0x4000A000 et 0x4000B000. Le firmware les distingue
/// par un parametre, ecrit le canal voulu dans le registre 0x00, puis attend le
/// bit 6 du registre 0x14 :
///
/// ```text
///   STR  r0, [base, #0]        ; canal
///   LDR  r0, [base, #0x14]     ; statut
///   LSLS r0, r0, #25           ; amene le bit 6 sur le signe
///   BMI  fini
/// ```
///
/// Sans peripherique reel derriere, la conversion est instantanee : le bit de
/// fin est pose des qu'un canal a ete demande. Seuls ces deux registres sont
/// modelises, pour que les autres restent visibles dans la trace MMIO.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SarAdc {
    pub channel: u32,
    /// Une conversion a ete demandee, donc le resultat est disponible.
    pub converted: bool,
    /// Valeur rendue dans les bits bas du registre de statut.
    ///
    /// Le registre de resultat n'a pas ete localise : le firmware n'est jamais
    /// vu en train de lire un autre offset de cette page. La faire varier de
    /// zero a pleine echelle ne change rien au comportement observe, cette
    /// valeur est donc une commodite, pas un fait etabli.
    pub valeur: u32,
}

/// Mesure correspondant a une pile pleine, sur douze bits.
pub const PILE_PLEINE: u32 = 0x0C00;

impl Default for SarAdc {
    fn default() -> Self {
        Self {
            channel: 0,
            converted: false,
            valeur: std::env::var("SONIX_ADC")
                .ok()
                .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                .unwrap_or(PILE_PLEINE),
        }
    }
}

pub const CHANNEL: u32 = 0x00;
pub const STATUS: u32 = 0x14;
/// Bit de fin de conversion dans le registre de statut.
pub const STATUS_DONE: u32 = 1 << 6;

impl SarAdc {
    /// Indique si l'offset fait partie des registres modelises.
    pub fn handles(offset: u32) -> bool {
        matches!(offset, CHANNEL | STATUS)
    }

    pub fn read_reg(&self, offset: u32) -> u32 {
        match offset {
            CHANNEL => self.channel,
            STATUS => {
                if self.converted {
                    STATUS_DONE | (self.valeur & 0x0FFF)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, offset: u32, val: u32) {
        if offset == CHANNEL {
            self.channel = val;
            self.converted = true;
        }
    }
}
