use eframe::egui;
use egui::{CentralPanel, Context, Key, SidePanel, TopBottomPanel};

use crate::audio::{AudioEngine, SoundEffect};
use crate::emulator::Machine;
use crate::gui::{ActiveModal, GuiWidgets, ShellColor};
use crate::hw_bridge::FlashInspector;
use crate::i18n::{I18n, Language};
use crate::ui::{ConsolePanel, CpuPanel, DisasmPanel, LcdPanel, MemoryPanel};

pub struct TamagotchiApp {
    pub machine: Machine,
    pub audio: AudioEngine,
    pub i18n: I18n,
    pub shell_color: ShellColor,
    pub show_debugger: bool,
    pub hex_base_addr: u32,
    pub last_frame_time: std::time::Instant,
    pub load_path_input: String,
    pub status_msg: Option<String>,
    pub flash_inspector: FlashInspector,
    pub active_modal: ActiveModal,
    pub disasm_view_addr: u32,
    /// Temps accorde a l'emulation a chaque image de l'interface.
    pub budget_ms: u64,
    /// Broches tenues basses tant que la commande dure. C'est ce qui porte
    /// l'appui long, celui qui ouvre le menu principal du jeu.
    pub maintenus: std::collections::HashSet<u32>,
    /// Broches tenues depuis le navigateur, qui annonce un debut et une fin
    /// plutot que de repeter son maintien a chaque image.
    pub tenus_distants: std::collections::HashSet<u32>,
    /// Broches en impulsion, avec le compte de pas ou elles remontent.
    ///
    /// La duree se compte en pas emules, pas en images : l'emulateur tourne a
    /// une fraction de la vitesse de la console, et un appui mesure en images
    /// durerait bien trop peu de temps a ses yeux.
    pub appuis: std::collections::HashMap<u32, u64>,
    /// Phases de l'encodeur restant a jouer, une par image.
    pub phases_encodeur: std::collections::VecDeque<(bool, bool)>,
    /// Texture de l'ecran, refaite seulement quand une trame arrive.
    pub ecran: Option<egui::TextureHandle>,
    /// Anneau d'instantanes automatiques, pour revenir avant un blocage.
    pub historique: crate::emulator::etat::Historique,
    /// Etat publie au serveur local et commandes qui en reviennent.
    pub partage: std::sync::Arc<std::sync::Mutex<crate::web::Partage>>,
    /// Port du serveur local, quand il a pu demarrer.
    pub port_web: Option<u16>,
    /// Debit atteint, en pas par seconde. C'est le seul chiffre qui dit si
    /// l'interface etouffe l'emulation.
    pub debit: f64,
    /// Point de depart de la mesure de debit en cours.
    pub debit_depart: (u64, std::time::Instant),
    /// Temps passe hors emulation a la derniere image, en millisecondes.
    ///
    /// C'est ce que l'interface prend a l'emulation. Sans ce chiffre, on ne
    /// peut que supposer ou passe le temps.
    pub cout_ui: f64,
    /// Emplacements de sauvegarde existants pour le dump charge.
    pub emplacements: Vec<String>,
    /// Emplacement suivi, vide quand la partie ne vit que le temps de la
    /// session.
    pub emplacement_choisi: String,
    /// Nom saisi pour creer un emplacement.
    pub nouvel_emplacement: String,
    /// Derniere recopie de la sauvegarde sur le disque. Le jeu ecrit sa flash
    /// souvent ; on espace les ecritures pour ne pas marteler le disque.
    pub derniere_ecriture: std::time::Instant,
    /// Angle de la molette, en degres cumules. Il ne sert qu'a animer les deux
    /// fleches de la fenetre transparente, et retombe doucement au repos.
    pub angle_molette: f32,
}

impl TamagotchiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let audio = AudioEngine::new();
        let i18n = I18n::default();
        let machine = Machine::new();

        // Le serveur local permet de suivre l'emulation depuis un navigateur,
        // et d'envoyer les memes commandes que la fenetre.
        let partage = std::sync::Arc::new(std::sync::Mutex::new(crate::web::Partage::default()));
        let port_web = crate::web::demarrer(std::sync::Arc::clone(&partage), 7340).ok();

        Self {
            machine,
            audio,
            i18n,
            shell_color: ShellColor::BlueWater,
            show_debugger: false,
            hex_base_addr: 0x6001_1000,
            last_frame_time: std::time::Instant::now(),
            load_path_input: String::new(),
            status_msg: Some("Tamagotchi Paradise Hardware Emulation Ready.".to_string()),
            flash_inspector: FlashInspector::new(),
            active_modal: ActiveModal::None,
            disasm_view_addr: 0x6001_1000,
            budget_ms: 40,
            maintenus: std::collections::HashSet::new(),
            tenus_distants: std::collections::HashSet::new(),
            appuis: std::collections::HashMap::new(),
            phases_encodeur: std::collections::VecDeque::new(),
            ecran: None,
            historique: crate::emulator::etat::Historique::default(),
            partage,
            port_web,
            debit: 0.0,
            debit_depart: (0, std::time::Instant::now()),
            cout_ui: 0.0,
            emplacements: Vec::new(),
            emplacement_choisi: String::new(),
            nouvel_emplacement: String::new(),
            derniere_ecriture: std::time::Instant::now(),
            angle_molette: 0.0,
        }
    }
}

