pub mod aes;
pub mod cpu;
pub mod edition;
pub mod etat;
pub mod loader;
pub mod mmu;
pub mod sauvegarde;
pub mod peripherals;
pub mod sonix;

pub use cpu::{Cpu, DisassembledInst, Disassembler, Mode, Registers, StepResult};
pub use edition::Edition;
pub use loader::{FirmwareLoader, ImageKind, LoadReport, LoadedRegion};
pub use mmu::{BootRom, InternalSram, LogEntry, MemoryBus, MmioStat, MmioTrace, Pram, SpiFlash};
pub use peripherals::{
    DisplayController, FuseRegisters, GpioController, Peripherals, SysRegisters, Timers,
    UartController,
};

use std::collections::HashSet;
use std::path::Path;

/// Raison pour laquelle l'execution s'est arretee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    Breakpoint(u32),
    Halted(u32),
    /// Instruction non decodee : l'emulateur ne sait pas executer ce code.
    Undefined { pc: u32, opcode: u32 },
}

pub struct Machine {
    pub cpu: Cpu,
    pub bus: MemoryBus,
    pub periph: Peripherals,
    pub breakpoints: HashSet<u32>,
    pub is_running: bool,
    pub instructions_per_frame: u32,
    /// Console de debug du firmware, telle qu'elle sortirait sur l'UART.
    ///
    /// Dans la boucle de formatage du printf, l'instruction 0x00001070 appelle
    /// la sortie avec le caractere dans r0. L'intercepter donne le journal
    /// complet sans modeliser le port serie.
    pub console: String,
    pub firmware_path: Option<String>,
    /// Cle de la puce, indispensable pour dechiffrer un dump chiffre.
    pub device_key: Option<u32>,
    pub last_report: Option<LoadReport>,
    pub last_stop: Option<StopReason>,
    /// Empreinte du dump charge, qui range ses sauvegardes a part.
    pub empreinte: Option<String>,
    /// Edition reconnue, qui donne la couleur de la coque.
    pub edition: Edition,
    /// Fichier de sauvegarde suivi. Sans lui, la partie ne vit que le temps de
    /// la session, comme avant.
    pub sauvegarde_active: Option<std::path::PathBuf>,
    /// Revision de flash deja recopiee sur le disque.
    pub revision_ecrite: u64,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Self {
        let mut bus = MemoryBus::default();
        let mut periph = Peripherals::default();
        FirmwareLoader::install_idle_state(&mut bus);

        let mut cpu = Cpu::default();
        cpu.reset(&mut bus, &mut periph);

        Self {
            cpu,
            bus,
            periph,
            breakpoints: HashSet::new(),
            // Sans firmware charge, rien ne tourne.
            is_running: false,
            instructions_per_frame: 20_000,
            console: String::new(),
            firmware_path: None,
            device_key: None,
            last_report: None,
            last_stop: None,
            empreinte: None,
            edition: Edition::default(),
            sauvegarde_active: None,
            revision_ecrite: 0,
        }
    }

    /// Vrai quand le jeu a ecrit dans sa flash depuis la derniere copie sur le
    /// disque. C'est le seul signal a surveiller pour tenir la sauvegarde a
    /// jour sans comparer seize mega-octets a chaque image.
    pub fn sauvegarde_a_ecrire(&self) -> bool {
        self.sauvegarde_active.is_some() && self.bus.flash.revision != self.revision_ecrite
    }

    /// Ouvre un emplacement de sauvegarde et y verse son contenu.
    ///
    /// A appeler juste apres le chargement du dump. Un emplacement qui n'existe
    /// pas encore est accepte : c'est une partie neuve, qui s'ecrira des que le
    /// jeu sauvegardera.
    pub fn ouvrir_sauvegarde(&mut self, chemin: std::path::PathBuf) -> Result<bool, String> {
        let existe = chemin.exists();
        if existe {
            sauvegarde::Sauvegarde::lire(&chemin)?.appliquer(self);
        }
        self.revision_ecrite = self.bus.flash.revision;
        self.sauvegarde_active = Some(chemin);
        Ok(existe)
    }

    /// Ferme l'emplacement suivi, sans rien effacer sur le disque.
    pub fn fermer_sauvegarde(&mut self) {
        self.sauvegarde_active = None;
    }

