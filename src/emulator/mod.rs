pub mod aes;
pub mod cpu;
pub mod edition;
pub mod etat;
pub mod scribe;
pub mod loader;
pub mod reprises;
pub mod mmu;
pub mod sauvegarde;
pub mod peripherals;
pub mod scenes;
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

/// Ce que la console faisait au moment ou elle a refuse un paquet.
#[derive(Debug, Clone, Default)]
pub struct TraceRefus {
    pub pc: u32,
    pub lr: u32,
    pub sp: u32,
    pub registres: [u32; 8],
    /// Adresses de retour relevees sur la pile, qui donnent la chaine d'appels.
    pub retours: Vec<u32>,
    /// Appels traverses juste avant le refus, dans l'ordre. Contrairement a la
    /// pile, qui garde des valeurs perimees des cadres precedents, ceci est le
    /// chemin reellement parcouru.
    pub chemin: Vec<u32>,
}

/// Where a watched flag lives.
///
/// Some are at a known address; others are reached through a pointer the
/// firmware keeps, and move with it.
#[derive(Clone, Debug)]
pub enum Lieu {
    Fixe(u32),
    Indirect { pointeur: u32, decalage: u32 },
}

pub struct Machine {
    pub cpu: Cpu,
    pub bus: MemoryBus,
    pub periph: Peripherals,
    pub breakpoints: HashSet<u32>,
    pub is_running: bool,
    pub instructions_per_frame: u32,
    /// Ceiling on console time for one `run_frame` call, in cycles.
    ///
    /// Zero disables it, and that is the default: the probes and the tests rely
    /// on `instructions_per_frame` alone, and a call returning early would make
    /// them execute far less than they asked for.
    ///
    /// The interface does set it. Now that the core can advance the clock
    /// without interpreting, twenty thousand instructions spent in a wait are
    /// worth four thousand skips, a third of a second of console time in one
    /// go: real-time accounting and melody sampling then happened in enormous
    /// blocks, and it was audible.
    pub plafond_cycles: u64,
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
    /// Contexte fige au premier refus emis sur la liaison serie. C'est la seule
    /// facon de savoir d'ou le firmware decide ce refus : l'echange n'a lieu
    /// qu'avec un outil exterieur, et toute sonde qui le ralentit l'empeche.
    pub trace_refus: Option<TraceRefus>,
    /// Vrai uniquement pendant l'affichage de l'onglet UART. Le lien serie ne
    /// depend pas de ce drapeau, seulement son instrumentation couteuse.
    diagnostic_uart_actif: bool,
    /// Anneau des derniers appels traverses. Il n'est rempli que lorsque la
    /// liaison serie est ouverte : hors de ce cas il ne servirait a rien et
    /// couterait une ecriture par instruction.
    anneau_appels: Vec<u32>,
    curseur_anneau: usize,
    /// Registre de lien du pas precedent, pour reconnaitre un appel.
    dernier_lr: u32,
    /// Empreinte du dump charge, qui range ses sauvegardes a part.
    pub empreinte: Option<String>,
    /// Edition reconnue, qui donne la couleur de la coque.
    pub edition: Edition,
    /// Fichier de sauvegarde suivi. Sans lui, la partie ne vit que le temps de
    /// la session, comme avant.
    pub sauvegarde_active: Option<std::path::PathBuf>,
    /// Revision de flash deja recopiee sur le disque.
    pub revision_ecrite: u64,
    /// Adresses des entrees du tableau des voix audio, reperees en memoire.
    ///
    /// Le tableau n'est pas au meme endroit d'une edition a l'autre :
    /// 0x1801C820 sur Water, huit octets plus loin sur Jade Forest, sortie plus
    /// tard. Le chercher plutot que le coder en dur evite de le relever a
    /// chaque edition, et c'est ce qui rendait Jade Forest muette.
    ///
    /// On garde les adresses exactes et non une base avec un pas : une base
    /// suppose de savoir quelle entree on a trouvee, ce qu'on ignore, et
    /// balayer a l'aveugle autour d'elle manquait le tableau une fois sur deux.
    pub voix: Vec<u32>,
    /// Does time pass for the console while the emulator is closed?
    ///
    /// True by default, like the real device: the save carries a timestamp, and
    /// the gap is added to the seconds counter on reopening. False makes the
    /// counter resume exactly where it stopped. Read by `Sauvegarde::appliquer`,
    /// so it must be set before `ouvrir_sauvegarde`.
    pub temps_hors_ligne: bool,
    /// Forbids the console from staying in deep sleep.
    ///
    /// The firmware falls asleep on its own after a few minutes without a
    /// press: it programs the power manager, parks in a loop in PRAM and waits
    /// for hardware. That is the real device's behaviour, and the screen goes
    /// dark.
    ///
    /// With this flag the emulator brings the console back as soon as it
    /// sleeps. When the idle counter below is known, nothing is woken at all:
    /// the firmware never decides to sleep in the first place. Otherwise a
    /// fallback applies, described at `sortir_de_veille`.
    pub veille_interdite: bool,
    /// Addresses of idle counters to clear, with their width in bytes.
    ///
    /// A counter of this shape rises while nothing happens and is reset by a
    /// press. Clearing it now and then means the firmware never sees the count
    /// reach its threshold.
    ///
    /// Nine candidates matching that signature were tried on the water edition
    /// and none of them governed the shutdown, so nothing is set by default.
    /// A matching signature is not proof.
    pub compteur_inactivite: Vec<(u32, u8)>,
    /// Phases of a synthetic wheel movement still to play, and the console
    /// cycle at which the next one is due.
    ///
    /// Eleven candidate addresses were tried on the water edition — counters
    /// rising while idle, counters cleared on a press, timestamps of the last
    /// press — and none of them governed the shutdown. Rather than keep
    /// guessing where the firmware keeps its idea of idleness, this feeds it
    /// what it already understands: a real turn of the wheel.
    ///
    /// One detent forward and one back. The net effect on the game is nothing —
    /// a list scrolls down a line and back up — but as far as the firmware is
    /// concerned somebody touched the console, and whatever it uses to decide
    /// it has been left alone is reset by its own code.
    ///
    /// Addresses of activity timestamps to refresh, with their width in bytes.
    ///
    /// The other way of writing the same idea, and the likelier one: rather
    /// than counting up, the firmware records the second at which the last
    /// press happened and compares `now - then` against a threshold. Such a
    /// word does not move while idle and jumps to the current time on a press.
    ///
    /// Writing the seconds counter into it once a second means no time ever
    /// appears to have passed since the last press. It is the firmware's own
    /// mechanism, so nothing is forced and no scene is short-circuited.
    ///
    /// Both shapes are supported.
    pub horodatage_activite: Vec<(u32, u8)>,
    /// Flag bits to hold, as a place, a width, bits to set and bits to clear.
    ///
    /// The third shape, and the one that governs the water edition. Following
    /// the calls out of the shutdown routine gave a chain of four gates, each
    /// reading a bit and passing the decision inward:
    ///
    /// ```text
    /// 0x10054CD2   [[0x18014038] + 20] bit 0 set    -> start winding down
    /// 0x10030C98   a byte at [r7 + 29]              -> keep going
    /// 0x1000FEAA   [0x18000BA0] bit 0 clear         -> first call
    /// 0x1000FEC2   [0x18000BA0] bit 7 set           -> second call
    /// ```
    ///
    /// Holding one of the inner ones is not enough: the firmware simply falls
    /// through to the next. The outermost, at `0x10054CD2`, is the one worth
    /// holding, and it is also the honest place to intervene — the inner ones
    /// have already decided and are only choosing a route.
    ///
    /// Such a flag is invisible to a search for values that rise or that jump
    /// to the clock, which is why three rounds of probing missed it.
    pub drapeau_activite: Vec<(Lieu, u8, u32, u32)>,
    /// Cycle of the last wake, to avoid retrying too often.
    veille_dernier: u64,
    /// Cycle of the last idle-counter clear.
    derniere_relance: u64,
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
            plafond_cycles: 0,
            console: String::new(),
            firmware_path: None,
            device_key: None,
            last_report: None,
            last_stop: None,
            empreinte: None,
            edition: Edition::default(),
            sauvegarde_active: None,
            revision_ecrite: 0,
            voix: Vec::new(),
            temps_hors_ligne: true,
            veille_interdite: false,
            compteur_inactivite: Vec::new(),
            horodatage_activite: Vec::new(),
            drapeau_activite: Vec::new(),
            veille_dernier: 0,
            derniere_relance: 0,
            trace_refus: None,
            diagnostic_uart_actif: false,
            anneau_appels: vec![0; 256],
            curseur_anneau: 0,
            dernier_lr: 0,
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
        } else {
            // Emplacement neuf : la flash doit repartir de l'image du dump.
            // Sans cela la partie neuve heritait de ce que la precedente avait
            // ecrit, et l'ecran ne changeait pas.
            self.bus.flash.revenir_a_la_reference();
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

    /// Prepares the save without touching the disk.
    ///
    /// Encoding reads the machine and must happen here; the system call can
    /// wait on another thread. It is the only way not to stop emulation once a
    /// second on a machine whose antivirus inspects every file that goes by.
    pub fn sauvegarde_a_confier(&mut self) -> Option<(std::path::PathBuf, Vec<u8>)> {
        let chemin = self.sauvegarde_active.clone()?;
        let octets = sauvegarde::Sauvegarde::depuis(self).encoder();
        self.revision_ecrite = self.bus.flash.revision;
        Some((chemin, octets))
    }

    /// Attache l'etat courant a un nouvel emplacement sans recharger la flash
    /// ni redemarrer le firmware.
    pub fn creer_sauvegarde_depuis_etat(
        &mut self,
        chemin: std::path::PathBuf,
    ) -> Result<(), String> {
        let precedente = self.sauvegarde_active.replace(chemin);
        if let Err(e) = self.ecrire_sauvegarde() {
            self.sauvegarde_active = precedente;
            return Err(e);
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus, &mut self.periph);
        // Le coeur repart, ses peripheriques aussi. L'UART surtout : ses files
        // materielles sont videes par un reset sur la console, et les y laisser
        // bloquait le message de demarrage derriere des octets d'avant.
        self.periph.uart.reinitialiser();
        self.last_stop = None;
    }

    pub fn step(&mut self) -> StepResult {
        // Meme raison que dans run_frame : le drapeau est teste ici, l'appel
        // n'a lieu que le jour ou il y a vraiment un reveil a appliquer.
        if self.periph.snsys.reveil_demande && self.reveil_materiel() {
            return StepResult::Ok(1);
        }
        if self.entretenir_la_veille() {
            return StepResult::Ok(1);
        }
        self.cpu.step(&mut self.bus, &mut self.periph)
    }

    /// Active ou coupe les sondes UART sans toucher au transport.
    pub fn regler_diagnostic_uart(&mut self, actif: bool) {
        if actif && !self.diagnostic_uart_actif {
            self.trace_refus = None;
            self.anneau_appels.fill(0);
            self.curseur_anneau = 0;
            self.dernier_lr = self.cpu.regs.lr;
        }
        self.diagnostic_uart_actif = actif;
        self.periph.uart.diagnostic_actif = actif;
        if !actif {
            self.periph.uart.refus_emis = false;
        }
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

    /// Releve ce que la console faisait au moment ou elle a refuse un paquet.
    fn relever_le_refus(&mut self) -> TraceRefus {
        let mut registres = [0u32; 8];
        for (i, r) in registres.iter_mut().enumerate() {
            *r = self.cpu.regs.get_reg(i as u8);
        }
        let sp = self.cpu.regs.get_sp();
        let mut retours = Vec::new();
        for k in 0..48u32 {
            let v = self.bus.read_u32(sp + k * 4, &mut self.periph, &self.cpu.nvic);
            // Une adresse de retour est impaire, le bit de pouce etant pose, et
            // tombe dans la memoire de programme ou dans la fenetre XIP.
            let cible = v & !1;
            if v & 1 == 1
                && ((0x100..0x10000).contains(&cible)
                    || (0x1000_0000..0x1010_0000).contains(&cible))
            {
                retours.push(cible);
            }
        }
        retours.truncate(10);
        // L'anneau est relu dans l'ordre chronologique, du plus ancien au plus
        // recent, en sautant les cases jamais ecrites.
        let n = self.anneau_appels.len();
        let mut chemin: Vec<u32> = Vec::with_capacity(n);
        for k in 0..n {
            let v = self.anneau_appels[(self.curseur_anneau + k) % n];
            if v != 0 && chemin.last() != Some(&v) {
                chemin.push(v);
            }
        }
        if chemin.len() > 40 {
            chemin = chemin.split_off(chemin.len() - 40);
        }
        TraceRefus {
            chemin,
            pc: self.cpu.regs.pc,
            lr: self.cpu.regs.lr,
            sp,
            registres,
            retours,
        }
    }

    pub fn run_frame(&mut self) -> StepResult {
        if !self.is_running {
            return StepResult::Halt;
        }

        if self.entretenir_la_veille() {
            return StepResult::Ok(1);
        }

        // Sans point d'arret pose, il n'y a rien a chercher. La table est une
        // table de hachage : l'interroger a chaque instruction coutait un
        // hachage complet par pas, soit plus cher que le decodage lui meme, et
        // pour rien la quasi totalite du temps. La sonde de vitesse passait a
        // cote, elle appelle step sans passer par ici.
        let poses = !self.breakpoints.is_empty();

        let par_trame = self.instructions_per_frame;
        let plafond = self.plafond_cycles;
        let depart_cycles = self.cpu.cycles;
        let mut executed = 0;
        while executed < par_trame {
            if plafond != 0 && self.cpu.cycles.wrapping_sub(depart_cycles) >= plafond {
                return StepResult::Ok(executed);
            }
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
            let lien_ouvert =
                self.diagnostic_uart_actif && self.periph.uart.ctrl & 0x41 == 0x41;
            match self.cpu.step(&mut self.bus, &mut self.periph) {
                StepResult::Ok(_) => {
                    executed += 1;
                    // Seuls les appels sont retenus, reconnus au registre de
                    // lien qui vient de changer. Garder toutes les ruptures de
                    // flot remplissait l'anneau avec la seule boucle d'emission,
                    // qui saute des milliers de fois sur place.
                    if lien_ouvert && self.trace_refus.is_none() {
                        let lr = self.cpu.regs.lr;
                        if lr != self.dernier_lr {
                            self.dernier_lr = lr;
                            let entree = self.cpu.regs.pc;
                            // Les fonctions d'aide du formatage vivent tout en
                            // bas de la memoire de programme et sont appelees
                            // une fois par caractere. Les retenir saturait
                            // l'anneau et masquait ce qui precede.
                            if entree >= 0x2000 {
                                let n = self.anneau_appels.len();
                                self.anneau_appels[self.curseur_anneau] = entree;
                                self.curseur_anneau = (self.curseur_anneau + 1) % n;
                            }
                        }
                    }
                    // Le contexte d'un refus se fige ici, au pas suivant celui
                    // qui l'a emis. Attendre la fin de la trame le perdrait :
                    // vingt mille instructions plus loin, la pile a change.
                    if self.periph.uart.refus_emis {
                        self.periph.uart.refus_emis = false;
                        if self.trace_refus.is_none() {
                            self.trace_refus = Some(self.relever_le_refus());
                        }
                    }
                }
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
    ///
    /// Adresse relevee sur Water. Elle ne vaut pas pour toutes les editions :
    /// voir `decalage_son`, qui la corrige.
    pub const SON_EN_COURS: u32 = 0x1801_4284;

    /// Deplacement du bloc du moteur audio par rapport a l'adresse relevee sur
    /// Water.
    ///
    /// Jade Forest range ce bloc huit octets plus loin. Mesure faite en lisant
    /// la memoire sur deux etats muets, un menu et l'ecran d'accueil :
    ///
    /// ```text
    ///   0x18014280  00 00 00 00 | 70 8a 00 00 | 01 01 01 00
    ///                80 81 82 83   84 85 86 87   88 89 8a 8b
    /// ```
    ///
    /// `0x18014284` y vaut `0x70` puis `0x03` alors que la console est muette,
    /// et change de valeur : ce n'est pas un booleen, c'est autre chose. Huit
    /// octets plus loin tout retombe juste, `0x1801428A` a 1 pour le moteur
    /// pret et `0x1801428C` a 0 pour le silence.
    ///
    /// Sans cette correction le modele lisait une adresse qui n'est pas le
    /// drapeau, la trouvait non nulle en permanence, et tenait une voix pour
    /// active sans arret : c'est le bourdonnement de quarante secondes en tete
    /// des sons de Jade Forest.
    ///
    /// L'edition se reconnait au nom du fichier, comme la coque : rien dans
    /// l'image ne la nomme. Une edition inconnue garde l'adresse de Water.
    pub fn decalage_son(edition: Edition) -> u32 {
        match edition {
            Edition::JadeForest => 8,
            _ => 0,
        }
    }

    /// L'adresse du drapeau pour l'edition chargee.
    pub fn adresse_son_en_cours(&self) -> u32 {
        Self::SON_EN_COURS + Self::decalage_son(self.edition)
    }
    /// Tableau des voix du moteur audio, huit entrees de 0x34 octets.
    ///
    /// L'allocation, en `0x10022BE2`, indexe ce tableau par le type de son.
    /// Une voix porte l'horloge du coeur en tete, son compte de rechargement
    /// en `+4`, un temoin d'activite en `+8` et son volume en `+0xC`. Le compte
    /// n'est pas une frequence : voir `BASE_DE_TEMPS_AUDIO`.
    /// Base relevee sur Water. Elle sert de repere et de repli ; la base reelle
    /// est cherchee en memoire, voir `localiser_les_voix`.
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

    /// Horloge du coeur, en tete de chaque entree de voix. C'est elle qui
    /// signe le tableau.
    const HORLOGE_VOIX: u32 = 0x05B8_D800;

    /// Vrai quand le firmware est en train de jouer un son.
    pub fn son_en_cours(&self) -> bool {
        self.lire_sram_u8(self.adresse_son_en_cours()) != 0
    }

    /// Cherche le tableau des voix en memoire vive et retient ses adresses.
    ///
    /// Une entree porte l'horloge du coeur en tete. On releve toutes les
    /// adresses qui la portent, puis on garde le plus gros groupe aligne sur le
    /// pas d'une entree : une valeur isolee qui vaut l'horloge par hasard ne
    /// fait pas un tableau. A appeler pendant qu'un son joue, et a chaque
    /// nouveau son : au silence le tableau peut n'avoir jamais ete rempli, et
    /// le firmware peut le reallouer ailleurs entre deux sons.
    pub fn localiser_les_voix(&mut self) {
        let d = &self.bus.sram.data;
        let mut candidats: Vec<u32> = Vec::new();
        let mut o = 0usize;
        while o + 16 <= d.len() {
            if u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) == Self::HORLOGE_VOIX {
                candidats.push(0x1800_0000 + o as u32);
            }
            o += 4;
        }
        if candidats.is_empty() {
            return;
        }
        // The largest group is counted first, then gathered once. The previous
        // version built a vector per candidate only to keep one: as many
        // allocations as addresses found, in a function already called at the
        // start of every sound.
        let mut tete = candidats[0];
        let mut taille = 0usize;
        for &a in &candidats {
            let n = candidats
                .iter()
                .filter(|&&b| (b.max(a) - b.min(a)) % Self::TAILLE_VOIX == 0)
                .count();
            if n > taille {
                taille = n;
                tete = a;
            }
        }
        self.voix = candidats
            .into_iter()
            .filter(|&b| (b.max(tete) - b.min(tete)) % Self::TAILLE_VOIX == 0)
            .collect();
    }

    /// True while the retained addresses still carry the clock value.
    ///
    /// A few reads against a sweep of all of RAM: this is what allows the table
    /// to be relocated only when it has actually moved, instead of redoing it
    /// at the start of every sound.
    pub fn voix_encore_valides(&self) -> bool {
        self.voix
            .iter()
            .any(|&base| self.lire_sram_u32(base) == Self::HORLOGE_VOIX)
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
        for &base in &self.voix {
            // L'entree doit toujours porter l'horloge : le firmware reutilise
            // sa memoire, et une adresse reperee peut avoir change de nature.
            if self.lire_sram_u32(base) != Self::HORLOGE_VOIX {
                continue;
            }
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
        self.voix
            .iter()
            .filter_map(|&base| {
                if self.lire_sram_u32(base) != Self::HORLOGE_VOIX {
                    return None;
                }
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

    /// The firmware's scene machine, in RAM.
    ///
    /// `SCENE` holds the current scene, `PRECEDENTE` the one we came from, and
    /// the low three bits of `PHASE` say where the cycle stands: 0 entering,
    /// 1 running, 2 leaving. Writing a scene and clearing those bits amounts to
    /// asking to enter it on the next pass.
    pub const SCENE: u32 = 0x1800_1BF4;
    pub const SCENE_PRECEDENTE: u32 = 0x1800_1BF8;
    pub const SCENE_PHASE: u32 = 0x1800_1BFA;

    fn lire_sram_u16(&self, adresse: u32) -> u16 {
        let o = (adresse - 0x1800_0000) as usize;
        let d = &self.bus.sram.data;
        if o + 2 > d.len() {
            return 0;
        }
        u16::from_le_bytes([d[o], d[o + 1]])
    }

    fn ecrire_sram_u16(&mut self, adresse: u32, valeur: u16) {
        let o = (adresse - 0x1800_0000) as usize;
        let d = &mut self.bus.sram.data;
        if o + 2 <= d.len() {
            d[o..o + 2].copy_from_slice(&valeur.to_le_bytes());
        }
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

    /// Keeps the console from shutting down, when it has been asked to.
    ///
    /// Called from both ways of advancing the machine. The maintenance used to
    /// live in `run_frame` alone, so anything driven one instruction at a time
    /// — the probes, and stepping from the keyboard — got the fallback without
    /// the protection that exists to make the fallback unnecessary, and
    /// reported that the setting did nothing.
    ///
    /// Rend vrai quand un reveil vient d'etre applique et que l'appelant doit
    /// rendre la main sans interpreter.
    fn entretenir_la_veille(&mut self) -> bool {
        if !self.veille_interdite {
            return false;
        }
        // Once a second of console time is ample: the firmware's delay is
        // measured in minutes. With an address known the console simply never
        // sleeps and the fallback below is never reached.
        const RYTHME: u64 = peripherals::snsys::CYCLES_PAR_SECONDE as u64;
        // The comparison is two-sided. A wake by reset puts the cycle counter
        // back to zero while this marker keeps a large value: subtracting
        // saturates at zero for ever after and the refresh never runs again. A
        // counter that has gone backwards means the clock restarted, so start
        // over.
        let ecoule = self.cpu.cycles.wrapping_sub(self.derniere_relance);
        if self.cpu.cycles < self.derniere_relance || ecoule >= RYTHME {
            self.derniere_relance = self.cpu.cycles;
            self.tenir_eveille();
        }
        // Fallback, for images whose address is unknown: try to come back to
        // the previous scene; failing that, wake as a button press would, which
        // is a reset and returns to the clock screen. It only delays the
        // shutdown, so it is throttled.
        self.sortir_de_veille()
    }

    /// Keeps the firmware from concluding that nothing has happened.
    ///
    /// Counters go to zero, timestamps go to the current second, flag bits are
    /// held: either way the firmware sees no elapsed idle time. Exactly what a
    /// button press would do, without the press.
    fn tenir_eveille(&mut self) {
        for (adresse, largeur) in self.compteur_inactivite.clone() {
            self.ecrire_sram_petit_boutiste(adresse, largeur, 0);
        }
        let maintenant = self.periph.snsys.secondes;
        for (adresse, largeur) in self.horodatage_activite.clone() {
            self.ecrire_sram_petit_boutiste(adresse, largeur, maintenant);
        }
        // The flags are handled twice over. The value in RAM is corrected, so
        // that anything reading it by other means agrees; and the bus is told to
        // force the same bits as the core reads them, which is what actually
        // settles it. Writing alone lost the race: the firmware sets its flag
        // and reads it back a few instructions later, long before the next
        // refresh comes round.
        self.bus.masques_lecture.clear();
        for (lieu, largeur, poser, effacer) in self.drapeau_activite.clone() {
            let Some(adresse) = self.resoudre_lieu(&lieu) else {
                continue;
            };
            let actuel = self.lire_sram_petit_boutiste(adresse, largeur);
            let voulu = (actuel | poser) & !effacer;
            if voulu != actuel {
                self.ecrire_sram_petit_boutiste(adresse, largeur, voulu);
            }
            for k in 0..(largeur as u32).min(4) {
                let p = ((poser >> (k * 8)) & 0xFF) as u8;
                let e = ((effacer >> (k * 8)) & 0xFF) as u8;
                if p != 0 || e != 0 {
                    self.bus.masques_lecture.push((adresse.wrapping_add(k), p, e));
                }
            }
        }
    }

    /// Turns a place into the address it names right now.
    ///
    /// The outermost gate does not live at a fixed address: the firmware keeps
    /// a pointer at `0x18014038` and the byte of interest sits twenty bytes
    /// into whatever it points at. That pointer moves, so it has to be followed
    /// each time rather than resolved once.
    pub fn resoudre_lieu(&self, lieu: &Lieu) -> Option<u32> {
        match *lieu {
            Lieu::Fixe(a) => Some(a),
            Lieu::Indirect { pointeur, decalage } => {
                let base = self.lire_sram_petit_boutiste(pointeur, 4);
                // A pointer the firmware has not filled in yet, or one that
                // leads outside RAM, names nothing worth writing to.
                if !(0x1800_0000..0x1802_0000).contains(&base) {
                    return None;
                }
                Some(base.wrapping_add(decalage))
            }
        }
    }

    /// A word of RAM, for tools that need to follow a pointer.
    pub fn lire_mot_sram(&self, adresse: u32) -> u32 {
        self.lire_sram_petit_boutiste(adresse, 4)
    }

    /// A byte of RAM, likewise.
    pub fn lire_octet_sram(&self, adresse: u32) -> u8 {
        self.lire_sram_petit_boutiste(adresse, 1) as u8
    }

    /// Reads `largeur` bytes of RAM little-endian, zero outside it.
    fn lire_sram_petit_boutiste(&self, adresse: u32, largeur: u8) -> u32 {
        let o = (adresse.wrapping_sub(0x1800_0000)) as usize;
        let n = (largeur as usize).min(4);
        let d = &self.bus.sram.data;
        if o + n > d.len() {
            return 0;
        }
        let mut octets = [0u8; 4];
        octets[..n].copy_from_slice(&d[o..o + n]);
        u32::from_le_bytes(octets)
    }

    /// Writes `valeur` little-endian over `largeur` bytes of RAM, ignoring
    /// anything that falls outside it.
    fn ecrire_sram_petit_boutiste(&mut self, adresse: u32, largeur: u8, valeur: u32) {
        let o = (adresse.wrapping_sub(0x1800_0000)) as usize;
        let n = (largeur as usize).min(4);
        let d = &mut self.bus.sram.data;
        if o + n <= d.len() {
            d[o..o + n].copy_from_slice(&valeur.to_le_bytes()[..n]);
        }
    }

    /// Brings the console out of sleep, by returning to the caller if possible.
    fn sortir_de_veille(&mut self) -> bool {
        if !self.en_veille_profonde() {
            return false;
        }
        // A tenth of a second of console time between attempts. If the trick
        // does not take, the firmware goes straight back to sleep: without this
        // brake we would retry thousands of times a second, which cost half the
        // speed for nothing.
        const REPOS: u64 = peripherals::snsys::CYCLES_PAR_SECONDE as u64 / 10;
        // Same two-sided test as the idle clear: a wake by reset restarts the
        // cycle counter, and a one-sided comparison would then lock this out.
        if self.veille_dernier != 0
            && self.cpu.cycles >= self.veille_dernier
            && self.cpu.cycles - self.veille_dernier < REPOS
        {
            return false;
        }
        self.veille_dernier = self.cpu.cycles;
        if self.ecourter_la_veille() {
            return true;
        }
        if self.reveiller_par_broche() {
            return true;
        }
        false
    }

    /// Vrai quand le coeur est gare dans cette boucle.
    pub fn en_veille_profonde(&self) -> bool {
        self.periph.pmu.deep_sleep_active || Self::VEILLE_PROFONDE.contains(&self.cpu.regs.pc)
    }

    /// Brings the console back from sleep without resetting it.
    ///
    /// The hardware wake is a reset: RAM survives, but the firmware restarts
    /// from its vector and lands on the clock screen. For anyone who only wants
    /// the console to stay lit on the current scene, that misses the point.
    ///
    /// The firmware reached its wait loop through a call, so the link register
    /// holds the sleep routine's return address. Sending the core there amounts
    /// to making that routine an empty function: the caller carries on where it
    /// was, on the same scene, with nothing reinitialised.
    ///
    /// Returns false if the link does not look like a code address — the loop
    /// may have overwritten it with a call of its own. The caller then falls
    /// back on the hardware wake, poorer but safe.
    pub fn ecourter_la_veille(&mut self) -> bool {
        // The shortcut only means anything if the core is parked in the loop:
        // that is the one situation where the link is the return we want.
        if !Self::VEILLE_PROFONDE.contains(&self.cpu.regs.pc) {
            return false;
        }
        let lien = self.cpu.regs.lr;
        let cible = lien & !1;
        let en_code = cible <= mmu::map::PRAM_END
            || (mmu::map::ICACHE_BASE..=mmu::map::ICACHE_END).contains(&cible);
        // The Thumb bit must be set: the core knows no other instruction set,
        // and an even value is not a return address.
        if (lien & 1) == 0 || !en_code {
            return false;
        }
        self.periph.pmu.declencher_reveil_broche();
        self.periph.snsys.reveil_demande = false;
        self.cpu.regs.pc = cible;
        self.rappeler_la_scene();
        true
    }

    /// Re-requests the scene from before the console fell asleep.
    ///
    /// Returning to the caller is not enough: the scene machine has entered its
    /// power-down scene, and that scene's loop handler calls the sleep routine
    /// again on every pass. The core therefore came out of sleep only to go
    /// straight back in — five thousand times in a few minutes, at half speed,
    /// the screen still dark.
    ///
    /// So the previous scene is written back and the phase reset to entry,
    /// which amounts to asking to enter it on the next pass. The scene left
    /// behind is not unwound cleanly — whatever it took on the heap stays there
    /// — but the power-down scene takes next to nothing, and this is the only
    /// lever available without knowing the firmware's idle counter.
    fn rappeler_la_scene(&mut self) {
        let precedente = self.lire_sram_u16(Self::SCENE_PRECEDENTE);
        let courante = self.lire_sram_u16(Self::SCENE);
        // A missing or identical predecessor leads nowhere.
        if precedente == 0xFFFF || precedente == courante {
            return;
        }
        self.ecrire_sram_u16(Self::SCENE, precedente);
        let phase = self.lire_sram_u16(Self::SCENE_PHASE);
        self.ecrire_sram_u16(Self::SCENE_PHASE, phase & !0x0007);
    }

    /// Reveille le coeur par une entree utilisateur et indique si un reveil a
    /// effectivement ete applique.
    pub fn reveiller_par_broche(&mut self) -> bool {
        if !self.en_veille_profonde() {
            return false;
        }
        self.periph.snsys.declencher_reveil();
        self.periph.pmu.declencher_reveil_broche();
        // Le controleur d'alimentation vient de quitter l'etat profond. Il ne
        // faut donc pas repasser par reveil_materiel, dont le test de sommeil
        // serait maintenant faux sur les editions garees hors de la boucle
        // historique.
        self.periph.snsys.reveil_demande = false;
        self.reset();
        self.is_running = true;
        true
    }

    /// Vrai quand la broche est tiree bas, donc le bouton enfonce.
    ///
    /// C'est ce que l'habillage regarde pour animer un bouton : peu importe
    /// que l'appui vienne du clavier, de la souris, de l'ecran ou du
    /// navigateur, la broche dit la verite.
    pub fn broche_basse(&self, id: u32) -> bool {
        let masque = 1u32 << (id & 0xF);
        let port = match id >> 4 {
            0 => &self.periph.port0,
            1 => &self.periph.port1,
            2 => &self.periph.port2,
            _ => return false,
        };
        port.entrees & masque == 0
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
        if self.reveiller_par_broche() {
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

        // Le tableau des voix est propre a une edition : celui de la
        // precedente ne veut plus rien dire ici.
        self.voix.clear();
        // Ne pas laisser l'image de l'edition precedente visible pendant le
        // demarrage de la nouvelle.
        self.periph.display = crate::emulator::peripherals::DisplayController::default();
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