impl TamagotchiApp {
    /// Charge un dump de flash et rend compte de ce qui s'est reellement passe.
    /// Relit la liste des emplacements de sauvegarde du dump charge.
    fn rafraichir_emplacements(&mut self) {
        self.emplacements = match &self.machine.empreinte {
            Some(e) => crate::emulator::sauvegarde::emplacements(e),
            None => Vec::new(),
        };
    }

    /// Suit un emplacement et y verse la partie qu'il contient.
    ///
    /// Un emplacement inconnu est accepte : c'est une partie neuve, qui
    /// s'ecrira des que le jeu sauvegardera.
    fn ouvrir_emplacement(&mut self, nom: String) {
        let Some(empreinte) = self.machine.empreinte.clone() else {
            return;
        };
        let chemin = crate::emulator::sauvegarde::chemin(&empreinte, &nom);
        match self.machine.ouvrir_sauvegarde(chemin) {
            Ok(true) => {
                // La flash porte maintenant la partie : le firmware doit la
                // relire depuis son demarrage, sinon il continue sur l'etat
                // vide qu'il avait deja en memoire. On ne remet en marche que ce
                // qui tournait deja : un dump non demarrable doit le rester.
                let tournait = self.machine.is_running;
                self.machine.reset();
                self.machine.is_running = tournait;
                self.historique.vider();
                self.status_msg = Some(format!("Partie {} chargee", nom));
            }
            Ok(false) => {
                self.status_msg = Some(format!("Nouvelle partie {}", nom));
            }
            Err(e) => {
                self.status_msg = Some(format!("Sauvegarde illisible : {}", e));
                return;
            }
        }
        self.emplacement_choisi = nom;
        self.rafraichir_emplacements();
    }

    /// Recopie la partie sur le disque quand le jeu a ecrit sa flash.
    ///
    /// Espacee d'une seconde : le firmware reecrit ses deux pages a chaque
    /// evenement, et il n'y a rien a gagner a suivre chaque octet.
    fn tenir_la_sauvegarde(&mut self) {
        if !self.machine.sauvegarde_a_ecrire() {
            return;
        }
        if self.derniere_ecriture.elapsed() < std::time::Duration::from_secs(1) {
            return;
        }
        self.derniere_ecriture = std::time::Instant::now();
        if let Err(e) = self.machine.ecrire_sauvegarde() {
            self.status_msg = Some(format!("Sauvegarde non ecrite : {}", e));
        }
    }

    fn load_firmware(&mut self, path: std::path::PathBuf) {
        self.load_path_input = path.to_string_lossy().to_string();
        self.machine.console.clear();
        self.appuis.clear();
        self.phases_encodeur.clear();
        self.maintenus.clear();
        self.tenus_distants.clear();
        self.historique.vider();
        match self.machine.load_firmware_file(&path) {
            Ok(report) => {
                // Les dumps Earth et Land ont ete extraits pile faible : sans
                // cela le firmware affiche son message et s'eteint aussitot.
                self.machine.remplacer_la_pile();
                let _ = self.flash_inspector.inspect_file(&path);
                // Un firmware demarrable s'execute depuis la PRAM mappee a 0,
                // sinon on laisse la vue sur le code XIP, lui toujours en clair.
                self.hex_base_addr = if report.bootable { 0 } else { 0x6001_1000 };
                self.disasm_view_addr = self.machine.cpu.regs.pc;
                self.status_msg = Some(self.describe_load(&report));
                // La console reprend sa partie toute seule, comme un vrai
                // Tamagotchi qu'on rallume. Sans cela il faudrait penser a
                // choisir un emplacement avant de jouer.
                self.shell_color = ShellColor::pour_edition(self.machine.edition);
                self.status_msg = Some(format!(
                    "{} charge, coque {}",
                    self.machine.edition.nom(),
                    self.shell_color.nom()
                ));
                self.rafraichir_emplacements();
                self.ouvrir_emplacement(
                    crate::emulator::sauvegarde::EMPLACEMENT_PAR_DEFAUT.to_string(),
                );
            }
            Err(e) => {
                self.status_msg = Some(self.i18n.t_args("emu_load_error", &[("error", &e)]));
            }
        }
    }

