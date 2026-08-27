/// Controleur de la flash SPI NOR externe, base 0x40022000.
///
/// Deduit du journal des acces du firmware. Deux fonctions distinctes :
///
/// Interrogation de la puce, registres 0x04, 0x10 et 0x14 :
/// ```text
///   STR  index, [base, #0x10]
///   STR  0x800, [base, #0x04]     ; puis 0x4000
///   LDR  id,    [base, #0x14]     ; resultat
/// ```
/// Le firmware compare l'identifiant JEDEC obtenu a sa table de fabricants et
/// affiche "unsupport chip, please check your flash vender" quand il ne le
/// reconnait pas. Le protocole exact de cette lecture n'est pas encore etabli :
/// rendre l'identifiant du MX25L12835F octet par octet ne suffit pas a le
/// satisfaire, le role du registre 0x10 reste a determiner.
///
/// Transfert par DMA, registres 0x100 a 0x10C :
/// ```text
///   STR  adresse_flash, [base, #0x10c]
///   STR  longueur,      [base, #0x104]
///   STR  adresse_ram,   [base, #0x100]
///   STR  2,             [base, #0x108]   ; depart
///   LDR  statut,        [base, #0x108]   ; attente
/// ```
/// La copie est faite d'un coup a l'ecriture du depart, et le statut retombe
/// aussitot a zero : rien ne modelise ici la duree d'un transfert reel.
pub struct FlashController {
    pub ctrl: u32,
    pub command: u32,
    pub index: u32,
    /// Identifiant JEDEC rendu par le registre de donnee.
    pub jedec_id: u32,
    /// Rang de l'octet d'identifiant a rendre a la prochaine lecture.
    ///
    /// La fonction de lecture du firmware ne garde que l'octet de poids faible
    /// de 0x14 (LDRB.W apres le LDR), l'identifiant se lit donc octet par
    /// octet, comme sur la FIFO de reception d'un controleur SPI.
    pub id_index: usize,
    pub dma_mem_addr: u32,
    pub dma_len: u32,
    pub dma_flash_addr: u32,
    /// Derniere copie effectuee, pour l'affichage de diagnostic.
    pub last_transfer: Option<(u32, u32, u32)>,
}

pub const CTRL: u32 = 0x000;
pub const COMMAND: u32 = 0x004;
pub const INDEX: u32 = 0x010;
pub const DATA: u32 = 0x014;
pub const DMA_MEM: u32 = 0x100;
pub const DMA_LEN: u32 = 0x104;
pub const DMA_CTRL: u32 = 0x108;
pub const DMA_FLASH: u32 = 0x10C;
/// Bit de depart du transfert, ecrit dans DMA_CTRL.
pub const DMA_START: u32 = 0x2;

/// Macronix MX25L12835F : fabricant 0xC2, type 0x20, capacite 0x18 (128 Mbit).
/// C'est la puce reellement montee sur la console.
pub const MX25L12835F_JEDEC: u32 = 0x00C2_2018;

impl Default for FlashController {
    fn default() -> Self {
        Self {
            ctrl: 0,
            command: 0,
            index: 0,
            jedec_id: MX25L12835F_JEDEC,
            id_index: 0,
            dma_mem_addr: 0,
            dma_len: 0,
            dma_flash_addr: 0,
            last_transfer: None,
        }
    }
}

impl FlashController {
    /// Octet courant de l'identifiant, du fabricant vers la capacite.
    fn octet_identifiant(&self) -> u32 {
        let octets = [
            (self.jedec_id >> 16) & 0xFF,
            (self.jedec_id >> 8) & 0xFF,
            self.jedec_id & 0xFF,
        ];
        octets[self.id_index % octets.len()]
    }

    pub fn read_reg(&mut self, offset: u32) -> u32 {
        match offset {
            CTRL => self.ctrl,
            // Le firmware scrute ce registre en attendant la fin d'une commande.
            // Sans latence modelisee, elle est toujours terminee.
            COMMAND => 0,
            INDEX => self.index,
            DATA => {
                let v = self.octet_identifiant();
                self.id_index += 1;
                v
            }
            DMA_MEM => self.dma_mem_addr,
            DMA_LEN => self.dma_len,
            // Zero signifie inactif, donc transfert termine.
            DMA_CTRL => 0,
            DMA_FLASH => self.dma_flash_addr,
            _ => 0,
        }
    }

    /// Ecrit un registre. Rend la description d'un transfert a effectuer quand
    /// l'ecriture declenche un depart de DMA.
    ///
    /// Le controleur ne touche pas lui-meme a la memoire : c'est le bus qui
    /// realise la copie, seul a voir a la fois la flash et la SRAM.
    #[must_use]
    pub fn write_reg(&mut self, offset: u32, val: u32) -> Option<Transfer> {
        match offset {
            CTRL => self.ctrl = val,
            COMMAND => self.command = val,
            // Une nouvelle transaction repart du premier octet.
            INDEX => {
                self.index = val;
                self.id_index = 0;
            }
            DMA_MEM => self.dma_mem_addr = val,
            DMA_LEN => self.dma_len = val,
            DMA_FLASH => self.dma_flash_addr = val,
            DMA_CTRL if val & DMA_START != 0 => {
                let t = Transfer {
                    flash_offset: self.dma_flash_addr & 0x00FF_FFFF,
                    mem_addr: self.dma_mem_addr,
                    len: self.dma_len,
                };
                self.last_transfer = Some((t.flash_offset, t.mem_addr, t.len));
                return Some(t);
            }
            _ => {}
        }
        None
    }
}

/// Copie demandee par le DMA du controleur.
#[derive(Debug, Clone, Copy)]
pub struct Transfer {
    pub flash_offset: u32,
    pub mem_addr: u32,
    pub len: u32,
}
