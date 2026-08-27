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
/// Transfert par DMA, registres 0x100 a 0x10C. Le registre de controle porte
/// deux bits distincts, releves dans la fonction de depart du firmware :
///
/// ```text
///   ctrl |= 2   ; direction : 0 = flash vers memoire, 1 = memoire vers flash
///   ctrl &= ~2
///   ctrl |= 1   ; bit 0 : depart
/// ```
///
/// Le firmware procede par lecture-modification-ecriture sur ce registre : il
/// doit donc se relire tel qu'il a ete ecrit, sinon le bit de direction est
/// perdu entre les deux etapes et toute ecriture passe pour une lecture.
///
/// La copie est faite d'un coup au depart et le bit de depart retombe aussitot :
/// rien ne modelise ici la duree d'un transfert reel.
pub struct FlashController {
    pub ctrl: u32,
    pub command: u32,
    pub index: u32,
    /// Identifiant JEDEC rendu par le registre de donnee.
    pub jedec_id: u32,
    /// Reponse d'identification sur deux octets : fabricant puis composant.
    ///
    /// La fonction de lecture du firmware ne garde que l'octet de poids faible
    /// de 0x14 (LDRB.W apres le LDR), l'identifiant se lit donc octet par
    /// octet, comme sur la FIFO de reception d'un controleur SPI.
    pub reponse: u32,
    pub id_index: usize,
    pub dma_mem_addr: u32,
    pub dma_len: u32,
    pub dma_flash_addr: u32,
    pub dma_ctrl: u32,
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
/// Bit de depart du transfert, dans DMA_CTRL.
pub const DMA_START: u32 = 0x1;
/// Bit de direction. A zero le transfert va de la flash vers la memoire, pose
/// il remonte la memoire vers la flash. Le sens est etabli par l'ordre des
/// transferts : le tout premier lit une page de sauvegarde pour la valider, et
/// il a ce bit a zero.
pub const DMA_VERS_FLASH: u32 = 0x2;

/// Macronix MX25L12835F : fabricant 0xC2, type 0x20, capacite 0x18 (128 Mbit).
/// C'est la puce reellement montee sur la console.
pub const MX25L12835F_JEDEC: u32 = 0x00C2_2018;
/// Reponse REMS du meme composant : fabricant 0xC2, identifiant 0x17.
pub const MX25L12835F_REMS: u32 = 0x0000_C217;

impl Default for FlashController {
    fn default() -> Self {
        Self {
            ctrl: 0,
            command: 0,
            index: 0,
            jedec_id: MX25L12835F_JEDEC,
            // Surchargeable pour balayer les candidats sans recompiler.
            reponse: std::env::var("SONIX_FLASH_ID")
                .ok()
                .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                .unwrap_or(MX25L12835F_REMS),
            id_index: 0,
            dma_mem_addr: 0,
            dma_len: 0,
            dma_flash_addr: 0,
            dma_ctrl: 0,
            last_transfer: None,
        }
    }
}

impl FlashController {
    /// Octet courant de la reponse d'identification.
    ///
    /// Le firmware fait deux lectures par tentative, pas trois : c'est la
    /// signature de REMS (0x90), qui rend le fabricant puis l'identifiant de
    /// composant, et non RDID (0x9F) qui en rend trois. Le rang est choisi par
    /// le registre d'index plutot que par un compteur, pour que deux tentatives
    /// successives ne se dephasent pas.
    fn octet_identifiant(&self) -> u32 {
        let rang = if self.index == 0 { 0 } else { 1 };
        (self.reponse >> (8 * (1 - rang))) & 0xFF
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
            // Le bit de depart retombe des la fin du transfert, instantanee ici,
            // mais le reste du registre doit se relire tel qu'ecrit.
            DMA_CTRL => self.dma_ctrl & !DMA_START,
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
            // Le rang n'est pas remis a zero ici : le firmware ecrit 0x00 puis
            // 0x40 dans ce registre entre deux lectures successives, et attend
            // l'octet suivant, pas le meme. Remettre a zero lui rendait deux
            // fois l'octet de fabricant.
            INDEX => self.index = val,
            DMA_MEM => self.dma_mem_addr = val,
            DMA_LEN => self.dma_len = val,
            DMA_FLASH => self.dma_flash_addr = val,
            DMA_CTRL => {
                self.dma_ctrl = val;
                if val & DMA_START != 0 {
                    let t = Transfer {
                        flash_offset: self.dma_flash_addr & 0x00FF_FFFF,
                        mem_addr: self.dma_mem_addr,
                        len: self.dma_len,
                        vers_memoire: val & DMA_VERS_FLASH == 0,
                    };
                    self.last_transfer = Some((t.flash_offset, t.mem_addr, t.len));
                    return Some(t);
                }
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
    /// Sens du transfert : vrai pour flash vers memoire, faux pour l'inverse.
    pub vers_memoire: bool,
}