    fn describe_load(&self, r: &crate::emulator::LoadReport) -> String {
        let bytes = r.bytes.to_string();
        if r.bootable {
            self.i18n.t_args(
                "emu_load_bootable",
                &[
                    ("bytes", &bytes),
                    ("pc", &format!("0x{:08X}", r.entry_pc)),
                    ("sp", &format!("0x{:08X}", r.entry_sp)),
                ],
            )
        } else if r.encrypted {
            self.i18n.t("emu_load_need_key")
        } else {
            self.i18n.t_args("emu_load_not_bootable", &[("bytes", &bytes)])
        }
    }
}

impl TamagotchiApp {
    /// Duree d'une impulsion, en pas emules.
    ///
    /// Le SysTick est arme a 95999, soit une milliseconde a 96 MHz : cent
    /// millisecondes de temps console font environ dix millions de pas. C'est
    /// assez pour que le firmware voie l'appui, et assez court pour qu'il ne le
    /// prenne pas pour un appui long.
    const IMPULSION: u64 = 10_000_000;

    /// Marque une broche comme tenue basse pour cette image.
    fn maintenir(&mut self, broche: u32) {
        self.maintenus.insert(broche);
    }

    /// Cycles du coeur pour une seconde de temps console.
    ///
    /// Le SysTick est arme a 95999 pour une milliseconde, ce qui place le coeur
    /// a 96 MHz.
    const SECONDE_CONSOLE: u64 = 96_000_000;

    /// Declenche une impulsion breve sur une broche.
    fn presser(&mut self, broche: u32) {
        self.presser_duree(broche, Self::IMPULSION);
    }

    /// Tient une broche basse pendant une duree donnee, en pas emules.
    ///
    /// C'est ce qu'il faut pour un appui long reproductible : l'emulateur ne
    /// tournant pas a la vitesse de la console, tenir trois secondes a la main
    /// ne fait pas trois secondes a ses yeux.
    fn presser_duree(&mut self, broche: u32, duree: u64) {
        let fin = self.machine.cpu.cycles + duree;
        // Un appui deja en cours n'est jamais raccourci.
        let entree = self.appuis.entry(broche).or_insert(fin);
        *entree = (*entree).max(fin);
    }

    /// Programme un cran d'encodeur, en quadrature.
    ///
    /// Les deux voies sont hautes au repos. Un cran les fait passer par la
    /// sequence de Gray, dans un sens ou dans l'autre selon le signe. Une phase
    /// par image de l'interface suffit : le firmware echantillonne l'encodeur
    /// dans son interruption de base de temps.
    fn tourner_molette(&mut self, sens: i32) {
        const AVANT: [(bool, bool); 4] = [(false, true), (false, false), (true, false), (true, true)];
        let mut phases: Vec<(bool, bool)> = AVANT.to_vec();
        if sens < 0 {
            phases = phases.iter().map(|&(a, b)| (b, a)).collect();
        }
        for phase in phases {
            self.phases_encodeur.push_back(phase);
        }
    }

    /// Toutes les broches de commande de la console.
    const COMMANDES: [u32; 4] = [
        Machine::BOUTON_MOLETTE,
        Machine::BOUTON_A,
        Machine::BOUTON_C,
        Machine::BOUTON_B,
    ];

    /// Applique l'etat des entrees pour l'image en cours.
    ///
    /// Une broche est basse tant qu'elle est tenue, ou tant que son impulsion
    /// n'est pas ecoulee. Les deux se cumulent : relacher le pointeur pendant
    /// une impulsion ne coupe pas l'appui avant terme.
    fn appliquer_entrees(&mut self) {
        let maintenant = self.machine.cpu.cycles;
        self.appuis.retain(|_, fin| *fin > maintenant);
        for broche in Self::COMMANDES {
            let bas = self.maintenus.contains(&broche) || self.appuis.contains_key(&broche);
            if bas {
                self.machine.appuyer(broche);
            } else {
                self.machine.relacher(broche);
            }
        }
        // Le clavier est lu avant cet appel, la coque et le navigateur apres :
        // vider ici laisse les trois alimenter la tranche suivante, et un
        // bouton relache cesse bien de tenir sa broche.
        self.maintenus.clear();
        if let Some((voie1, voie2)) = self.phases_encodeur.pop_front() {
            if voie1 {
                self.machine.relacher(Machine::ENCODEUR_1);
            } else {
                self.machine.appuyer(Machine::ENCODEUR_1);
            }
            if voie2 {
                self.machine.relacher(Machine::ENCODEUR_2);
            } else {
                self.machine.appuyer(Machine::ENCODEUR_2);
            }
        }
    }

    /// Revient a l'instantane precedent.
    fn reculer(&mut self) {
        match self.historique.reculer() {
            Some(etat) => {
                let cycles = etat.cycles;
                self.machine.restaurer(&etat);
                self.appuis.clear();
                self.maintenus.clear();
                self.tenus_distants.clear();
                self.phases_encodeur.clear();
                self.debit_depart = (self.machine.cpu.cycles, std::time::Instant::now());
                self.status_msg = Some(format!("Retour a {} pas executes.", cycles));
            }
            None => {
                self.status_msg = Some("Aucun instantane a restaurer.".to_string());
            }
        }
    }

