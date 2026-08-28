/// Controleur de la flash SPI NOR externe, base 0x40022000.
///
/// Deduit du journal des acces du firmware. Deux fonctions distinctes :
///
/// Acces aux registres de la puce, registres 0x04, 0x10 et 0x14, releve en
/// 0x00005808 :
/// ```text
///   STR  valeur, [base, #0x10]    ; donnee a ecrire
///   ctrl |= 1 << 11               ; ordre d'ecriture, sur 0x04
///   ctrl |= 1 << 14               ; ordre de lecture
///   LDR  valeur, [base, #0x14]    ; donnee lue
/// ```
/// Le firmware appelle cette sequence avec -1 pour lire sans ecrire : la
/// comparaison `ADDS r0, r1, #1` en 0x00005816 saute alors l'ecriture.
/// Identification d'un bloc, registres 0x04 et 0x18 :
/// ```text
///   ctrl |= 1 << 15    ; sur 0x04, lance la lecture d'identification
///   attendre que le bit 15 retombe, puis le bit 1
///   LDR  paire, [base, #0x18]
/// ```
/// Le firmware compare le fabricant a sa table et se fige en imprimant
/// "unsupport chip, please check your flash vender" quand il ne le reconnait
/// pas. Ce message n'est pas du texte de repli : c'est une boucle sans sortie.
///
/// Transfert par DMA, registres 0x100 a 0x10C. Le registre de controle porte
/// deux bits distincts, releves dans la fonction de depart du firmware :
///
/// ```text
///   ctrl |= 2   ; direction : 1 = flash vers memoire, 0 = memoire vers flash
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
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FlashController {
    pub ctrl: u32,
    pub command: u32,
    pub index: u32,
    /// Paire d'identification rendue par le registre 0x18.
    pub reponse: u32,
    /// Registre de configuration de la puce, lu en 0x14 et ecrit par 0x10.
    ///
    /// Le firmware le lit d'abord sans rien ecrire et attend 0x40, en
    /// 0x0000918C ; s'il ne l'obtient pas il ecrit 64 et relit. C'est le bit
    /// Quad Enable du MX25L, deja pose sur une console qui demarre en quad. Son
    /// bit 0 est aussi le temoin d'ecriture en cours, que le firmware scrute
    /// apres chaque programmation : le laisser pose le fige.
    pub registre: u32,
    /// Derniere valeur deposee en 0x10, en attente de l'ordre d'ecriture.
    pub valeur_a_ecrire: u32,
    pub dma_mem_addr: u32,
    pub dma_len: u32,
    pub dma_flash_addr: u32,
    pub dma_ctrl: u32,
    /// Derniere copie effectuee, pour l'affichage de diagnostic.
    pub last_transfer: Option<(u32, u32, u32)>,
    /// Journal des transferts, avec leur sens. Sert a verifier qui ecrit en
    /// flash plutot qu'a le deduire de l'ordre des ecritures de registres.
    pub transferts: Vec<(u32, u32, u32, bool)>,
}

