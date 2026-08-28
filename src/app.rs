use eframe::egui;
use egui::{CentralPanel, Context, Key, SidePanel, TopBottomPanel};

// Le retour sonore de l'interface, clic de bouton et cran de molette, a ete
// retire : il se superposait aux tonalites du jeu et brouillait le seul son
// qui compte, celui que la console compose. `SoundEffect` reste dans
// `audio.rs` si le besoin revient, derriere un reglage.
use crate::audio::AudioEngine;
use crate::emulator::Machine;
use crate::gui::{ActiveModal, GuiWidgets, ShellColor};
use crate::hw_bridge::FlashInspector;
use crate::i18n::{I18n, Language};
use crate::ui::{ConsolePanel, CpuPanel, DisasmPanel, LcdPanel, MemoryPanel};

/// Ouvre le choix d'une image.
fn choisir_une_image() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "bmp", "gif", "webp"])
        .set_title("Choisir une image")
        .pick_file()
}

/// Reglages de cadrage d'une image : zoom, decalage et rotation.
///
/// Rend deux temoins : le premier dit qu'il faut reecrire le fichier, le second
/// qu'il faut recuire la texture. Ce sont les memes ici, mais les separer evite
/// d'oublier l'un des deux ailleurs.
fn cadrage_reglable(
    ui: &mut egui::Ui,
    cadrage: &mut crate::gui::fond::Cadrage,
    quoi: &str,
) -> (bool, bool) {
    let mut change = false;
    change |= ui
        .add(egui::Slider::new(&mut cadrage.zoom, 0.1..=4.0).text(format!("zoom {}", quoi)))
        .changed();
    change |= ui
        .add(egui::Slider::new(&mut cadrage.dx, -1.5..=1.5).text("gauche / droite"))
        .changed();
    change |= ui
        .add(egui::Slider::new(&mut cadrage.dy, -1.5..=1.5).text("haut / bas"))
        .changed();
    change |= ui
        .add(egui::Slider::new(&mut cadrage.rotation, -180.0..=180.0).text("rotation"))
        .changed();
    if ui.button(format!("Recentrer le {}", quoi)).clicked() {
        *cadrage = Default::default();
        change = true;
    }
    (change, change)
}