    /// Relit un instantane et remet la machine dedans.
    ///
    /// Un instantane ne porte que les pages de flash modifiees : il faut son
    /// firmware sous les pieds. On le recharge quand ce n'est pas deja celui en
    /// place, sans quoi la machine repartirait sur une flash vide.
    fn restaurer_fichier(&mut self, chemin: &std::path::Path) -> String {
        let etat = match crate::emulator::etat::Instantane::lire(chemin) {
            Ok(e) => e,
            Err(e) => return format!("Lecture impossible : {}", e),
        };
        if !etat.firmware.is_empty() && etat.firmware != self.load_path_input {
            self.load_firmware(std::path::PathBuf::from(etat.firmware.clone()));
        }
        if self.load_path_input.is_empty() {
            return "Charge d'abord le dump de flash correspondant.".to_string();
        }
        self.machine.restaurer(&etat);
        self.appuis.clear();
        self.maintenus.clear();
        self.tenus_distants.clear();
        self.phases_encodeur.clear();
        // Le compteur de pas vient de sauter : repartir de la remet le debit
        // d'accord avec la realite au lieu d'afficher un chiffre absurde.
        self.debit_depart = (self.machine.cpu.cycles, std::time::Instant::now());
        format!("Etat restaure, {} pas executes.", etat.cycles)
    }

    /// Publie l'image et le diagnostic pour le serveur local.
    fn publier(&mut self) {
        let rapport = self.diagnostic();
        let mut partage = self.partage.lock().unwrap();
        partage.ecran.clear();
        partage.ecran.extend_from_slice(&self.machine.periph.display.vram);
        partage.largeur = self.machine.periph.display.width;
        partage.hauteur = self.machine.periph.display.height;
        partage.trames = self.machine.periph.display.trames;
        partage.diagnostic = rapport;
    }

    /// Rapport d'etat copiable, pour signaler un blocage sans capture d'ecran.
    fn diagnostic(&self) -> String {
        let n = &self.machine.cpu.nvic;
        let mode = match self.machine.cpu.regs.mode {
            crate::emulator::cpu::registers::Mode::Thread => "Thread",
            _ => "Handler",
        };
        let etat = |a: u32| -> u32 {
            let o = (a - 0x1800_0000) as usize;
            let b = |i: usize| self.machine.bus.sram.read_u8(o + i) as u32;
            b(0) | (b(1) << 8)
        };
        let console: String = self.machine.console.chars().rev().take(600).collect::<Vec<_>>()
            .into_iter().rev().collect();
        format!(
            "== diagnostic emulateur Tamagotchi Paradise\n\
             firmware      {}\n\
             pas executes  {}   debit {:.1} millions par seconde\n\
             cout interface {:.1} ms par image\n\
             PC            {:#010x}   mode {}   PRIMASK {}\n\
             trames ecran  {}   instantanes {}\n\
             etat du jeu   courant {}   transition demandee {}\n\
             boutons       {}\n\
             IRQ 0..31     activees {:#010x}  en attente {:#010x}\n\
             IRQ 32..63    activees {:#010x}  en attente {:#010x}\n\
             dernier transfert vers l'ecran : {}\n\
             console du firmware (fin) :\n\
             {}\n",
            self.load_path_input,
            self.machine.cpu.cycles,
            self.debit / 1e6,
            self.cout_ui,
            self.machine.cpu.regs.pc,
            mode,
            self.machine.cpu.regs.primask,
            self.machine.periph.display.trames,
            self.historique.len(),
            etat(0x1800_1BF4),
            etat(0x1800_1BF6),
            {
                // Niveau reel de chaque broche de commande, avec sa direction.
                // Une entree se lit haute au repos ; si elle est declaree en
                // sortie, l'appui n'a plus aucun effet.
                let d = |id: u32| -> String {
                    let port = match id >> 4 {
                        0 => &self.machine.periph.port0,
                        1 => &self.machine.periph.port1,
                        _ => &self.machine.periph.port2,
                    };
                    let broche = id & 0xF;
                    let niveau = (port.read_reg(0) >> broche) & 1;
                    let sortie = (port.direction >> broche) & 1;
                    format!("{}{}", niveau, if sortie == 1 { "s" } else { "e" })
                };
                format!(
                    "molette {} A {} C {} B {} encodeur {} {}",
                    d(Machine::BOUTON_MOLETTE),
                    d(Machine::BOUTON_A),
                    d(Machine::BOUTON_C),
                    d(Machine::BOUTON_B),
                    d(Machine::ENCODEUR_1),
                    d(Machine::ENCODEUR_2)
                )
            },
            n.iser[0],
            n.ispr[0],
            n.iser[1],
            n.ispr[1],
            match self.machine.periph.dma.canaux.first() {
                Some(c) => format!(
                    "source {:#010x}  destination {:#010x}  unites {}",
                    c.source,
                    c.destination,
                    c.compte & crate::emulator::peripherals::dma::MASQUE_COMPTE
                ),
                None => "aucun".to_string(),
            },
            console.trim_end()
        )
    }
}