    /// Recopie les pages ecrites par le jeu dans le fichier suivi.
    pub fn ecrire_sauvegarde(&mut self) -> Result<(), String> {
        let Some(chemin) = self.sauvegarde_active.clone() else {
            return Ok(());
        };
        sauvegarde::Sauvegarde::depuis(self).ecrire(&chemin)?;
        self.revision_ecrite = self.bus.flash.revision;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus, &mut self.periph);
        self.last_stop = None;
    }

    pub fn step(&mut self) -> StepResult {
        // Meme raison que dans run_frame : le drapeau est teste ici, l'appel
        // n'a lieu que le jour ou il y a vraiment un reveil a appliquer.
        if self.periph.snsys.reveil_demande && self.reveil_materiel() {
            return StepResult::Ok(1);
        }
        self.cpu.step(&mut self.bus, &mut self.periph)
    }

    /// Applique le reveil materiel quand il y en a un a appliquer.
    ///
    /// La veille profonde n'a pas de sortie logicielle : le firmware programme
    /// son echeance en `0x45000230`, s'endort, et c'est le bloc d'horloge qui
    /// rallume le coeur. Le firmware retrouve ensuite la raison du reveil dans
    /// le statut `0x45000234`, qu'on a pose en meme temps.
    fn reveil_materiel(&mut self) -> bool {
        if !self.periph.snsys.reveil_demande {
            return false;
        }
        self.periph.snsys.reveil_demande = false;
        if !self.en_veille_profonde() {
            return false;
        }
        self.reset();
        self.is_running = true;
        true
    }

    pub fn run_frame(&mut self) -> StepResult {
        if !self.is_running {
            return StepResult::Halt;
        }

        // Sans point d'arret pose, il n'y a rien a chercher. La table est une
        // table de hachage : l'interroger a chaque instruction coutait un
        // hachage complet par pas, soit plus cher que le decodage lui meme, et
        // pour rien la quasi totalite du temps. La sonde de vitesse passait a
        // cote, elle appelle step sans passer par ici.
        let poses = !self.breakpoints.is_empty();

        let par_trame = self.instructions_per_frame;
        let mut executed = 0;
        while executed < par_trame {
            let pc = self.cpu.regs.pc;
            if poses && self.breakpoints.contains(&pc) {
                self.is_running = false;
                self.last_stop = Some(StopReason::Breakpoint(pc));
                return StepResult::Breakpoint;
            }
            if pc == Self::SORTIE_CONSOLE {
                let c = (self.cpu.regs.get_reg(0) & 0xFF) as u8;
                if c == 10 || (0x20..0x7F).contains(&c) {
                    self.console.push(c as char);
                }
                // Le journal ne sert qu'au diagnostic : on borne sa taille.
                if self.console.len() > 8000 {
                    let reste = self.console.split_off(self.console.len() - 4000);
                    self.console = reste;
                }
            }

            // Le drapeau est teste ici plutot que dans l'appel : un reveil
            // materiel arrive une fois par mise en veille, l'appel de fonction
            // arrivait a chaque instruction.
            if self.periph.snsys.reveil_demande && self.reveil_materiel() {
                executed += 1;
                continue;
            }
            match self.cpu.step(&mut self.bus, &mut self.periph) {
                StepResult::Ok(_) => executed += 1,
                StepResult::Breakpoint => {
                    self.is_running = false;
                    self.last_stop = Some(StopReason::Breakpoint(pc));
                    return StepResult::Breakpoint;
                }
                StepResult::Halt => {
                    self.is_running = false;
                    self.last_stop = Some(StopReason::Halted(pc));
                    return StepResult::Halt;
                }
                // Une instruction non decodee fausse tout ce qui suit. On s'arrete
                // au lieu de continuer sur un etat de registres devenu faux.
                StepResult::Undefined(op) => {
                    self.is_running = false;
                    self.last_stop = Some(StopReason::Undefined { pc, opcode: op as u32 });
                    return StepResult::Undefined(op);
                }
            }
        }

        // L'afficheur n'est plus recopie depuis la SRAM : il recoit les trames
        // que le controleur de transferts lui pousse, comme sur la console.
        StepResult::Ok(executed)
    }

    /// Charge un dump et prepare le demarrage du vrai firmware.
    /// Adresses des deux pages de sauvegarde, principale puis copie.
    pub const PAGES_SAUVEGARDE: [usize; 2] = [0xEFE000, 0xEFF000];
    /// Longueur d'une page de sauvegarde, en-tete compris.
    pub const TAILLE_PAGE_SAUVEGARDE: usize = 0x1000;
    /// Polynome de la somme de controle des pages de sauvegarde, celui que le
    /// firmware programme dans l'accelerateur en 0x1000569E.
    pub const POLYNOME_SAUVEGARDE: u16 = 0xA001;
    /// Drapeau de pile faible, bit 3 du premier octet de l'etat sauvegarde.
    ///
    /// Le firmware le lit en 0x10030E54, imprime
    /// "** LOW BATTERY FLAG DETECTED **" et passe a l'etat 111, qui affiche
    /// "remplacez la pile" puis eteint la console. Le dump d'origine porte ce
    /// drapeau : la console etait en fin de pile au moment de l'extraction.
    pub const DRAPEAU_PILE_FAIBLE: u8 = 1 << 3;

    /// Somme de controle d'une page de sauvegarde, sur ses 0xFFC octets utiles.
    fn somme_sauvegarde(&self, page: usize) -> u16 {
        let mut crc: u16 = 0;
        for i in 4..Self::TAILLE_PAGE_SAUVEGARDE {
            crc ^= self.bus.flash.read_u8(page + i) as u16;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ Self::POLYNOME_SAUVEGARDE
                } else {
                    crc >> 1
                };
            }
        }
        crc
    }

    /// Efface le drapeau de pile faible des deux pages de sauvegarde et remet
    /// leur en-tete d'accord avec le contenu.
    ///
    /// C'est l'equivalent exact du geste physique : sans cela le firmware
    /// affiche son message de pile a remplacer et s'eteint, quel que soit le
    /// reste du modele.
    pub fn remplacer_la_pile(&mut self) {
        for page in Self::PAGES_SAUVEGARDE {
            let etat = self.bus.flash.read_u8(page + 4);
            if etat & Self::DRAPEAU_PILE_FAIBLE == 0 {
                continue;
            }
            self.bus.flash.write_u8(page + 4, etat & !Self::DRAPEAU_PILE_FAIBLE);
            let somme = self.somme_sauvegarde(page);
            self.bus.flash.write_u8(page, (somme & 0xFF) as u8);
            self.bus.flash.write_u8(page + 1, (somme >> 8) as u8);
            let complement = !somme;
            self.bus.flash.write_u8(page + 2, (complement & 0xFF) as u8);
            self.bus.flash.write_u8(page + 3, (complement >> 8) as u8);
        }
    }

    /// Instruction qui appelle la sortie caractere du printf de debug, avec le
    /// caractere dans r0.
    pub const SORTIE_CONSOLE: u32 = 0x0000_1070;

    /// Drapeau pose par le firmware tant qu'un son joue, teste par son moteur
    /// en `0x1007922C` avant de faire quoi que ce soit.
    pub const SON_EN_COURS: u32 = 0x1801_4284;
    /// Tableau des voix du moteur audio, huit entrees de 0x34 octets.
    ///
    /// L'allocation, en `0x10022BE2`, indexe ce tableau par le type de son.
    /// Une voix porte l'horloge du coeur en tete, son compte de rechargement
    /// en `+4`, un temoin d'activite en `+8` et son volume en `+0xC`. Le compte
    /// n'est pas une frequence : voir `BASE_DE_TEMPS_AUDIO`.
    pub const VOIX_AUDIO: u32 = 0x1801_C820;
    pub const TAILLE_VOIX: u32 = 0x34;
    pub const NOMBRE_VOIX: u32 = 8;

    /// Base de temps du generateur de notes, en hertz.
    ///
    /// Le champ `+4` d'une voix n'est pas une frequence, c'est un compte de
    /// rechargement : la hauteur vaut cette base divisee par lui. Trois choses
    /// le montrent, et la premiere suffit.
    ///
    /// Les valeurs relevees sur des notes reelles, 4545, 1911, 1516, 1351, 955,
    /// 758 et 568, ne tombent sur la gamme temperee que prises ainsi, a trois
    /// cents pres : Mi3, Sol4, Si4, Do#5, Sol5, Si5 et Mi6. Lues comme des
    /// hertz elles en sont toutes a quarante deux cents, presque un quart de
    /// ton, et un firmware ne compose pas faux de facon aussi reguliere.
    ///
    /// La hauteur etant l'inverse du compte, le contour de chaque melodie
    /// s'inverse : une suite qui monte dans le tableau descend a l'oreille.
    /// C'est ce qui faisait entendre les melodies a l'envers.
    ///
    /// L'octave, elle, ne se deduit pas de la gamme : doubler ou diviser par
    /// deux garde toutes les notes justes. Elle a ete calee a l'oreille contre
    /// la console posee a cote, et donne 750 000. Ce chiffre est le plus
    /// naturel des deux pour du materiel : c'est 96 MHz divises par 64, donc
    /// une base de 1,5 MHz, puis par deux parce qu'un timer en carre bascule sa
    /// sortie a chaque comparaison et met donc deux comparaisons par periode.
    /// Le son de validation vaut alors Do#5 puis Sol4.
    pub const BASE_DE_TEMPS_AUDIO: f32 = 750_000.0;

    /// Hauteur d'une voix, en hertz, a partir du compte range dans son champ.
    ///
    /// Rend zero hors de la bande audible : c'est alors une voix au repos ou un
    /// champ mal lu, pas une note.
    pub fn hauteur_de_voix(compte: u32) -> f32 {
        if compte == 0 {
            return 0.0;
        }
        let hz = Self::BASE_DE_TEMPS_AUDIO / compte as f32;
        if (20.0..=12_000.0).contains(&hz) {
            hz
        } else {
            0.0
        }
    }

    /// Vrai quand le firmware est en train de jouer un son.
    pub fn son_en_cours(&self) -> bool {
        self.lire_sram_u8(Self::SON_EN_COURS) != 0
    }

    /// Frequence de la note en cours, zero au silence.
    ///
    /// Version sans allocation de `voix_audio`, appelee tres souvent pour
    /// suivre la melodie note par note au lieu de l'echantillonner a la cadence
    /// de l'interface, bien trop grossiere : une melodie dure cent cinquante
    /// millisecondes et l'interface ne rend que soixante images par seconde.
    pub fn note_courante(&self) -> f32 {
        if !self.son_en_cours() {
            return 0.0;
        }
        for i in 0..Self::NOMBRE_VOIX {
            let base = Self::VOIX_AUDIO + i * Self::TAILLE_VOIX;
            if self.lire_sram_u8(base + 8) == 0 {
                continue;
            }
            let hauteur = Self::hauteur_de_voix(self.lire_sram_u32(base + 4));
            if hauteur > 0.0 && self.lire_sram_u32(base + 0xC) > 0 {
                return hauteur;
            }
        }
        0.0
    }

    /// Frequences et volumes des voix actives, telles que le firmware les a
    /// calculees.
    ///
    /// Le buzzer de la console est un signal carre : reproduire ces frequences
    /// rend donc le vrai son, sans avoir a modeliser le peripherique de sortie,
    /// que le firmware n'atteint pas dans le modele actuel.
    pub fn voix_audio(&self) -> Vec<(f32, f32)> {
        if !self.son_en_cours() {
            return Vec::new();
        }
        (0..Self::NOMBRE_VOIX)
            .filter_map(|i| {
                let base = Self::VOIX_AUDIO + i * Self::TAILLE_VOIX;
                // L'octet en `+8` distingue la voix qui joue de celles qui
                // gardent seulement leur derniere valeur : le tableau reste
                // rempli au silence, seule celle la est en cours.
                if self.lire_sram_u8(base + 8) == 0 {
                    return None;
                }
                let hauteur = Self::hauteur_de_voix(self.lire_sram_u32(base + 4));
                let volume = self.lire_sram_u32(base + 0xC);
                if hauteur > 0.0 && volume > 0 {
                    Some((hauteur, (volume.min(100) as f32) / 100.0))
                } else {
                    None
                }
            })
            .collect()
    }

    fn lire_sram_u8(&self, adresse: u32) -> u8 {
        self.bus
            .sram
            .data
            .get((adresse - 0x1800_0000) as usize)
            .copied()
            .unwrap_or(0)
    }

    fn lire_sram_u32(&self, adresse: u32) -> u32 {
        let o = (adresse - 0x1800_0000) as usize;
        let d = &self.bus.sram.data;
        if o + 4 > d.len() {
            return 0;
        }
        u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
    }

    /// Boutons de la console, avec l'identifiant que le firmware leur donne :
    /// port dans les bits hauts, broche dans les quatre bits bas.
    pub const BOUTON_MOLETTE: u32 = 0x08;
    pub const BOUTON_A: u32 = 0x09;
    pub const BOUTON_C: u32 = 0x0A;
    pub const BOUTON_B: u32 = 0x0B;
    pub const ENCODEUR_1: u32 = 0x20;
    pub const ENCODEUR_2: u32 = 0x21;

    /// Port correspondant a un identifiant de broche, s'il est modelise.
    fn port_de(&mut self, id: u32) -> Option<&mut crate::emulator::peripherals::GpioPort> {
        match id >> 4 {
            0 => Some(&mut self.periph.port0),
            1 => Some(&mut self.periph.port1),
            2 => Some(&mut self.periph.port2),
            _ => None,
        }
    }

    /// Boucle de veille profonde du firmware, en PRAM.
    ///
    /// Elle demande la mise hors tension du coeur par le bit 0 de
    /// `0x45000300`, execute un `WFI`, puis se rebranche sur elle meme sans
    /// aucune condition de sortie : le saut de `0x00002432` vers `0x000023D0`
    /// est inconditionnel, et les deux seules interruptions restees autorisees,
    /// 2 et 3, ont des gestionnaires qui reviennent dans la boucle. Aucune
    /// sortie logicielle n'existe donc, et le reveil ne peut venir que du
    /// materiel, qui remet le coeur a zero. C'est ce que reproduit `appuyer`.
    pub const VEILLE_PROFONDE: std::ops::Range<u32> = 0x0000_23D0..0x0000_2434;

    /// Vrai quand le coeur est gare dans cette boucle.
    pub fn en_veille_profonde(&self) -> bool {
        Self::VEILLE_PROFONDE.contains(&self.cpu.regs.pc)
    }

    /// Tire une broche vers le bas, ce que fait un appui.
    ///
    /// Les entrees sont a resistance de tirage : au repos elles se lisent
    /// hautes, un appui les tire bas. C'est la convention que le firmware
    /// attend, verifiee sur les broches 0x20 et 0x21 de l'encodeur.
    ///
    /// En veille profonde, l'appui ne tire pas seulement la broche : il rallume
    /// la console. La memoire vive est effacee par le demarrage du firmware,
    /// mais la sauvegarde est en flash et l'horloge continue de tourner, donc
    /// la partie reprend la ou elle en etait.
    pub fn appuyer(&mut self, id: u32) {
        if self.en_veille_profonde() {
            self.periph.snsys.declencher_reveil();
            self.reveil_materiel();
            return;
        }
        let broche = id & 0xF;
        if let Some(port) = self.port_de(id) {
            port.appuyer(broche);
        }
    }

    /// Relache une broche, qui remonte par sa resistance de tirage.
    pub fn relacher(&mut self, id: u32) {
        let broche = id & 0xF;
        if let Some(port) = self.port_de(id) {
            port.relacher(broche);
        }
    }

    pub fn load_firmware_file<P: AsRef<Path>>(&mut self, path: P) -> Result<LoadReport, String> {
        let p = path.as_ref();
        let report = FirmwareLoader::load_flash_dump(&mut self.bus, p, self.device_key)?;

        self.firmware_path = Some(p.to_string_lossy().to_string());
        // Le firmware peut relire la cle dans les fusibles, comme sur la puce.
        self.periph.fuses.device_key = self.device_key;
        self.bus.mmio_trace.clear();
        self.bus.mmio_trace.enabled = true;

        // L'image chargee sert de fond aux instantanes.
        self.bus.flash.figer_reference();
        // L'empreinte range les sauvegardes par dump : les cinq editions n'ont
        // ni les memes ressources ni la meme disposition, leurs parties ne se
        // melangent pas.
        self.empreinte = Some(sauvegarde::empreinte(p, &self.bus.flash.reference));
        self.edition = Edition::depuis_le_nom(p);
        self.sauvegarde_active = None;
        self.revision_ecrite = self.bus.flash.revision;

        self.reset();
        self.is_running = report.bootable;
        // L'afficheur n'est plus recopie depuis la SRAM : il recoit les trames
        // que le controleur de transferts lui pousse, comme sur la console.
        self.last_report = Some(report.clone());
        Ok(report)
    }

    pub fn get_disassembly_window(&mut self, count: usize) -> Vec<DisassembledInst> {
        self.get_disassembly_at(self.cpu.regs.pc, count)
    }

    pub fn get_disassembly_at(&mut self, start_addr: u32, count: usize) -> Vec<DisassembledInst> {
        let mut list = Vec::new();
        let mut cur_pc = start_addr;

        for _ in 0..count {
            let w1 = self.bus.read_u16(cur_pc, &mut self.periph, &self.cpu.nvic);
            let w2 = self.bus.read_u16(cur_pc + 2, &mut self.periph, &self.cpu.nvic);
            let inst = Disassembler::disassemble(cur_pc, &[w1, w2]);
            let advance = if inst.is_32bit { 4 } else { 2 };
            list.push(inst);
            cur_pc = cur_pc.wrapping_add(advance);
        }

        list
    }
}