/// Rend un nom de partie utilisable comme nom de fichier.
///
/// Le nom saisi devient un fichier dans le dossier de la console : tout ce qui
/// designe un chemin ou fache le systeme est remplace par un tiret.
fn nettoyer_nom(brut: &str) -> String {
    brut.trim()
        .chars()
        .map(|c| match c {
            c if r#"/\:*?"<>|."#.contains(c) => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Ouvre un dossier dans l'explorateur du systeme.
///
/// Trois commandes selon la plateforme, aucune dependance de plus. Un echec
/// n'est pas signale : c'est un confort, pas une fonction.
fn open_dossier(chemin: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let commande = "explorer";
    #[cfg(target_os = "macos")]
    let commande = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let commande = "xdg-open";
    std::process::Command::new(commande).arg(chemin).spawn().map(|_| ())
}

/// Onglet du panneau lateral.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Onglet {
    /// Console, partie, points de reprise, diagnostic.
    Console,
    /// Habillage de la coque.
    Personnalisation,
}

/// Ce que la fenetre montre.
///
/// L'inspection coute plus cher a dessiner que l'emulation n'en gagne a
/// tourner : desassembleur, registres, memoire et diagnostic sont refaits a
/// chaque image. En mode jeu rien de tout cela n'existe, et la fenetre se
/// reduit a la console elle meme, decoupee sur le bureau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Ecran de depart : choisir un dump, un emplacement, puis jouer.
    Accueil,
    /// La console seule, sans cadre de fenetre, deplacable sur le bureau.
    Jeu,
    /// La fenetre complete, avec tous les panneaux.
    Inspection,
}

pub struct TamagotchiApp {
    /// Ce que la fenetre montre.
    pub mode: Mode,
    /// Dernier mode applique a la fenetre, pour ne poser les commandes de
    /// viewport qu'au changement.
    mode_applique: Option<Mode>,
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
    /// Points de reprise horodates, ecrits sur le disque et propres au dump.
    pub reprises: crate::emulator::reprises::Journal,
    /// Onglet ouvert dans le panneau lateral.
    pub onglet: Onglet,
    /// Vue de restauration ouverte.
    pub voir_reprises: bool,
    /// Habillage de la console courante : papier, titre et vitre.
    pub fond: crate::gui::fond::Habillage,
    /// Texture du papier, papier et masque deja cuits ensemble.
    fond_texture: Option<egui::TextureHandle>,
    /// Texture du papier propre a la calotte.
    chapeau_texture: Option<egui::TextureHandle>,
    /// Nom en cours de saisie pour une nouvelle sauvegarde.
    ///
    /// `Some` tant que la fenetre de saisie est ouverte. Un menu contextuel ne
    /// peut pas porter de champ de texte utilisable : il se referme au premier
    /// clic ailleurs.
    saisie_sauvegarde: Option<String>,
    /// Le papier doit etre relu a la prochaine image.
    ///
    /// Il faut un contexte egui pour en faire une texture, et le chargement
    /// d'un dump peut arriver hors d'une image, au demarrage notamment.
    papier_a_relire: bool,
    /// Etat publie au serveur local et commandes qui en reviennent.
    pub partage: std::sync::Arc<std::sync::Mutex<crate::web::Partage>>,
    /// Port du serveur local, quand il a pu demarrer.
    pub port_web: Option<u16>,
    /// Temoin qui arrete le serveur local.
    serveur_actif: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Taille de la fenetre du mode jeu, en fraction de sa taille de base.
    pub zoom_jeu: f32,
    /// Fenetre du mode jeu maintenue au dessus des autres.
    pub toujours_devant: bool,
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
    /// Vitesse d'ecoulement du temps de la console, 1 pour le temps reel.
    ///
    /// Sans gouverneur, l'emulateur va aussi vite que la machine le permet, et
    /// la console vit plusieurs fois plus vite que la vraie. Elle pousse alors
    /// plus d'images que la fenetre n'en affiche, ce qui saccade en plus d'etre
    /// faux. Zero met en pause.
    pub vitesse: f32,
    /// Note en cours et cycle ou elle a commence, pour suivre la melodie sans
    /// rien manquer entre deux images.
    pub note_courante: f32,
    pub note_depuis: u64,
    /// Vrai tant que le firmware jouait a l'image precedente.
    pub son_jouait: bool,
    /// Derniere recherche du tableau des voix, pour ne pas la refaire a chaque
    /// image quand elle echoue.
    derniere_recherche_voix: std::time::Instant,
    /// Note heritee de la melodie precedente, et cycle jusqu'auquel s'en mefier.
    ///
    /// Le tableau de voix garde sa derniere valeur au silence. Quand le
    /// firmware releve son drapeau de son, il met encore quelques
    /// millisecondes a ecrire la premiere note : jusque la, la voix annonce
    /// celle de la melodie precedente, et l'ancienne s'entendait en tete de
    /// chaque son.
    pub note_perimee: f32,
    pub perimee_jusqu: u64,
    /// Notes relevees pendant la tranche d'emulation, avec leur duree en cycles.
    pub notes: Vec<(f32, u64)>,
    /// Cycles dus a la console, en retard a rattraper.
    ///
    /// La dette est bornee : apres un a coup de l'interface, il ne faut pas que
    /// l'emulation reparte en trombe pour se rattraper.
    pub cycles_dus: f64,
}

impl TamagotchiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let audio = AudioEngine::new();
        let i18n = I18n::default();
        let machine = Machine::new();

        // Le serveur local reste eteint au demarrage. Il coute une copie de la
        // memoire d'ecran et un rapport de diagnostic a chaque image, pour un
        // service dont on ne se sert pas en jouant. Il s'allume depuis le
        // panneau d'inspection.
        let partage = std::sync::Arc::new(std::sync::Mutex::new(crate::web::Partage::default()));
        let port_web: Option<u16> = None;

        // Les premieres versions rangeaient les parties a cote du binaire :
        // elles sont deplacees une fois vers le dossier de donnees du systeme.
        crate::emulator::sauvegarde::migrer_les_anciennes_donnees();

        let mut app = Self {
            mode: Mode::Accueil,
            mode_applique: None,
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
            reprises: crate::emulator::reprises::Journal::default(),
            onglet: Onglet::Console,
            voir_reprises: false,
            fond: crate::gui::fond::Habillage::default(),
            fond_texture: None,
            chapeau_texture: None,
            papier_a_relire: true,
            saisie_sauvegarde: None,
            partage,
            port_web,
            serveur_actif: None,
            zoom_jeu: 1.0,
            toujours_devant: false,
            debit: 0.0,
            debit_depart: (0, std::time::Instant::now()),
            cout_ui: 0.0,
            emplacements: Vec::new(),
            emplacement_choisi: String::new(),
            nouvel_emplacement: String::new(),
            derniere_ecriture: std::time::Instant::now(),
            angle_molette: 0.0,
            note_courante: 0.0,
            note_depuis: 0,
            son_jouait: false,
            derniere_recherche_voix: std::time::Instant::now(),
            note_perimee: 0.0,
            perimee_jusqu: 0,
            notes: Vec::new(),
            vitesse: 1.0,
            cycles_dus: 0.0,
        };
        // La console reprend ou elle en etait, comme un vrai Tamagotchi
        // qu'on rallume : le dump et l'emplacement du dernier lancement sont
        // rouverts sans rien demander.
        app.reprendre_la_derniere_partie();
        app
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
                // La flash est revenue a l'image du dump : la console doit
                // repartir dessus, sinon elle continue sur l'etat en memoire de
                // la partie precedente et l'ecran ne bouge pas.
                let tournait = self.machine.is_running;
                self.machine.reset();
                self.machine.is_running = tournait;
                self.historique.vider();
                self.status_msg = Some(format!("Nouvelle partie {}", nom));
            }
            Err(e) => {
                self.status_msg = Some(format!("Sauvegarde illisible : {}", e));
                return;
            }
        }
        self.emplacement_choisi = nom;
        self.rafraichir_emplacements();
        self.retenir_la_partie();
        if let Some(empreinte) = self.machine.empreinte.clone() {
            let dossier = crate::emulator::sauvegarde::dossier_reprises(
                &empreinte,
                &self.emplacement_choisi,
            );
            self.reprises.ouvrir(dossier);
        }
    }

    /// Note le dump et l'emplacement en cours, pour les retrouver au prochain
    /// lancement.
    fn retenir_la_partie(&self) {
        // Rien a retenir tant qu'aucun dump n'est reellement charge : ecrire le
        // chemin d'un fichier qui a echoue le ferait retenter a chaque
        // lancement.
        if self.load_path_input.is_empty() || self.machine.empreinte.is_none() {
            return;
        }
        crate::emulator::sauvegarde::ecrire_derniere_partie(
            &crate::emulator::sauvegarde::DernierePartie {
                dump: self.load_path_input.clone(),
                emplacement: self.emplacement_choisi.clone(),
                mode: match self.mode {
                    Mode::Accueil => "accueil",
                    Mode::Jeu => "jeu",
                    Mode::Inspection => "inspection",
                }
                .to_string(),
                son: self.audio.enabled,
                volume: self.audio.volume,
                hauteur: self.audio.hauteur,
                coque: self.shell_color.nom().to_string(),
                zoom_jeu: self.zoom_jeu,
                toujours_devant: self.toujours_devant,
            },
        );
    }

    /// Rouvre la partie du dernier lancement, si son dump est toujours la.
    ///
    /// Rien n'est signale quand il manque : c'est le cas d'un premier
    /// demarrage, ou d'un fichier deplace, et l'ecran de chargement suffit.
    fn reprendre_la_derniere_partie(&mut self) {
        let Some(partie) = crate::emulator::sauvegarde::lire_derniere_partie() else {
            return;
        };
        // Les reglages de son valent meme sans dump : ils ne dependent pas de
        // la console chargee.
        self.audio.enabled = partie.son;
        self.audio.volume = partie.volume.clamp(0.0, 1.0);
        self.audio.hauteur = if partie.hauteur > 0.0 { partie.hauteur } else { 1.0 };
        self.zoom_jeu = if partie.zoom_jeu > 0.0 { partie.zoom_jeu.clamp(0.5, 3.0) } else { 1.0 };
        self.toujours_devant = partie.toujours_devant;

        let chemin = std::path::PathBuf::from(&partie.dump);
        if !chemin.is_file() {
            return;
        }
        self.load_firmware(chemin);
        if !partie.emplacement.is_empty() && partie.emplacement != self.emplacement_choisi {
            self.ouvrir_emplacement(partie.emplacement);
        }
        // La coque suit l'edition par defaut ; un choix a la main la remplace.
        if let Some(coque) =
            ShellColor::TOUTES.iter().find(|c| c.nom() == partie.coque)
        {
            self.shell_color = *coque;
        }
        // On ne rallume en mode jeu que si la console y est prete : sans dump
        // demarrable, l'accueil est le seul endroit ou faire quelque chose.
        self.mode = match partie.mode.as_str() {
            "jeu" if self.machine.is_running => Mode::Jeu,
            "inspection" => Mode::Inspection,
            _ => Mode::Accueil,
        };
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
                // La trace des acces peripheriques coute une recherche par
                // acces, et l'ecran en fait des millions par seconde. Elle sert
                // aux sondes, pas au jeu : sans elle l'emulation tient le temps
                // reel, avec elle non.
                self.machine.bus.mmio_trace.enabled = false;
                self.machine.bus.mmio_trace.clear();
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
                // Le papier suit la console : il est relu a chaque changement.
                self.papier_a_relire = true;
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
    ///
    /// Ne fait rien tant que le serveur est eteint : sans lecteur en face, la
    /// copie de la memoire d'ecran et la mise en forme du rapport seraient
    /// payees soixante fois par seconde pour rien.
    fn publier(&mut self) {
        if self.port_web.is_none() {
            return;
        }
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
             vitesse       demandee {}   atteinte {:.2} fois le temps reel\n\
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
            if self.vitesse == 0.0 {
                "pause".to_string()
            } else if self.vitesse.is_infinite() {
                "max".to_string()
            } else {
                format!("x{}", self.vitesse)
            },
            self.debit / crate::emulator::peripherals::snsys::CYCLES_PAR_SECONDE as f64,
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

    /// Note a jouer, la valeur heritee de la melodie precedente ecartee.
    ///
    /// Au moment ou le drapeau de son se leve, la voix porte encore la
    /// derniere note du son d'avant. On la tient pour du silence tant qu'elle
    /// n'a pas change, et au plus cinquante millisecondes : passe ce delai
    /// c'est que le firmware la joue vraiment.
    fn note_jouee(&mut self) -> f32 {
        let joue = self.machine.son_en_cours();
        // Le tableau des voix est cherche au debut de chaque son, et repris
        // tant qu'il n'a rien donne. Une seule tentative ne suffit pas : au
        // premier son la table peut n'etre pas encore allouee, et sur Jade
        // Forest le drapeau de son reste leve si longtemps que le debut de son
        // suivant, seule occasion de reessayer, n'arrivait jamais. La reprise
        // est espacee d'une demi seconde, le balayage lisant toute la memoire
        // vive.
        if joue
            && (!self.son_jouait
                || (self.machine.voix.is_empty()
                    && self.derniere_recherche_voix.elapsed().as_secs_f32() > 0.5))
        {
            self.machine.localiser_les_voix();
            self.derniere_recherche_voix = std::time::Instant::now();
        }
        if joue && !self.son_jouait {
            self.note_perimee = self.machine.note_courante();
            self.perimee_jusqu = self.machine.cpu.cycles
                + crate::emulator::peripherals::snsys::CYCLES_PAR_SECONDE as u64 / 20;
        }
        self.son_jouait = joue;

        let note = self.machine.note_courante();
        if note > 0.0 && self.machine.cpu.cycles < self.perimee_jusqu {
            if (note - self.note_perimee).abs() < 0.5 {
                return 0.0;
            }
            // Une autre note est arrivee : la voix est a jour, plus de doute.
            self.perimee_jusqu = 0;
        }
        note
    }

    /// Recopie la memoire d'ecran de la console dans une texture.
    ///
    /// L'ecran est une texture, pas seize mille rectangles : le tesseler a
    /// chaque image mangeait le temps qui doit aller a l'emulation.
    fn rafraichir_la_texture(&mut self, ctx: &Context) {
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
    }

    /// Dessine la coque et envoie ses commandes sur les broches.
    fn dessiner_la_console(&mut self, ctx: &Context, ui: &mut egui::Ui, zone: egui::Rect) {
        // Les papiers sont deja composes, masque compris : le panneau n'a plus
        // qu'a les poser.
        let habits = crate::ui::lcd_panel::Habits {
            reglages: &self.fond,
            papier: self.fond_texture.as_ref(),
            chapeau: self.chapeau_texture.as_ref(),
        };
        let commandes = LcdPanel::render(
            ui,
            zone,
            &self.machine.periph.display,
            self.ecran.as_ref(),
            self.shell_color,
            self.angle_molette,
            &habits,
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
            }
        }
        if commandes.molette_tournee != 0 {
            self.tourner_molette(commandes.molette_tournee);
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
    }

    /// Dossier ou vit le papier de la console courante.
    ///
    /// Il suit la console et non la partie : sur la vraie machine le papier est
    /// glisse dans la coque, il ne change pas quand le Tamagotchi change.
    fn dossier_du_papier(&self) -> Option<std::path::PathBuf> {
        let empreinte = self.machine.empreinte.as_ref()?;
        Some(crate::emulator::sauvegarde::dossier_du_dump(empreinte))
    }

    /// Relit le papier de la console courante et en refait la texture.
    fn recharger_le_papier(&mut self, ctx: &Context) {
        self.fond = crate::gui::fond::Habillage::default();
        self.fond_texture = None;
        self.chapeau_texture = None;
        let Some(dossier) = self.dossier_du_papier() else {
            return;
        };
        self.fond = crate::gui::fond::Habillage::lire(&dossier);
        self.recomposer_les_papiers(ctx);
    }

    /// Recuit les papiers dans leurs textures.
    ///
    /// A appeler des qu'un cadrage change : la transparence est calculee une
    /// fois ici, et non a chaque image.
    fn recomposer_les_papiers(&mut self, ctx: &Context) {
        use crate::gui::fond;
        let Some(dossier) = self.dossier_du_papier() else {
            return;
        };
        let charger = |nom: &str| -> Option<image::RgbaImage> {
            if nom.is_empty() {
                return None;
            }
            fond::charger_image(&dossier.join(nom)).ok()
        };
        let masque = charger(&self.fond.masque);

        self.fond_texture = charger(&self.fond.fichier).map(|papier| {
            let image = fond::composer(
                &papier,
                &self.fond.papier,
                masque.as_ref().map(|m| (m, &self.fond.masque_cadrage)),
            );
            ctx.load_texture("papier_coque", image, egui::TextureOptions::LINEAR)
        });

        self.chapeau_texture = charger(&self.fond.chapeau_fichier).map(|papier| {
            let image = fond::composer(&papier, &self.fond.chapeau_cadrage, None);
            ctx.load_texture("papier_chapeau", image, egui::TextureOptions::LINEAR)
        });
    }

    /// Habillage de la coque : papiers, masque, mot imprime, vitre, couleurs.
    ///
    /// Tout y est retenu par console, comme le papier de la vraie machine suit
    /// la coque et non le Tamagotchi.
    fn dessiner_l_habillage(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        ui.label(egui::RichText::new("Habillage de la coque").strong());
        let Some(dossier) = self.dossier_du_papier() else {
            ui.label(egui::RichText::new("Charge une console d'abord.").small());
            return;
        };
        let mut change = false;
        let mut recomposer = false;

        // --- le papier
        ui.horizontal(|ui| {
            if ui
                .button("Papier...")
                .on_hover_text(
                    "L'image se glisse sous la fenetre transparente, comme le papier \
                     imprime de la vraie console.",
                )
                .clicked()
            {
                if let Some(chemin) = choisir_une_image() {
                    match crate::gui::fond::adopter_image(&chemin, &dossier, "fond") {
                        Ok(nom) => {
                            self.fond.fichier = nom;
                            self.fond.papier = Default::default();
                            change = true;
                            recomposer = true;
                        }
                        Err(e) => self.status_msg = Some(format!("Image refusee : {}", e)),
                    }
                }
            }
            if !self.fond.fichier.is_empty() && ui.button("Retirer").clicked() {
                self.fond.retirer_le_papier(&dossier);
                recomposer = true;
            }
        });
        if !self.fond.fichier.is_empty() {
            let (c, r) = cadrage_reglable(ui, &mut self.fond.papier, "papier");
            change |= c;
            recomposer |= r;
            change |= ui
                .checkbox(&mut self.fond.couvre_tout, "Couvrir toute la coque")
                .changed();
            if self.fond.couvre_tout {
                change |= ui
                    .checkbox(&mut self.fond.inclut_le_chapeau, "y compris le chapeau")
                    .changed();
            }
        }

        ui.separator();

        // --- le masque de decoupe
        ui.horizontal(|ui| {
            if ui
                .button("Masque...")
                .on_hover_text(
                    "Image en noir et blanc : le noir laisse voir le papier, le blanc \
                     le cache, et ce qui est hors de l'image est cache aussi.",
                )
                .clicked()
            {
                if let Some(chemin) = choisir_une_image() {
                    match crate::gui::fond::adopter_image(&chemin, &dossier, "masque") {
                        Ok(nom) => {
                            self.fond.masque = nom;
                            self.fond.masque_cadrage = Default::default();
                            change = true;
                            recomposer = true;
                        }
                        Err(e) => self.status_msg = Some(format!("Image refusee : {}", e)),
                    }
                }
            }
            if !self.fond.masque.is_empty() && ui.button("Retirer").clicked() {
                self.fond.retirer_le_masque(&dossier);
                recomposer = true;
            }
        });
        if !self.fond.masque.is_empty() {
            let (c, r) = cadrage_reglable(ui, &mut self.fond.masque_cadrage, "masque");
            change |= c;
            recomposer |= r;
        }

        ui.separator();

        // --- le chapeau, quand il n'est pas couvert par le papier general
        if !(self.fond.couvre_tout && self.fond.inclut_le_chapeau) {
            ui.label(egui::RichText::new("Chapeau").strong());
            ui.horizontal(|ui| {
                let mut teinte = self.fond.chapeau_couleur.is_some();
                if ui.checkbox(&mut teinte, "couleur").changed() {
                    self.fond.chapeau_couleur = if teinte {
                        let c = self.shell_color.couleurs().calotte;
                        Some([c.r(), c.g(), c.b()])
                    } else {
                        None
                    };
                    change = true;
                }
                if let Some(rvb) = &mut self.fond.chapeau_couleur {
                    change |= ui.color_edit_button_srgb(rvb).changed();
                } else {
                    ui.label(egui::RichText::new("celle de la coque").small());
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Image du chapeau...").clicked() {
                    if let Some(chemin) = choisir_une_image() {
                        match crate::gui::fond::adopter_image(&chemin, &dossier, "chapeau") {
                            Ok(nom) => {
                                self.fond.chapeau_fichier = nom;
                                self.fond.chapeau_cadrage = Default::default();
                                change = true;
                                recomposer = true;
                            }
                            Err(e) => self.status_msg = Some(format!("Image refusee : {}", e)),
                        }
                    }
                }
                if !self.fond.chapeau_fichier.is_empty() && ui.button("Retirer").clicked() {
                    self.fond.retirer_le_chapeau(&dossier);
                    recomposer = true;
                }
            });
            if !self.fond.chapeau_fichier.is_empty() {
                let (c, r) = cadrage_reglable(ui, &mut self.fond.chapeau_cadrage, "chapeau");
                change |= c;
                recomposer |= r;
            }
            ui.separator();
        }

        // --- le mot imprime au dessus de l'ecran
        change |= ui
            .checkbox(&mut self.fond.titre_visible, "Mot imprime")
            .changed();
        if self.fond.titre_visible {
            change |= ui
                .add(
                    egui::TextEdit::singleline(&mut self.fond.titre)
                        .hint_text("TAMAGOTCHI")
                        .desired_width(180.0),
                )
                .changed();
            change |= ui
                .add(egui::Slider::new(&mut self.fond.titre_taille, 0.3..=3.0).text("taille"))
                .changed();
            ui.horizontal(|ui| {
                let mut choisie = self.fond.titre_couleur.is_some();
                if ui.checkbox(&mut choisie, "couleur").changed() {
                    self.fond.titre_couleur = if choisie {
                        let a = self.shell_color.couleurs().accent;
                        Some([a.r(), a.g(), a.b()])
                    } else {
                        None
                    };
                    change = true;
                }
                if let Some(rvb) = &mut self.fond.titre_couleur {
                    change |= ui.color_edit_button_srgb(rvb).changed();
                } else {
                    ui.label(egui::RichText::new("celle de la coque").small());
                }
            });
        }

        ui.separator();

        // --- la vitre autour de la dalle
        change |= ui
            .checkbox(&mut self.fond.vitre_visible, "Vitre autour de l'ecran")
            .changed();
        if self.fond.vitre_visible {
            change |= ui
                .add(
                    egui::Slider::new(&mut self.fond.vitre_epaisseur, 0.0..=0.10)
                        .text("epaisseur"),
                )
                .changed();
            change |= ui
                .color_edit_button_srgb(&mut self.fond.vitre_couleur)
                .changed();
        } else {
            ui.label(
                egui::RichText::new("Sans vitre, c'est la dalle qui prend l'arrondi.")
                    .small()
                    .color(egui::Color32::GRAY),
            );
        }

        ui.separator();

        // --- la dalle
        ui.label(egui::RichText::new("Ecran").strong());
        change |= ui
            .add(egui::Slider::new(&mut self.fond.ecran_taille, 0.4..=1.8).text("taille"))
            .changed();
        change |= ui
            .add(egui::Slider::new(&mut self.fond.ecran_dy, -0.4..=0.4).text("haut / bas"))
            .changed();
        if ui.button("Ecran d'origine").clicked() {
            self.fond.ecran_taille = 1.0;
            self.fond.ecran_dy = 0.0;
            change = true;
        }

        ui.separator();

        // --- les commandes
        ui.label(egui::RichText::new("Commandes").strong());
        for (etiquette, defaut, champ) in [
            ("boutons", self.shell_color.couleurs().bouton, 0usize),
            ("molette", self.shell_color.couleurs().accent, 1),
        ] {
            ui.horizontal(|ui| {
                let actuel = if champ == 0 {
                    &mut self.fond.bouton_couleur
                } else {
                    &mut self.fond.molette_couleur
                };
                let mut choisie = actuel.is_some();
                if ui.checkbox(&mut choisie, etiquette).changed() {
                    *actuel = if choisie {
                        Some([defaut.r(), defaut.g(), defaut.b()])
                    } else {
                        None
                    };
                    change = true;
                }
                if let Some(rvb) = actuel {
                    change |= ui.color_edit_button_srgb(rvb).changed();
                } else {
                    ui.label(egui::RichText::new("celle de la coque").small());
                }
            });
        }

        if change || recomposer {
            self.fond.ecrire(&dossier);
        }
        if recomposer {
            self.recomposer_les_papiers(ctx);
        }
    }

    /// Fenetre de saisie du nom d'une nouvelle sauvegarde.
    ///
    /// Elle est dessinee dans tous les modes : le mode jeu n'a pas de panneau
    /// ou loger un champ de texte, et le menu contextuel se referme des qu'on
    /// clique dedans.
    fn dessiner_la_saisie(&mut self, ctx: &Context) {
        let Some(mut nom) = self.saisie_sauvegarde.clone() else {
            return;
        };
        let mut ouverte = true;
        let mut valider = false;
        let mut annuler = false;
        egui::Window::new("Nouvelle sauvegarde")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut ouverte)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Nom de la partie").small());
                let champ = ui.add(
                    egui::TextEdit::singleline(&mut nom)
                        .hint_text("ma partie")
                        .desired_width(200.0),
                );
                champ.request_focus();
                if champ.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    valider = true;
                }
                let existe = self.emplacements.iter().any(|e| *e == nettoyer_nom(&nom));
                if existe {
                    ui.label(
                        egui::RichText::new("Ce nom existe deja, il sera ouvert tel quel.")
                            .small()
                            .color(egui::Color32::from_rgb(220, 200, 90)),
                    );
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !nettoyer_nom(&nom).is_empty(),
                            egui::Button::new("Creer"),
                        )
                        .clicked()
                    {
                        valider = true;
                    }
                    if ui.button("Annuler").clicked() {
                        annuler = true;
                    }
                });
            });

        if valider {
            let propre = nettoyer_nom(&nom);
            self.saisie_sauvegarde = None;
            if !propre.is_empty() {
                self.ouvrir_emplacement(propre);
            }
        } else if annuler || !ouverte {
            self.saisie_sauvegarde = None;
        } else {
            self.saisie_sauvegarde = Some(nom);
        }
    }

    /// Premier nom de partie libre, `partie-2`, `partie-3`, et ainsi de suite.
    ///
    /// Un menu contextuel n'est pas l'endroit ou saisir un nom : on en trouve
    /// un tout seul, et il se renomme depuis le panneau lateral.
    fn nom_de_partie_libre(&self) -> String {
        for numero in 2..1000 {
            let nom = format!("partie-{}", numero);
            if !self.emplacements.iter().any(|e| *e == nom) {
                return nom;
            }
        }
        "partie".to_string()
    }

    /// Pose un point de reprise tout de suite.
    fn poser_un_point(&mut self) {
        if self.reprises.prendre_maintenant(&self.machine) {
            self.status_msg = Some("Point de reprise pose.".to_string());
        } else {
            self.status_msg = Some("Ouvre une partie avant de poser un point.".to_string());
        }
    }

    /// Adopte un instantane venu d'ailleurs.
    fn importer_un_point(&mut self) {
        let Some(chemin) = rfd::FileDialog::new()
            .add_filter("Instantane", &["tamastate"])
            .set_title("Importer un instantane")
            .pick_file()
        else {
            return;
        };
        match self.reprises.adopter(&chemin) {
            Ok(()) => self.status_msg = Some("Instantane importe.".to_string()),
            Err(e) => self.status_msg = Some(format!("Instantane refuse : {}", e)),
        }
    }

    /// Ecrit l'etat courant de la console dans un fichier choisi.
    ///
    /// Un instantane ne porte que les pages de flash modifiees : il ne veut
    /// rien dire sans son dump, et c'est pour cela qu'il retient le chemin de
    /// celui ci.
    fn exporter_l_etat(&mut self) {
        if self.machine.empreinte.is_none() {
            self.status_msg = Some("Charge une console avant d'exporter.".to_string());
            return;
        }
        let defaut = format!(
            "{}-{}.tamastate",
            if self.emplacement_choisi.is_empty() { "partie" } else { &self.emplacement_choisi },
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        let Some(chemin) = rfd::FileDialog::new()
            .add_filter("Instantane", &["tamastate"])
            .set_file_name(defaut)
            .set_title("Exporter l'etat de la console")
            .save_file()
        else {
            return;
        };
        match self.machine.instantane().ecrire(&chemin) {
            Ok(()) => self.status_msg = Some("Etat exporte.".to_string()),
            Err(e) => self.status_msg = Some(format!("Export impossible : {}", e)),
        }
    }

    /// Recopie un point de reprise vers un fichier choisi.
    fn exporter_un_point(&mut self, indice: usize) {
        let Some(source) = self.reprises.chemin_du_point(indice) else {
            return;
        };
        let defaut = self
            .reprises
            .points()
            .get(indice)
            .map(|p| format!("point-{}.tamastate", p.quand.format("%Y%m%d-%H%M%S")))
            .unwrap_or_else(|| "point.tamastate".to_string());
        let Some(cible) = rfd::FileDialog::new()
            .add_filter("Instantane", &["tamastate"])
            .set_file_name(defaut)
            .set_title("Exporter ce point de reprise")
            .save_file()
        else {
            return;
        };
        match std::fs::copy(&source, &cible) {
            Ok(_) => self.status_msg = Some("Point exporte.".to_string()),
            Err(e) => self.status_msg = Some(format!("Export impossible : {}", e)),
        }
    }

    /// Restaure un point de reprise et remet les commandes au repos.
    fn revenir_au_point(&mut self, indice: usize) {
        let Some(etat) = self.reprises.restaurer(indice) else {
            self.status_msg = Some("Point de reprise illisible.".to_string());
            return;
        };
        let quand = self
            .reprises
            .points()
            .get(indice)
            .map(|p| p.quand.format("%H:%M").to_string())
            .unwrap_or_default();
        self.machine.restaurer(&etat);
        self.appuis.clear();
        self.maintenus.clear();
        self.tenus_distants.clear();
        self.phases_encodeur.clear();
        self.historique.vider();
        self.debit_depart = (self.machine.cpu.cycles, std::time::Instant::now());
        self.status_msg = Some(format!("Console revenue a {}.", quand));
    }

    /// Liste des points de reprise, avec l'heure et l'age de chacun.
    fn dessiner_les_reprises(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Points de reprise").strong());
            ui.label(
                egui::RichText::new(format!("{} points", self.reprises.points().len())).small(),
            );
        });
        if !self.reprises.actif() {
            ui.label(egui::RichText::new("Ouvre une partie pour en garder.").small());
            return;
        }
        ui.horizontal(|ui| {
            if ui.button("Poser un point").clicked() {
                self.poser_un_point();
            }
            if ui
                .button("Importer...")
                .on_hover_text("Ajoute un fichier .tamastate a la liste ci dessous")
                .clicked()
            {
                self.importer_un_point();
            }
            if ui.button("Exporter l'etat").clicked() {
                self.exporter_l_etat();
            }
        });
        if self.reprises.points().is_empty() {
            ui.label(
                egui::RichText::new("Le premier point est pris apres une minute de jeu.")
                    .small()
                    .color(egui::Color32::GRAY),
            );
            return;
        }
        ui.label(
            egui::RichText::new(
                "Cliquez sur une heure pour y ramener la console. Un point est pris                  chaque minute et garde jusqu'a douze heures.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
        // Du plus recent au plus ancien : c'est dans cet ordre qu'on cherche.
        let mut a_restaurer = None;
        let mut a_oublier = None;
        let mut a_exporter = None;
        egui::ScrollArea::vertical()
            .max_height(150.0)
            .id_salt("reprises")
            .show(ui, |ui| {
                for (indice, point) in self.reprises.points().iter().enumerate().rev() {
                    ui.horizontal(|ui| {
                        if ui
                            .button(egui::RichText::new(point.quand.format("%H:%M").to_string()))
                            .on_hover_text("Ramener la console a cet instant")
                            .clicked()
                        {
                            a_restaurer = Some(indice);
                        }
                        ui.label(egui::RichText::new(point.age_lisible()).small());
                        ui.label(
                            egui::RichText::new(point.quand.format("%d/%m").to_string())
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        if ui
                            .small_button("^")
                            .on_hover_text("Exporter ce point vers un fichier")
                            .clicked()
                        {
                            a_exporter = Some(indice);
                        }
                        if ui.small_button("x").on_hover_text("Effacer ce point").clicked() {
                            a_oublier = Some(indice);
                        }
                    });
                }
            });
        if let Some(indice) = a_restaurer {
            self.revenir_au_point(indice);
        }
        if let Some(indice) = a_exporter {
            self.exporter_un_point(indice);
        }
        if let Some(indice) = a_oublier {
            self.reprises.oublier(indice);
        }
    }

    /// Menu de choix de console, pour changer d'edition sans repasser par
    /// l'accueil.
    ///
    /// Les dumps proposes sont ceux du dossier de donnees. Un dump importe
    /// d'ailleurs y est recopie a l'import, il apparait donc ici ensuite.
    fn menu_des_consoles(&mut self, ui: &mut egui::Ui) {
        let connus = crate::emulator::sauvegarde::firmwares_connus();
        let courant = std::path::Path::new(&self.load_path_input)
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "aucune".to_string());
        let mut voulue = None;
        ui.menu_button(format!("Console : {}", courant), |ui| {
            if connus.is_empty() {
                ui.label(
                    egui::RichText::new("Aucun dump dans le dossier de donnees.").small(),
                );
            }
            for chemin in &connus {
                let nom = chemin
                    .file_stem()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let choisi = self.load_path_input == chemin.to_string_lossy().to_string();
                if ui.selectable_label(choisi, &nom).clicked() {
                    if !choisi {
                        voulue = Some(chemin.clone());
                    }
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button("Importer un dump...").clicked() {
                if let Some(chemin) = rfd::FileDialog::new()
                    .add_filter("Dump de flash", &["bin", "rom", "dump", "raw"])
                    .set_title("Choisir un dump de flash Tamagotchi")
                    .pick_file()
                {
                    voulue = Some(crate::emulator::sauvegarde::adopter_firmware(&chemin));
                }
                ui.close_menu();
            }
        });
        if let Some(chemin) = voulue {
            self.load_firmware(chemin);
        }
    }

    /// Pose la fenetre pour le mode courant, une seule fois par changement.
    fn appliquer_le_mode(&mut self, ctx: &Context) {
        use egui::ViewportCommand as Cmd;
        if self.mode_applique == Some(self.mode) {
            return;
        }
        self.mode_applique = Some(self.mode);
        self.retenir_la_partie();
        match self.mode {
            Mode::Accueil => {
                ctx.send_viewport_cmd(Cmd::Decorations(true));
                ctx.send_viewport_cmd(Cmd::InnerSize(egui::vec2(560.0, 600.0)));
            }
            Mode::Jeu => {
                // Sans cadre ni barre de titre : ne reste que la coque,
                // decoupee sur le bureau. La fenetre garde la proportion de
                // l'oeuf, un peu plus haute que large.
                ctx.send_viewport_cmd(Cmd::Decorations(false));
                ctx.send_viewport_cmd(Cmd::WindowLevel(if self.toujours_devant {
                    egui::viewport::WindowLevel::AlwaysOnTop
                } else {
                    egui::viewport::WindowLevel::Normal
                }));
                // Forme de la console : 6,5 sur 7,5, plus le debord de la
                // molette. La fenetre est donc presque carree.
                let z = self.zoom_jeu.clamp(0.5, 3.0);
                ctx.send_viewport_cmd(Cmd::InnerSize(egui::vec2(430.0 * z, 450.0 * z)));
            }
            Mode::Inspection => {
                ctx.send_viewport_cmd(Cmd::Decorations(true));
                ctx.send_viewport_cmd(Cmd::InnerSize(egui::vec2(1180.0, 800.0)));
            }
        }
    }

    /// Ecran de depart : le dump, l'emplacement, puis on joue.
    fn dessiner_accueil(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Tamagotchi Paradise").size(28.0).strong());
                ui.label(
                    egui::RichText::new("emulateur du SoC Sonix SNC7340")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
            ui.add_space(20.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Console").strong());
                // Les dumps deja connus, un bouton chacun : changer d'edition
                // ne doit pas demander de retrouver un fichier.
                let connus = crate::emulator::sauvegarde::firmwares_connus();
                if connus.is_empty() {
                    ui.label(
                        egui::RichText::new("Aucun dump connu. Importes-en un.")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                } else {
                    ui.horizontal_wrapped(|ui| {
                        for chemin in &connus {
                            let nom = chemin
                                .file_stem()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let courant = self.load_path_input
                                == chemin.to_string_lossy().to_string();
                            if ui.selectable_label(courant, &nom).clicked() && !courant {
                                self.load_firmware(chemin.clone());
                            }
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button("Importer un dump...").clicked() {
                        if let Some(chemin) = rfd::FileDialog::new()
                            .add_filter("Dump de flash", &["bin", "rom", "dump", "raw"])
                            .set_title("Choisir un dump de flash Tamagotchi")
                            .pick_file()
                        {
                            // Le dump est recopie dans le dossier de donnees :
                            // il reste disponible meme si l'original bouge.
                            let range = crate::emulator::sauvegarde::adopter_firmware(&chemin);
                            self.load_firmware(range);
                        }
                    }
                    if ui.small_button("Ouvrir le dossier").clicked() {
                        let dossier = crate::emulator::sauvegarde::dossier_firmwares();
                        let _ = std::fs::create_dir_all(&dossier);
                        let _ = open_dossier(&dossier);
                    }
                });
                if self.machine.empreinte.is_some() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}, coque {}",
                            self.machine.edition.nom(),
                            self.shell_color.nom()
                        ))
                        .small(),
                    );
                }
            });

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Partie").strong());
                if self.machine.empreinte.is_none() {
                    ui.label(egui::RichText::new("Charge d'abord un dump.").small());
                } else {
                    ui.horizontal_wrapped(|ui| {
                        for nom in self.emplacements.clone() {
                            if ui
                                .selectable_label(self.emplacement_choisi == nom, &nom)
                                .clicked()
                            {
                                self.ouvrir_emplacement(nom);
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.nouvel_emplacement)
                                .hint_text("nouvelle partie")
                                .desired_width(180.0),
                        );
                        if ui.button("Creer").clicked() && !self.nouvel_emplacement.is_empty() {
                            let nom = self.nouvel_emplacement.clone();
                            self.nouvel_emplacement.clear();
                            self.ouvrir_emplacement(nom);
                        }
                    });
                    ui.label(
                        egui::RichText::new(
                            "La partie s'ecrit toute seule et vieillit en temps reel, meme ordinateur eteint.",
                        )
                        .small()
                        .color(egui::Color32::GRAY),
                    );
                }
            });

            ui.add_space(16.0);

            ui.vertical_centered(|ui| {
                let pret = self.machine.empreinte.is_some();
                if ui
                    .add_enabled(
                        pret,
                        egui::Button::new(egui::RichText::new("Jouer").size(20.0).strong())
                            .min_size(egui::vec2(220.0, 44.0)),
                    )
                    .clicked()
                {
                    self.mode = Mode::Jeu;
                }
                ui.add_space(6.0);
                if ui
                    .add(egui::Button::new("Inspection").min_size(egui::vec2(220.0, 30.0)))
                    .clicked()
                {
                    self.mode = Mode::Inspection;
                }
            });

            if let Some(msg) = &self.status_msg {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(msg)
                        .small()
                        .color(egui::Color32::from_rgb(220, 200, 90)),
                );
            }
        });
    }

    /// Mode jeu : la console seule, decoupee, deplacable sur le bureau.
    fn dessiner_jeu(&mut self, ctx: &Context) {
        CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let zone = ui.available_rect_before_wrap();

                // Le fond sert de poignee. Il est alloue avant la console pour
                // que les boutons et l'ecran gardent la priorite du pointeur :
                // egui donne la main au dernier element pose.
                let fond = ui.allocate_rect(zone, egui::Sense::drag());
                if fond.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                self.rafraichir_la_texture(ctx);
                self.dessiner_la_console(ctx, ui, zone);

                // Un seul bouton, minuscule, pose sur le bas de la coque : le
                // reste des commandes passe par le clic droit, comme partout
                // ailleurs sur le bureau. Sans barre de titre il faut bien que
                // la fermeture vive quelque part, et un menu vaut mieux qu'un
                // bouton de plus sur un objet qu'on veut voir nu.
                let cote = 15.0;
                let pastille = egui::Rect::from_center_size(
                    egui::pos2(zone.center().x, zone.max.y - cote * 1.6),
                    egui::vec2(cote, cote),
                );
                if ui
                    .put(
                        pastille,
                        egui::Button::new(egui::RichText::new("i").size(9.0))
                            .rounding(cote * 0.5)
                            .fill(egui::Color32::from_black_alpha(40)),
                    )
                    .on_hover_text("Inspection")
                    .clicked()
                {
                    self.mode = Mode::Inspection;
                }

                // Clic droit sur la coque : le menu de la fenetre.
                let mut mode_voulu = None;
                let mut console_voulue = None;
                let mut zoom_voulu = None;
                let mut point_voulu = None;
                let mut poser_un_point = false;
                let mut importer_un_point = false;
                let mut exporter_l_etat = false;
                let mut partie_voulue = None;
                let mut basculer_le_son = false;
                let mut basculer_le_dessus = false;
                let mut ouvrir_la_saisie = false;
                fond.context_menu(|ui| {
                    // Des sections qui se deplient sur place, et non des sous
                    // menus. La fenetre du mode jeu fait quatre cents pixels de
                    // large : un sous menu n'y tient pas a droite, egui le
                    // renvoie a gauche, et il recouvre le menu qui l'a ouvert.
                    ui.set_min_width(210.0);
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        egui::CollapsingHeader::new("Partie").show(ui, |ui| {
                            if self.machine.empreinte.is_none() {
                                ui.label(egui::RichText::new("Aucune console chargee.").small());
                            }
                            for nom in self.emplacements.clone() {
                                let courant = self.emplacement_choisi == nom;
                                if ui.selectable_label(courant, &nom).clicked() {
                                    if !courant {
                                        partie_voulue = Some(nom);
                                    }
                                    ui.close_menu();
                                }
                            }
                            if ui
                                .button("Nouvelle partie")
                                .on_hover_text("Cree une partie nommee toute seule")
                                .clicked()
                            {
                                partie_voulue = Some(self.nom_de_partie_libre());
                                ui.close_menu();
                            }
                            if ui
                                .button("Nouvelle sauvegarde...")
                                .on_hover_text("Cree une partie et demande son nom")
                                .clicked()
                            {
                                ouvrir_la_saisie = true;
                                ui.close_menu();
                            }
                        });

                        egui::CollapsingHeader::new("Console").show(ui, |ui| {
                            for chemin in crate::emulator::sauvegarde::firmwares_connus() {
                                let nom = chemin
                                    .file_stem()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                let courant =
                                    self.load_path_input == chemin.to_string_lossy().to_string();
                                if ui.selectable_label(courant, &nom).clicked() {
                                    if !courant {
                                        console_voulue = Some(chemin.clone());
                                    }
                                    ui.close_menu();
                                }
                            }
                        });

                        egui::CollapsingHeader::new("Ramener la console a...").show(ui, |ui| {
                            if self.reprises.points().is_empty() {
                                ui.label(
                                    egui::RichText::new("Aucun point pour l'instant.").small(),
                                );
                            }
                            // Du plus recent au plus ancien, et pas plus de dix :
                            // au dela la liste deviendrait illisible ici, et le
                            // panneau d'inspection les montre tous.
                            for (indice, point) in
                                self.reprises.points().iter().enumerate().rev().take(10)
                            {
                                let etiquette = format!(
                                    "{}   {}",
                                    point.quand.format("%H:%M"),
                                    point.age_lisible()
                                );
                                if ui.button(etiquette).clicked() {
                                    point_voulu = Some(indice);
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            if ui.button("Poser un point maintenant").clicked() {
                                poser_un_point = true;
                                ui.close_menu();
                            }
                            if ui.button("Importer un instantane...").clicked() {
                                importer_un_point = true;
                                ui.close_menu();
                            }
                            if ui.button("Exporter l'etat courant...").clicked() {
                                exporter_l_etat = true;
                                ui.close_menu();
                            }
                            if ui.button("Voir tous les points...").clicked() {
                                mode_voulu = Some(Mode::Inspection);
                                ui.close_menu();
                            }
                        });

                        egui::CollapsingHeader::new("Taille").show(ui, |ui| {
                            if ui.button("Agrandir de 25 %").clicked() {
                                zoom_voulu = Some((self.zoom_jeu * 1.25).min(3.0));
                                ui.close_menu();
                            }
                            if ui.button("Reduire de 25 %").clicked() {
                                zoom_voulu = Some((self.zoom_jeu / 1.25).max(0.5));
                                ui.close_menu();
                            }
                            if ui.button("Taille d'origine").clicked() {
                                zoom_voulu = Some(1.0);
                                ui.close_menu();
                            }
                        });

                        ui.separator();

                        if ui
                            .button(if self.audio.enabled {
                                "Couper le son"
                            } else {
                                "Remettre le son"
                            })
                            .clicked()
                        {
                            basculer_le_son = true;
                            ui.close_menu();
                        }
                        if ui
                            .button(if self.toujours_devant {
                                "Ne plus rester au dessus"
                            } else {
                                "Rester au dessus"
                            })
                            .clicked()
                        {
                            basculer_le_dessus = true;
                            ui.close_menu();
                        }

                        ui.separator();

                        if ui.button("Inspection").clicked() {
                            mode_voulu = Some(Mode::Inspection);
                            ui.close_menu();
                        }
                        if ui.button("Accueil").clicked() {
                            mode_voulu = Some(Mode::Accueil);
                            ui.close_menu();
                        }
                        if ui.button("Fermer").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
                if ouvrir_la_saisie {
                    self.saisie_sauvegarde = Some(String::new());
                }
                if let Some(nom) = partie_voulue {
                    self.ouvrir_emplacement(nom);
                }
                if basculer_le_dessus {
                    self.toujours_devant = !self.toujours_devant;
                    // Le niveau est pose au changement de mode : on force la
                    // reprise pour qu'il soit applique tout de suite.
                    self.mode_applique = None;
                }
                if basculer_le_son {
                    self.audio.enabled = !self.audio.enabled;
                    if !self.audio.enabled {
                        self.audio.silence_buzzer();
                    }
                    self.retenir_la_partie();
                }
                if let Some(indice) = point_voulu {
                    self.revenir_au_point(indice);
                }
                if poser_un_point {
                    self.poser_un_point();
                }
                if importer_un_point {
                    self.importer_un_point();
                }
                if exporter_l_etat {
                    self.exporter_l_etat();
                }
                if let Some(z) = zoom_voulu {
                    self.zoom_jeu = z;
                    self.retenir_la_partie();
                    // La taille est posee au changement de mode : on force la
                    // reprise pour qu'elle soit appliquee tout de suite.
                    self.mode_applique = None;
                }
                if let Some(chemin) = console_voulue {
                    self.load_firmware(chemin);
                }
                if let Some(m) = mode_voulu {
                    self.mode = m;
                }
            });
    }
}

impl eframe::App for TamagotchiApp {
    /// Derniere recopie avant de fermer la fenetre.
    ///
    /// L'ecriture periodique est espacee d'une seconde : sans ce dernier
    /// passage, la derniere sauvegarde du jeu pourrait rester en memoire.
    fn on_exit(&mut self) {
        let _ = self.machine.ecrire_sauvegarde();
        // Les reglages de son ont pu changer sans qu'on ouvre d'emplacement :
        // c'est ici qu'ils sont surs d'etre retenus.
        self.retenir_la_partie();
    }

    /// Fond de la fenetre. Transparent en mode jeu : c'est ce qui decoupe la
    /// coque sur le bureau, le reste de la surface ne peignant rien.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if self.mode == Mode::Jeu {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            visuals.panel_fill.to_normalized_gamma_f32()
        }
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
        //
        // La console ne doit rien entendre pendant qu'un menu ou un champ de
        // saisie est ouvert : taper un nom de partie appuyait sur A et sur C,
        // et derouler le menu du clic droit faisait tourner la molette.
        let interface_occupee = ctx.wants_keyboard_input()
            || self.saisie_sauvegarde.is_some()
            || ctx.memory(|m| m.any_popup_open())
            || ctx.is_pointer_over_area();
        if interface_occupee {
            // Les broches tenues sont relachees : sans cela un bouton reste
            // enfonce au moment ou le menu s'ouvre le resterait pour toujours.
            self.maintenus.clear();
            self.appliquer_entrees();
        }
        let key_f10 = !interface_occupee && ctx.input(|i| i.key_pressed(Key::F10));
        // Fleche haut tourne vers la droite, fleche bas vers la gauche, comme
        // la molette de la console.
        let molette = if interface_occupee {
            0
        } else {
            ctx.input(|i| {
                (i.key_pressed(Key::ArrowUp) as i32) - (i.key_pressed(Key::ArrowDown) as i32)
            })
        };
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
            if !interface_occupee && ctx.input(|i| keys.iter().any(|k| i.key_down(*k))) {
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
        if self.machine.is_running && self.vitesse > 0.0 {
            let debut = std::time::Instant::now();
            let limite = std::time::Duration::from_millis(self.budget_ms.max(1));
            // Une seconde de console vaut 96 millions de cycles : c'est ce que
            // le firmware declare en armant son SysTick a 95999 pour une
            // milliseconde. La dette suit donc le temps reel, multipliee par la
            // vitesse demandee.
            let par_seconde =
                crate::emulator::peripherals::snsys::CYCLES_PAR_SECONDE as f64;
            if self.vitesse.is_finite() {
                self.cycles_dus += par_seconde * self.vitesse as f64 * _dt as f64;
                // Au plus un quart de seconde de retard : au dela, on abandonne
                // le rattrapage plutot que de partir en trombe.
                self.cycles_dus = self.cycles_dus.min(par_seconde * 0.25);
            } else {
                self.cycles_dus = f64::INFINITY;
            }
            let depart = self.machine.cpu.cycles;
            while ((self.machine.cpu.cycles - depart) as f64) < self.cycles_dus
                && debut.elapsed() < limite
            {
                if !matches!(self.machine.run_frame(), crate::emulator::StepResult::Ok(_)) {
                    break;
                }
                // La melodie se suit ici, pas une fois par image : elle change
                // de note plusieurs fois en cent cinquante millisecondes, et un
                // releve par image n'en attraperait que des morceaux, dans le
                // desordre.
                let note = self.note_jouee();
                if (note - self.note_courante).abs() > 0.5 {
                    let duree = self.machine.cpu.cycles.saturating_sub(self.note_depuis);
                    self.notes.push((self.note_courante, duree));
                    self.note_courante = note;
                    self.note_depuis = self.machine.cpu.cycles;
                }
            }
            let faits = (self.machine.cpu.cycles - depart) as f64;
            self.cycles_dus = (self.cycles_dus - faits).max(0.0);
            self.historique.suivre(&self.machine);
            // Les points de reprise, eux, sont horodates et ecrits sur le
            // disque : un par minute, elagues avec l'age.
            self.reprises.suivre(&self.machine);
            // Sans cela l'interface ne se redessine qu'aux evenements, et
            // l'animation de la console s'arrete des qu'on lache la souris.
            ctx.request_repaint();
        }
        // La partie suit le jeu sur le disque : eteindre l'ordinateur ne coute
        // plus rien, la console retrouve son personnage au prochain lancement.
        self.tenir_la_sauvegarde();

        // Le buzzer de la console. On ne modelise pas le peripherique de sortie,
        // que le firmware n'atteint pas : on rend les frequences que son moteur
        // audio a calculees, en signal carre, ce qu'est un buzzer. La suite de
        // notes relevee pendant la tranche est rendue d'un bloc, a l'echelle du
        // temps reellement ecoule : l'ordre et les durees relatives sont donc
        // ceux de la console, meme quand l'emulation traine.
        let reste = self.machine.cpu.cycles.saturating_sub(self.note_depuis);
        if reste > 0 {
            self.notes.push((self.note_courante, reste));
            self.note_depuis = self.machine.cpu.cycles;
        }
        if self.notes.iter().any(|n| n.0 > 0.0) {
            self.audio.buzzer_notes(&self.notes, _dt.max(0.001));
            ctx.request_repaint();
        } else {
            self.audio.silence_buzzer();
        }
        self.notes.clear();

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

        if self.papier_a_relire {
            self.papier_a_relire = false;
            self.recharger_le_papier(ctx);
        }
        self.appliquer_le_mode(ctx);
        self.dessiner_la_saisie(ctx);
        match self.mode {
            Mode::Accueil => {
                self.dessiner_accueil(ctx);
                return;
            }
            Mode::Jeu => {
                self.dessiner_jeu(ctx);
                return;
            }
            Mode::Inspection => {}
        }

        // 3. Top Status & Menu Bar
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tamagotchi Paradise SNC73410 Emulator").strong());

                ui.separator();

                if ui.button("Retour au jeu").clicked() {
                    self.mode = Mode::Jeu;
                }
                if ui.button("Accueil").clicked() {
                    self.mode = Mode::Accueil;
                }
                self.menu_des_consoles(ui);
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
                ui.label("Hauteur :");
                for (nom, h) in [("/2", 0.5_f32), ("x1", 1.0), ("x2", 2.0), ("x4", 4.0)] {
                    if ui
                        .selectable_label((self.audio.hauteur - h).abs() < 0.01, nom)
                        .clicked()
                    {
                        self.audio.hauteur = h;
                    }
                }

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
                    ui.horizontal(|ui| {
                        for (onglet, nom) in [
                            (Onglet::Console, "Console"),
                            (Onglet::Personnalisation, "Personnalisation"),
                        ] {
                            if ui.selectable_label(self.onglet == onglet, nom).clicked() {
                                self.onglet = onglet;
                            }
                        }
                    });
                    ui.separator();

                    // L'habillage occupe plus de place que le panneau n'en a :
                    // il a son onglet, et il defile.
                    if self.onglet == Onglet::Personnalisation {
                        egui::ScrollArea::vertical().id_salt("personnalisation").show(
                            ui,
                            |ui| {
                                self.dessiner_l_habillage(ui, ctx);
                                ui.add_space(12.0);
                            },
                        );
                        return;
                    }

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
                            // Deux positions, et pas davantage. L'emulation
                            // tient tout juste le temps reel : au dessus, la
                            // machine donne deja tout et le reglage ne change
                            // rien. Le diagnostic affiche la vitesse atteinte,
                            // c'est le chiffre a regarder.
                            for (nom, v) in [("Pause", 0.0_f32), ("Temps reel", 1.0)] {
                                let choisi = if v.is_infinite() {
                                    self.vitesse.is_infinite()
                                } else {
                                    (self.vitesse - v).abs() < 0.01
                                };
                                if ui.selectable_label(choisi, nom).clicked() {
                                    self.vitesse = v;
                                    self.cycles_dus = 0.0;
                                }
                            }
                            // Ces deux la sont des outils de mise au point, pas
                            // des fonctions de jeu. Leurs anciens noms,
                            // « revenir en arriere » et « rejouer depuis le
                            // debut », se confondaient avec les points de
                            // reprise, qui eux ramenent la console a une heure.
                            if ui
                                .button("Annuler les 2 dernieres secondes")
                                .on_hover_text(
                                    "Filet de mise au point : revient a l'instantane                                      automatique precedent, pris toutes les deux secondes                                      d'emulation. Pour remonter plus loin, servez vous des                                      points de reprise.",
                                )
                                .clicked()
                            {
                                self.reculer();
                            }
                            if ui
                                .button("Rallumer la console")
                                .on_hover_text(
                                    "Recharge le dump et remet la console a son demarrage.                                      La partie sauvegardee n'est pas touchee : elle est                                      relue et le jeu reprend ou il en etait.",
                                )
                                .clicked()
                            {
                                let chemin = self.load_path_input.clone();
                                if !chemin.is_empty() {
                                    self.load_firmware(std::path::PathBuf::from(chemin));
                                }
                            }
                        });
                        // La sauvegarde et le chargement d'un etat vivaient aussi
                        // ici. Ils faisaient double emploi avec les points de
                        // reprise, qui les portent tous les deux et gardent en
                        // plus ce qui est importe.
                        ui.label(
                            egui::RichText::new(format!(
                                "{} instantanes automatiques",
                                self.historique.len()
                            ))
                            .small(),
                        );
                        match self.port_web {
                            Some(port) => {
                                ui.horizontal(|ui| {
                                    let adresse = format!("http://127.0.0.1:{}/", port);
                                    ui.hyperlink_to(
                                        egui::RichText::new(&adresse)
                                            .small()
                                            .color(egui::Color32::from_rgb(140, 220, 160)),
                                        &adresse,
                                    );
                                    if ui.small_button("Arreter").clicked() {
                                        if let Some(temoin) = self.serveur_actif.take() {
                                            temoin.store(
                                                false,
                                                std::sync::atomic::Ordering::Relaxed,
                                            );
                                        }
                                        self.port_web = None;
                                    }
                                });
                            }
                            None => {
                                // Eteint par defaut : il ne sert qu'a suivre
                                // l'emulation depuis un navigateur, et il coute
                                // une copie d'ecran a chaque image.
                                if ui.button("Demarrer le serveur local").clicked() {
                                    match crate::web::demarrer(
                                        std::sync::Arc::clone(&self.partage),
                                        7340,
                                    ) {
                                        Ok((port, temoin)) => {
                                            self.port_web = Some(port);
                                            self.serveur_actif = Some(temoin);
                                        }
                                        Err(e) => {
                                            self.status_msg =
                                                Some(format!("Serveur local : {}", e));
                                        }
                                    }
                                }
                                ui.label(
                                    egui::RichText::new(
                                        "Suivi dans le navigateur, eteint. Une fois allume,                                          il le reste jusqu'a la fermeture.",
                                    )
                                    .small(),
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

                    ui.separator();
                    ui.group(|ui| {
                        self.dessiner_les_reprises(ui);
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

            self.rafraichir_la_texture(ctx);
            self.dessiner_la_console(ctx, ui, available_rect);
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