impl eframe::App for TamagotchiApp {
    /// Derniere recopie avant de fermer la fenetre.
    ///
    /// L'ecriture periodique est espacee d'une seconde : sans ce dernier
    /// passage, la derniere sauvegarde du jeu pourrait rester en memoire.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.machine.ecrire_sauvegarde();
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let now = std::time::Instant::now();
        let _dt = (now - self.last_frame_time).as_secs_f32().min(0.1);
        self.last_frame_time = now;

        // Auto repaint for 60 FPS emulator loop
        ctx.request_repaint();

        // 1. Entrees. Une touche tenue tient la broche basse aussi longtemps
        //    qu'elle reste enfoncee : c'est ce que le jeu attend pour son appui
        //    long, celui qui ouvre le menu principal.
        let key_f10 = ctx.input(|i| i.key_pressed(Key::F10));
        // Fleche haut tourne vers la droite, fleche bas vers la gauche, comme
        // la molette de la console.
        let molette = ctx.input(|i| {
            (i.key_pressed(Key::ArrowUp) as i32) - (i.key_pressed(Key::ArrowDown) as i32)
        });
        // Chaque touche tient sa broche tant qu'elle est enfoncee, et plusieurs
        // touches tenues ensemble donnent les combinaisons de la console :
        // molette maintenue plus B pour le menu special, A plus C pour la
        // remise a zero.
        let touches = [
            (Machine::BOUTON_A, [Key::A, Key::Q, Key::ArrowLeft]),
            (Machine::BOUTON_B, [Key::B, Key::Space, Key::Num2]),
            (Machine::BOUTON_C, [Key::C, Key::D, Key::ArrowRight]),
            (Machine::BOUTON_MOLETTE, [Key::Enter, Key::S, Key::Num0]),
        ];
        for (broche, keys) in touches {
            if ctx.input(|i| keys.iter().any(|k| i.key_down(*k))) {
                self.maintenir(broche);
            }
        }
        if molette != 0 {
            self.tourner_molette(molette);
        }

        if key_f10 {
            self.machine.is_running = false;
            self.machine.step();
        }

        // 2. Avance de l'emulation, bornee en temps pour que l'interface reste
        //    reactive. Le coeur tourne a environ dix-neuf millions de pas par
        //    seconde, soit un cinquieme de la console.
        // Commandes venues du navigateur : elles passent par les memes broches
        // que la fenetre.
        let recues: Vec<crate::web::Commande> = {
            let mut partage = self.partage.lock().unwrap();
            std::mem::take(&mut partage.commandes)
        };
        for commande in recues {
            match commande {
                crate::web::Commande::Presser(broche) => self.presser(broche),
                // Le navigateur ne peut pas repeter son maintien a chaque image :
                // il annonce le debut et la fin, et c'est nous qui tenons entre
                // les deux.
                crate::web::Commande::Tenir(broche, true) => {
                    self.tenus_distants.insert(broche);
                }
                crate::web::Commande::Tenir(broche, false) => {
                    self.tenus_distants.remove(&broche);
                }
                crate::web::Commande::Long(broche, secondes) => {
                    self.presser_duree(broche, Self::SECONDE_CONSOLE * secondes as u64);
                }
                crate::web::Commande::Tourner(sens) => self.tourner_molette(sens),
                crate::web::Commande::Reculer => self.reculer(),
                crate::web::Commande::Charger(chemin) => {
                    self.load_firmware(std::path::PathBuf::from(chemin));
                }
                crate::web::Commande::Vitesse(ms) => self.budget_ms = ms,
                crate::web::Commande::SauverEtat(chemin) => {
                    let etat = self.machine.instantane();
                    self.status_msg = Some(match etat.ecrire(std::path::Path::new(&chemin)) {
                        Ok(()) => format!("Etat ecrit dans {}", chemin),
                        Err(e) => format!("Ecriture impossible : {}", e),
                    });
                }
                crate::web::Commande::ChargerEtat(chemin) => {
                    self.status_msg =
                        Some(self.restaurer_fichier(std::path::Path::new(&chemin)));
                }
            }
        }
        for broche in self.tenus_distants.clone() {
            self.maintenir(broche);
        }

        self.appliquer_entrees();
        let debut_emulation = std::time::Instant::now();
        if self.machine.is_running && self.budget_ms > 0 {
            let debut = std::time::Instant::now();
            let limite = std::time::Duration::from_millis(self.budget_ms);
            while debut.elapsed() < limite {
                if !matches!(self.machine.run_frame(), crate::emulator::StepResult::Ok(_)) {
                    break;
                }
            }
            self.historique.suivre(&self.machine);
        }
        // La partie suit le jeu sur le disque : eteindre l'ordinateur ne coute
        // plus rien, la console retrouve son personnage au prochain lancement.
        self.tenir_la_sauvegarde();