pub const CTRL: u32 = 0x000;
pub const COMMAND: u32 = 0x004;
pub const INDEX: u32 = 0x010;
pub const DATA: u32 = 0x014;
/// Paire d'identification, rendue d'un bloc : fabricant en bits 15:8,
/// composant en bits 7:0. C'est la meme reponse que le registre 0x14 rend
/// octet par octet.
///
/// Le firmware pose le bit 15 de COMMAND, attend qu'il retombe, attend aussi
/// que le bit 1 retombe, puis lit ce registre d'un seul LDR en 0x000039E8. Il
/// en fait `(valeur & 0xFFFF) << 8` puis compare les bits 23:16 a 0xC2
/// (Macronix) et 0xC8 (GigaDevice). Tout autre fabricant le fige dans une
/// boucle d'impression sans sortie, en 0x1006A018.
///
/// La verification a ete faite en imposant des valeurs arbitraires : 0x01020304
/// donne 0x00030400, ce qui etablit la transformation sans ambiguite.
pub const ID_JEDEC: u32 = 0x018;
/// Second mot d'identification, lu juste apres. Verifie sans effet : imposer
/// une valeur ici ne change rien au resultat construit par le firmware.
pub const ID_ETENDU: u32 = 0x01C;
pub const DMA_MEM: u32 = 0x100;
pub const DMA_LEN: u32 = 0x104;
pub const DMA_CTRL: u32 = 0x108;
pub const DMA_FLASH: u32 = 0x10C;
/// Bit de depart du transfert, dans DMA_CTRL.
pub const DMA_START: u32 = 0x1;
/// Bit de direction. Pose, le transfert va de la flash vers la memoire ; a zero
/// il remonte la memoire vers la flash.
///
/// Le sens est etabli par la trace : les trois premiers transferts lisent les
/// pages 0xD49000, 0xEFE000 et 0xEFF000 pour les valider, et ils ont ce bit
/// pose. Pris a l'envers, ils ecrasaient ces pages avec un tampon fraichement
/// alloue, rempli du motif de poison 0xAB.
pub const DMA_VERS_MEMOIRE: u32 = 0x2;

/// Reponse d'identification du composant : fabricant 0xC2, identifiant 0x17.
pub const MX25L12835F_REMS: u32 = 0x0000_C217;
/// Bit d'ordre d'ecriture d'un registre de la puce, dans COMMAND.
pub const CMD_ECRIRE_REGISTRE: u32 = 1 << 11;
/// Valeur du registre de configuration au repos : Quad Enable pose, aucune
/// ecriture en cours.
pub const REGISTRE_AU_REPOS: u32 = 0x40;

impl Default for FlashController {
    fn default() -> Self {
        Self {
            ctrl: 0,
            command: 0,
            index: 0,
            // Surchargeable pour balayer les candidats sans recompiler.
            reponse: std::env::var("SONIX_FLASH_ID")
                .ok()
                .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                .unwrap_or(MX25L12835F_REMS),
            registre: REGISTRE_AU_REPOS,
            valeur_a_ecrire: 0,
            dma_mem_addr: 0,
            dma_len: 0,
            dma_flash_addr: 0,
            dma_ctrl: 0,
            last_transfer: None,
            transferts: Vec::new(),
        }
    }
}

impl FlashController {
    pub fn read_reg(&mut self, offset: u32) -> u32 {
        match offset {
            CTRL => self.ctrl,
            // Le firmware scrute ce registre en attendant la fin d'une commande.
            // Sans latence modelisee, elle est toujours terminee.
            COMMAND => 0,
            INDEX => self.index,
            DATA => self.registre,
            ID_JEDEC => self.reponse,
            ID_ETENDU => 0,
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
            COMMAND => {
                self.command = val;
                if val & CMD_ECRIRE_REGISTRE != 0 {
                    self.registre = self.valeur_a_ecrire;
                }
            }
            // Ce registre porte la donnee a ecrire ; elle n'est prise en compte
            // qu'a l'ordre d'ecriture, sur COMMAND.
            INDEX => {
                self.index = val;
                self.valeur_a_ecrire = val;
            }
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
                        vers_memoire: val & DMA_VERS_MEMOIRE != 0,
                    };
                    self.last_transfer = Some((t.flash_offset, t.mem_addr, t.len));
                    if self.transferts.len() < 200 {
                        self.transferts.push((t.flash_offset, t.mem_addr, t.len, t.vers_memoire));
                    }
                    return Some(t);
                }
            }
            _ => {}
        }
        None
    }
}

/// Copie demandee par le DMA du controleur.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Transfer {
    pub flash_offset: u32,
    pub mem_addr: u32,
    pub len: u32,
    /// Sens du transfert : vrai pour flash vers memoire, faux pour l'inverse.
    pub vers_memoire: bool,
}