        // Ce que l'interface prend a l'emulation : tout ce qui n'est pas la
        // tranche d'execution, moyenne sur les dernieres images.
        let emulation_ms = debut_emulation.elapsed().as_secs_f64() * 1000.0;
        let image_ms = _dt as f64 * 1000.0;
        self.cout_ui = self.cout_ui * 0.9 + (image_ms - emulation_ms).max(0.0) * 0.1;

        // Debit reel, mesure sur une demi-seconde.
        let ecoule = self.debit_depart.1.elapsed().as_secs_f64();
        if ecoule >= 0.5 {
            let faits = self.machine.cpu.cycles.saturating_sub(self.debit_depart.0);
            self.debit = faits as f64 / ecoule;
            self.debit_depart = (self.machine.cpu.cycles, std::time::Instant::now());
        }

        // Support Drag and Drop of firmware files onto the emulator
        ctx.input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first() {
                if let Some(path) = &dropped.path {
                    self.load_firmware(path.clone());
                }
            }
        });

        // 3. Top Status & Menu Bar
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tamagotchi Paradise SNC73410 Emulator").strong());

                ui.separator();

                if ui.button(if self.show_debugger { "Masquer l'inspection" } else { "Afficher l'inspection" }).clicked() {
                    self.show_debugger = !self.show_debugger;
                }

                if ui.button("💾 Inspecteur Flash").clicked() {
                    self.active_modal = ActiveModal::FlashInspector;
                }

                ui.separator();

                ui.label("Son :");
                if ui
                    .selectable_label(self.audio.enabled, if self.audio.enabled { "actif" } else { "coupe" })
                    .clicked()
                {
                    self.audio.enabled = !self.audio.enabled;
                }
                ui.add(
                    egui::Slider::new(&mut self.audio.volume, 0.0..=1.0)
                        .show_value(false)
                        .text(""),
                );

                ui.separator();

                ui.label("Shell Color:");
                for coque in ShellColor::TOUTES {
                    if ui.selectable_label(self.shell_color == coque, coque.nom()).clicked() {
                        self.shell_color = coque;
                    }
                }
                if ui.selectable_label(self.i18n.language() == Language::En, "EN").clicked() {
                    self.i18n.set_language(Language::En);
                }
            });
        });

        // 4. Panneau lateral : l'essentiel toujours, l'inspection sur demande.
        {
            SidePanel::right("debug_panel")
                .min_width(420.0)
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.add_space(4.0);

                    // Firmware File Loader Box
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Firmware / Flash Dump (.bin):").strong());
                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new("📂 Parcourir / Browse...").strong()).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Firmware Binary (*.bin, *.rom, *.hex, *.elf)", &["bin", "rom", "hex", "elf", "raw", "dump"])
                                    .set_title("Sélectionner un dump de firmware Tamagotchi")
                                    .pick_file()
                                {
                                    self.load_firmware(path.clone());
                                }
                            }

                            ui.text_edit_singleline(&mut self.load_path_input);
                            if ui.button("Load").clicked() && !self.load_path_input.is_empty() {
                                self.load_firmware(std::path::PathBuf::from(self.load_path_input.clone()));
                            }
                        });
                        if let Some(msg) = &self.status_msg {
                            ui.label(egui::RichText::new(msg).small().color(egui::Color32::from_rgb(255, 230, 80)));
                        }
                    });

                    ui.separator();

                    // Sauvegarde de la console. Elle n'a rien a voir avec les
                    // instantanes : ici on ne garde que ce que le jeu a ecrit
                    // dans sa flash, sa vraie memoire, et elle survit a
                    // l'extinction de l'ordinateur.
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("Sauvegarde de la console :").strong());
                        let suivie = self.machine.sauvegarde_active.is_some();
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("emplacement_sauvegarde")
                                .selected_text(if self.emplacement_choisi.is_empty() {
                                    "aucune".to_string()
                                } else {
                                    self.emplacement_choisi.clone()
                                })
                                .show_ui(ui, |ui| {
                                    for nom in self.emplacements.clone() {
                                        if ui
                                            .selectable_label(self.emplacement_choisi == nom, &nom)
                                            .clicked()
                                        {
                                            self.ouvrir_emplacement(nom.clone());
                                        }
                                    }
                                });
                            if ui.button("Nouvelle partie").clicked() {
                                // On cesse de suivre le fichier et on repart du
                                // dump nu : la partie enregistree reste intacte
                                // sur le disque tant qu'on n'en ouvre pas une.
                                let chemin = self.load_path_input.clone();
                                if !chemin.is_empty() {
                                    self.load_firmware(std::path::PathBuf::from(chemin));
                                    self.machine.fermer_sauvegarde();
                                    self.emplacement_choisi.clear();
                                    self.status_msg = Some(
                                        "Partie neuve, non enregistree tant qu'aucun emplacement n'est choisi"
                                            .to_string(),
                                    );
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Nouvel emplacement :");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.nouvel_emplacement)
                                    .desired_width(120.0),
                            );
                            if ui.button("Creer").clicked() {
                                let nom: String = self
                                    .nouvel_emplacement
                                    .trim()
                                    .chars()
                                    .filter(|c| {
                                        c.is_ascii_alphanumeric() || *c == '-' || *c == '_'
                                    })
                                    .collect();
                                if !nom.is_empty() {
                                    self.ouvrir_emplacement(nom);
                                    self.nouvel_emplacement.clear();
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(if suivie {
                                match &self.machine.sauvegarde_active {
                                    Some(c) => format!("Enregistree dans {}", c.display()),
                                    None => String::new(),
                                }
                            } else {
                                "Partie non enregistree".to_string()
                            })
                            .small(),
                        );
                    });

                    ui.separator();

                    // Vitesse et diagnostic : de quoi jouer, puis rapporter un
                    // blocage sans avoir a relire la trace.
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Vitesse :").strong());
                            for (nom, ms) in
                                // Au dela de quarante millisecondes l'emulation
                                // ne gagne presque plus rien et la fenetre cesse
                                // de repondre : il n'y a pas de mode plus rapide.
                                [("Pause", 0u64), ("Normale", 12), ("Rapide", 40)]
                            {
                                if ui.selectable_label(self.budget_ms == ms, nom).clicked() {
                                    self.budget_ms = ms;
                                }
                            }
                            if ui.button("Revenir en arriere").clicked() {
                                self.reculer();
                            }
                            if ui.button("Rejouer depuis le debut").clicked() {
                                let chemin = self.load_path_input.clone();
                                if !chemin.is_empty() {
                                    self.load_firmware(std::path::PathBuf::from(chemin));
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Sauver l'etat...").clicked() {
                                if let Some(chemin) = rfd::FileDialog::new()
                                    .add_filter("Etat de l'emulateur (*.tamastate)", &["tamastate"])
                                    .set_file_name("tamagotchi.tamastate")
                                    .save_file()
                                {
                                    let etat = self.machine.instantane();
                                    self.status_msg = Some(match etat.ecrire(&chemin) {
                                        Ok(()) => format!("Etat ecrit dans {}", chemin.display()),
                                        Err(e) => format!("Ecriture impossible : {}", e),
                                    });
                                }
                            }
                            if ui.button("Charger un etat...").clicked() {
                                if let Some(chemin) = rfd::FileDialog::new()
                                    .add_filter("Etat de l'emulateur (*.tamastate)", &["tamastate"])
                                    .pick_file()
                                {
                                    self.status_msg = Some(self.restaurer_fichier(&chemin));
                                }
                            }
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} instantanes automatiques",
                                    self.historique.len()
                                ))
                                .small(),
                            );
                        });
                        match self.port_web {
                            Some(port) => {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Suivi dans le navigateur : http://127.0.0.1:{}/",
                                        port
                                    ))
                                    .small()
                                    .color(egui::Color32::from_rgb(140, 220, 160)),
                                );
                            }
                            None => {
                                ui.label(
                                    egui::RichText::new("Serveur local indisponible.").small(),
                                );
                            }
                        }
                        ui.label(
                            egui::RichText::new(
                                "Clavier : A ou Fleche gauche, B ou Espace, C ou Fleche droite,                                  Entree pour l'appui de molette, Fleche haut et Fleche bas pour la                                  tourner. Les touches tenues se combinent : molette plus B ouvre le                                  menu special, A plus C reinitialise.",
                            )
                            .small(),
                        );

                        let rapport = self.diagnostic();
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Diagnostic").strong());
                            if ui.button("Copier").clicked() {
                                ui.output_mut(|o| o.copied_text = rapport.clone());
                                self.status_msg =
                                    Some("Diagnostic copie dans le presse-papiers.".to_string());
                            }
                        });
                        egui::ScrollArea::vertical()
                            .max_height(170.0)
                            .id_source("diagnostic")
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(egui::RichText::new(&rapport).monospace().small())
                                        .wrap(),
                                );
                            });
                    });

                    // Les panneaux d'inspection coutent plus cher a dessiner que
                    // l'emulation n'en gagne a tourner : ils restent replies par
                    // defaut, et le debit affiche dit tout de suite ce qu'ils
                    // prennent.
                    if !self.show_debugger {
                        return;
                    }

                    ui.separator();

                    // CPU Registers Inspector
                    CpuPanel::render(
                        ui,
                        &self.machine.cpu.regs,
                        self.machine.cpu.cycles,
                        self.machine.is_running,
                        &self.i18n,
                    );

                    ui.separator();

                    // Disassembly Stream
                    let instructions = self.machine.get_disassembly_at(self.disasm_view_addr, 12);
                    let current_pc = self.machine.cpu.regs.pc;
                    let is_running_ref = &mut self.machine.is_running;
                    let view_addr_ref = &mut self.disasm_view_addr;

                    let mut step_requested = false;
                    let mut reset_requested = false;
                    let mut new_pc_target = None;
                    DisasmPanel::render(
                        ui,
                        &instructions,
                        current_pc,
                        is_running_ref,
                        view_addr_ref,
                        || {
                            step_requested = true;
                        },
                        || {
                            reset_requested = true;
                        },
                        |target| {
                            new_pc_target = Some(target);
                        },
                    );

                    if let Some(target) = new_pc_target {
                        self.machine.cpu.regs.pc = target & !1;
                        self.disasm_view_addr = target & !1;
                    }
                    if step_requested {
                        self.machine.step();
                        self.disasm_view_addr = self.machine.cpu.regs.pc;
                    }
                    if reset_requested {
                        self.machine.reset();
                        self.disasm_view_addr = self.machine.cpu.regs.pc;
                    }

                    ui.separator();

                    // Hex Memory Viewer
                    MemoryPanel::render(
                        ui,
                        &mut self.machine.bus,
                        &mut self.machine.periph,
                        &self.machine.cpu.nvic,
                        &mut self.hex_base_addr,
                    );

                    ui.separator();

                    // UART Console
                    ConsolePanel::render(ui, &mut self.machine.periph.uart);
                });
        }

        // 5. Central Virtual Device Display & Controls
        CentralPanel::default().show(ctx, |ui| {
            let available_rect = ui.available_rect_before_wrap();

            // L'ecran est une texture : la retesseler en seize mille rectangles a
            // chaque image mangeait le temps qui doit aller a l'emulation.
            if self.machine.periph.display.dirty || self.ecran.is_none() {
                let d = &self.machine.periph.display;
                let mut pixels = Vec::with_capacity(d.width * d.height);
                for &brut in &d.vram {
                    let r = (((brut >> 11) & 0x1F) * 255 / 31) as u8;
                    let v = (((brut >> 5) & 0x3F) * 255 / 63) as u8;
                    let b = ((brut & 0x1F) * 255 / 31) as u8;
                    pixels.push(egui::Color32::from_rgb(r, v, b));
                }
                let image = egui::ColorImage { size: [d.width, d.height], pixels };
                let options = egui::TextureOptions::NEAREST;
                match &mut self.ecran {
                    Some(texture) => texture.set(image, options),
                    None => {
                        self.ecran = Some(ctx.load_texture("ecran_console", image, options));
                    }
                }
                self.machine.periph.display.dirty = false;
                self.publier();
            }

            let commandes = LcdPanel::render(
                ui,
                available_rect,
                &self.machine.periph.display,
                self.ecran.as_ref(),
                self.shell_color,
                self.angle_molette,
            );

            // Les commandes vont sur les vraies broches : bouton A en P0.9, B en
            // P0.11, C en P0.10, appui de molette en P0.8, encodeur sur P2.0 et
            // P2.1.
            for (broche, etat) in [
                (Machine::BOUTON_A, commandes.bouton_a),
                (Machine::BOUTON_B, commandes.bouton_b),
                (Machine::BOUTON_C, commandes.bouton_c),
                (Machine::BOUTON_MOLETTE, commandes.molette),
            ] {
                // Le pointeur enfonce tient la broche, un clic bref declenche
                // une impulsion assez longue pour que le firmware la voie.
                if etat.maintenu {
                    self.maintenir(broche);
                }
                if etat.clique {
                    self.presser(broche);
                    // La console est muette tant que le son est coupe dans ses
                    // reglages ; le retour sonore de l'interface, lui, doit
                    // repondre a chaque appui.
                    self.audio.play(SoundEffect::ButtonClick);
                }
            }
            if commandes.molette_tournee != 0 {
                self.tourner_molette(commandes.molette_tournee);
                self.audio.play(SoundEffect::DialTick);
                // La molette garde son elan : les deux fleches de la fenetre
                // continuent de defiler un instant apres le geste, comme sur la
                // vraie, qui est crantee mais pas instantanee.
                self.angle_molette += commandes.molette_tournee as f32 * 24.0;
            }
            // Retour au repos, doux, pour que l'animation ne s'arrete pas net.
            self.angle_molette *= 0.88;
            if self.angle_molette.abs() < 0.01 {
                self.angle_molette = 0.0;
            } else {
                ctx.request_repaint();
            }
        });

        // 6. Modals
        GuiWidgets::render_flash_inspector_modal(
            ctx,
            &self.i18n,
            &mut self.active_modal,
            &self.flash_inspector,
        );
    }
}
