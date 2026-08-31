use eframe::egui;
use egui::{CentralPanel, Context, Key, SidePanel, TopBottomPanel};

// Le retour sonore de l'interface, clic de bouton et cran de molette, a ete
// retire : il se superposait aux tonalites du jeu et brouillait le seul son
// qui compte, celui que la console compose. `SoundEffect` reste dans
// `audio.rs` si le besoin revient, derriere un reglage.
use crate::audio::AudioEngine;
use crate::emulator::Machine;
use crate::gui::{ActiveModal, GuiWidgets, ShellColor};
use crate::hw_bridge::{FlashInspector, UartBridge};
use crate::i18n::{I18n, Language};
use crate::ui::{ConsolePanel, CpuPanel, DisasmPanel, LcdPanel, MemoryPanel};

/// Entete repliable du menu du clic droit, une seule section ouverte a la fois.
///
/// egui a bien un entete repliable, mais il garde son etat dans sa memoire :
/// referme le menu, rouvre le, la section est encore depliee. Celui ci suit
/// l'etat que l'application tient, et qu'elle remet a zero a la fermeture.
fn section<R>(
    ui: &mut egui::Ui,
    ouverte: &mut Option<u8>,
    numero: u8,
    titre: &str,
    contenu: impl FnOnce(&mut egui::Ui) -> R,
) {
    let depliee = *ouverte == Some(numero);
    let fleche = if depliee { "v" } else { ">" };
    if ui.button(format!("{}  {}", fleche, titre)).clicked() {
        *ouverte = if depliee { None } else { Some(numero) };
    }
    if depliee {
        ui.indent(("section", numero), |ui| {
            contenu(ui);
        });
    }
}

/// Ouvre le choix d'une image.
fn choisir_une_image(i18n: &I18n) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "bmp", "gif", "webp"])
        .set_title(i18n.choisir("Choisir une image", "Choose an image"))
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
    i18n: &I18n,
) -> (bool, bool) {
    let mut change = false;
    // Trois decimales et un pas fin : un curseur large de deux cents pixels
    // pour une course de trois ne permettait pas de poser une image au pixel
    // pres. La valeur reste saisissable au clavier, et les fleches la font
    // avancer d'un cran.
    change |= ui
        .add(
            egui::Slider::new(&mut cadrage.zoom, 0.1..=4.0)
                .text(format!("zoom {}", quoi))
                .fixed_decimals(3)
                .step_by(0.001),
        )
        .changed();
    change |= ui
        .add(
            egui::Slider::new(&mut cadrage.dx, -1.5..=1.5)
                .text(i18n.choisir("gauche / droite", "left / right"))
                .fixed_decimals(3)
                .step_by(0.001),
        )
        .changed();
    change |= ui
        .add(
            egui::Slider::new(&mut cadrage.dy, -1.5..=1.5)
                .text(i18n.choisir("haut / bas", "up / down"))
                .fixed_decimals(3)
                .step_by(0.001),
        )
        .changed();
    change |= ui
        .add(
            egui::Slider::new(&mut cadrage.rotation, -180.0..=180.0)
                .text("rotation")
                .fixed_decimals(2)
                .step_by(0.1),
        )
        .changed();
    if ui.button(format!("{} {}", i18n.choisir("Recentrer le", "Center"), quoi)).clicked() {
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
    /// Chargement du dump, console du firmware, vitesse et diagnostic.
    Console,
    /// Connexion serie, captures et console UART.
    Uart,
    /// Points de reprise et emplacements de sauvegarde.
    Sauvegardes,
    /// Registres, memoire, desassemblage. Ces panneaux coutent plus cher a
    /// dessiner que l'emulation n'en gagne a tourner : les mettre dans leur
    /// propre onglet suffit a ne les payer que quand on les regarde.
    Inspection,
    /// Habillage de la coque.
    Personnalisation,
    /// Mode d'emploi, et liens du projet.
    Aide,
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

/// Ce que la fenetre de saisie du nom doit faire une fois validee.
///
/// Le meme champ de texte sert aux deux : nommer un emplacement pour y ranger
/// la partie en cours, ou nommer la partie qu'on veut recommencer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ButDeLaSaisie {
    /// Attache l'etat en cours a un emplacement neuf.
    EnregistrerSous,
    /// Ouvre un emplacement neuf : la flash revient a l'image du dump et la
    /// console redemarre dessus.
    PartieNeuve,
}

pub struct TamagotchiApp {
    /// Ce que la fenetre montre.
    pub mode: Mode,
    /// Dernier mode applique a la fenetre, pour ne poser les commandes de
    /// viewport qu'au changement.
    mode_applique: Option<Mode>,
    pub machine: Machine,
    pub uart_bridge: UartBridge,
    pub audio: AudioEngine,
    pub i18n: I18n,
    pub shell_color: ShellColor,
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
    /// Section depliee du menu du clic droit, une seule a la fois.
    ///
    /// Elle est tenue ici et non par egui : l'etat d'un entete repliable
    /// d'egui survit a la fermeture du menu, et on le retrouvait ouvert au
    /// clic droit suivant. Elle est refermee des que le menu se ferme.
    section_menu: Option<u8>,
    /// Habillage de la console courante : papier, titre et vitre.
    pub fond: crate::gui::fond::Habillage,
    /// Texture du papier, papier et masque deja cuits ensemble.
    fond_texture: Option<egui::TextureHandle>,
    /// Texture du papier propre a la calotte.
    chapeau_texture: Option<egui::TextureHandle>,
    /// Texture du fond de coque.
    coque_texture: Option<egui::TextureHandle>,
    /// Nom en cours de saisie pour une nouvelle sauvegarde.
    ///
    /// `Some` tant que la fenetre de saisie est ouverte. Un menu contextuel ne
    /// peut pas porter de champ de texte utilisable : il se referme au premier
    /// clic ailleurs.
    saisie_sauvegarde: Option<String>,
    /// Ce que la fenetre de saisie fera du nom entre.
    but_de_la_saisie: ButDeLaSaisie,
    /// Emplacement dont la suppression attend une confirmation.
    suppression_demandee: Option<String>,
    /// Verification des mises a jour, a la demande.
    maj: crate::maj::Maj,
    /// Correspondance clavier, reglable et retenue.
    touches: crate::touches::Touches,
    /// Commande dont on attend la prochaine touche frappee.
    capture_touche: Option<crate::touches::Commande>,
    /// Ce que font les boutons de la souris sur l'ecran.
    souris: crate::touches::Souris,
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
    /// Table des scenes du firmware charge, pour nommer la scene courante dans
    /// le diagnostic. Elle ne se cherche qu'une fois la fenetre XIP programmee :
    /// avant cela les pointeurs de noms ne se ramenent a aucun offset.
    pub table_scenes: Option<crate::emulator::scenes::TableScenes>,
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
    /// Icone de zone de notification. Son absence n'empeche pas l'application
    /// de fonctionner sur un bureau qui ne fournit pas ce service.
    tray: Option<crate::tray::Tray>,
}

impl TamagotchiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let audio = AudioEngine::new();
        let i18n = I18n::default();
        let message_initial = i18n
            .choisir(
                "Capybara est pret, emulation materielle Paradise.",
                "Capybara is ready, Paradise hardware emulation.",
            )
            .to_string();
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
            uart_bridge: UartBridge::default(),
            audio,
            i18n,
            shell_color: ShellColor::BlueWater,
            hex_base_addr: 0x6001_1000,
            last_frame_time: std::time::Instant::now(),
            load_path_input: String::new(),
            status_msg: Some(message_initial),
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
            section_menu: None,
            fond: crate::gui::fond::Habillage::default(),
            fond_texture: None,
            chapeau_texture: None,
            coque_texture: None,
            papier_a_relire: true,
            saisie_sauvegarde: None,
            but_de_la_saisie: ButDeLaSaisie::EnregistrerSous,
            suppression_demandee: None,
            maj: crate::maj::Maj::default(),
            touches: crate::touches::Touches::default(),
            capture_touche: None,
            souris: crate::touches::Souris::default(),
            partage,
            port_web,
            serveur_actif: None,
            zoom_jeu: 1.0,
            toujours_devant: false,
            debit: 0.0,
            debit_depart: (0, std::time::Instant::now()),
            cout_ui: 0.0,
            table_scenes: None,
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
            tray: None,
        };
        // La console reprend ou elle en etait, comme un vrai boitier qu'on
        // rallume : le dump et l'emplacement du dernier lancement sont rouverts
        // sans rien demander.
        app.reprendre_la_derniere_partie();
        // L'icone de la zone de notification vient apres : son menu ne se
        // reetiquette pas, il lui faut la langue une fois relue.
        app.tray = crate::tray::Tray::new(&cc.egui_ctx, &app.i18n).ok();
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
                // Les dumps ont ete extraits pile faible. La flash vient
                // d'etre reecrite, donc le drapeau est revenu : sans cela le
                // firmware affiche son message de pile et ne va pas plus loin.
                self.machine.remplacer_la_pile();
                let tournait = self.machine.is_running;
                self.machine.reset();
                self.machine.is_running = tournait;
                self.historique.vider();
                self.status_msg = Some(format!("{} {}", self.i18n.choisir("Partie chargee :", "Game loaded:"), nom));
            }
            Ok(false) => {
                // La flash est revenue a l'image du dump : la console doit
                // repartir dessus, sinon elle continue sur l'etat en memoire de
                // la partie precedente et l'ecran ne bouge pas.
                // Meme raison : l'image du dump porte le drapeau de pile usee.
                self.machine.remplacer_la_pile();
                let tournait = self.machine.is_running;
                self.machine.reset();
                self.machine.is_running = tournait;
                self.historique.vider();
                self.status_msg = Some(format!("{} {}", self.i18n.choisir("Nouvelle partie", "New game"), nom));
            }
            Err(e) => {
                self.status_msg = Some(format!("{} : {}", self.i18n.choisir("Sauvegarde illisible", "Unreadable save"), e));
                return;
            }
        }
        self.emplacement_choisi = nom;
        crate::emulator::sauvegarde::retenir_emplacement(
            &empreinte,
            &self.emplacement_choisi,
        );
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

    /// Donne un nom a la partie courante sans recharger la flash ni remettre
    /// le processeur a zero. Si le nom existe deja, il conserve le comportement
    /// annonce par l'interface et ouvre cet emplacement.
    fn creer_emplacement_depuis_partie_courante(&mut self, nom: String) {
        let nom = nettoyer_nom(&nom);
        if nom.is_empty() {
            return;
        }
        let Some(empreinte) = self.machine.empreinte.clone() else {
            return;
        };
        let chemin = crate::emulator::sauvegarde::chemin(&empreinte, &nom);
        if chemin.exists() {
            self.ouvrir_emplacement(nom);
            return;
        }
        if let Err(e) = self.machine.creer_sauvegarde_depuis_etat(chemin) {
            self.status_msg = Some(format!("{} : {}", self.i18n.choisir("Sauvegarde non creee", "Save not created"), e));
            return;
        }
        self.emplacement_choisi = nom;
        crate::emulator::sauvegarde::retenir_emplacement(
            &empreinte,
            &self.emplacement_choisi,
        );
        self.rafraichir_emplacements();
        self.retenir_la_partie();
        self.reprises.ouvrir(crate::emulator::sauvegarde::dossier_reprises(
            &empreinte,
            &self.emplacement_choisi,
        ));
        self.status_msg = Some(format!(
            "{} {}",
            self.i18n.choisir("Partie courante enregistree dans", "Current game saved in"),
            self.emplacement_choisi
        ));
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
                langue: self.i18n.language().code().to_string(),
                touches: self.touches.clone(),
                souris: self.souris,
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
        self.touches = partie.touches.clone();
        self.souris = partie.souris;
        self.i18n.set_language(if partie.langue == "en" {
            Language::En
        } else {
            Language::Fr
        });

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
            self.status_msg = Some(format!("{} : {}", self.i18n.choisir("Sauvegarde non ecrite", "Save not written"), e));
        }
    }

    fn load_firmware(&mut self, path: std::path::PathBuf) {
        let nouveau_chemin = path.to_string_lossy().to_string();
        if self.load_path_input != nouveau_chemin {
            if let Err(e) = self.machine.ecrire_sauvegarde() {
                self.status_msg = Some(format!(
                    "{} : {}",
                    self.i18n.choisir(
                        "Changement de console annule, sauvegarde non ecrite",
                        "Console switch cancelled, save not written",
                    ),
                    e
                ));
                return;
            }
        }
        self.machine.console.clear();
        self.appuis.clear();
        self.phases_encodeur.clear();
        self.maintenus.clear();
        self.tenus_distants.clear();
        self.historique.vider();
        match self.machine.load_firmware_file(&path) {
            Ok(report) => {
                self.load_path_input = nouveau_chemin;
                self.ecran = None;
                self.table_scenes = None;
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
                    "{} {}, {} {}",
                    self.machine.edition.nom(),
                    self.i18n.choisir("charge", "loaded"),
                    self.i18n.choisir("coque", "shell"),
                    self.shell_color.nom()
                ));
                self.rafraichir_emplacements();
                let emplacement = self
                    .machine
                    .empreinte
                    .as_deref()
                    .and_then(crate::emulator::sauvegarde::dernier_emplacement)
                    .unwrap_or_else(|| {
                        crate::emulator::sauvegarde::EMPLACEMENT_PAR_DEFAUT.to_string()
                    });
                self.ouvrir_emplacement(emplacement);
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
        let reveil_demande = !self.appuis.is_empty()
            || !self.maintenus.is_empty()
            || !self.phases_encodeur.is_empty();
        if reveil_demande && self.machine.reveiller_par_broche() {
            // Le reset remet le compteur de cycles a zero. Les echeances des
            // impulsions appartenaient a l'ancien compteur et resteraient
            // sinon actives pendant une duree demesuree.
            self.appuis.clear();
            self.maintenus.clear();
            self.phases_encodeur.clear();
            for broche in Self::COMMANDES {
                self.machine.relacher(broche);
            }
            self.machine.relacher(Machine::ENCODEUR_1);
            self.machine.relacher(Machine::ENCODEUR_2);
            return;
        }
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
                self.status_msg = Some(format!("{} {} {}.", self.i18n.choisir("Retour a", "Restored at"), cycles, self.i18n.choisir("pas executes", "executed steps")));
            }
            None => {
                self.status_msg = Some(self.i18n.choisir("Aucun instantane a restaurer.", "No snapshot to restore.").to_string());
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
        partage.edition = self.machine.edition.nom().to_string();
        partage.sauvegarde = self.emplacement_choisi.clone();
        partage.langue = self.i18n.language().code().to_string();
        partage.en_marche = self.machine.is_running;
        partage.vitesse = (self.vitesse * 100.0).clamp(0.0, 400.0) as u32;
        partage.son = self.audio.enabled;
        partage.volume = (self.audio.volume * 100.0).clamp(0.0, 100.0) as u8;
        partage.titre = self.fond.titre.clone();
        let palette = self.shell_color.couleurs();
        partage.corps = [palette.corps.r(), palette.corps.g(), palette.corps.b()];
        partage.calotte = [palette.calotte.r(), palette.calotte.g(), palette.calotte.b()];
        partage.ombre = [palette.ombre.r(), palette.ombre.g(), palette.ombre.b()];
        partage.bouton = [palette.bouton.r(), palette.bouton.g(), palette.bouton.b()];
        partage.accent = [palette.accent.r(), palette.accent.g(), palette.accent.b()];
        partage.motif = [palette.motif.r(), palette.motif.g(), palette.motif.b()];
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
        // Le nom que le firmware donne lui meme a la scene. Un numero seul ne
        // dit rien, et il change d'une edition a l'autre : la table est lue
        // dans l'image chargee, jamais codee en dur.
        let nom_scene = |numero: u32| -> String {
            if numero == 0xFFFF {
                return "(aucune)".to_string();
            }
            match self.table_scenes.as_ref().and_then(|t| t.nom(numero as u16)) {
                Some(nom) => format!("({})", nom),
                None => String::new(),
            }
        };
        let console: String = self.machine.console.chars().rev().take(600).collect::<Vec<_>>()
            .into_iter().rev().collect();
        format!(
            "== diagnostic Capybara\n\
             firmware      {}\n\
             pas executes  {}   debit {:.1} millions par seconde\n\
             vitesse       demandee {}   atteinte {:.2} fois le temps reel\n\
             cout interface {:.1} ms par image\n\
             PC            {:#010x}   mode {}   PRIMASK {}\n\
             trames ecran  {}   instantanes {}\n\
             etat du jeu   courant {} {}   transition demandee {} {}\n\
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
            nom_scene(etat(0x1800_1BF4)),
            etat(0x1800_1BF6),
            nom_scene(etat(0x1800_1BF6)),
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
            coque: self.coque_texture.as_ref(),
            papier: self.fond_texture.as_ref(),
            chapeau: self.chapeau_texture.as_ref(),
            masque_impose: !self.fond.masque.is_empty(),
        };
        let commandes = LcdPanel::render(
            ui,
            zone,
            &self.machine.periph.display,
            self.ecran.as_ref(),
            self.shell_color,
            self.angle_molette,
            &habits,
            // L'animation suit la broche, pas le pointeur : un appui au clavier
            // enfonce le bouton dessine comme un clic dessus.
            crate::ui::lcd_panel::Enfonces {
                a: self.machine.broche_basse(Machine::BOUTON_A),
                b: self.machine.broche_basse(Machine::BOUTON_B),
                c: self.machine.broche_basse(Machine::BOUTON_C),
                molette: self.machine.broche_basse(Machine::BOUTON_MOLETTE),
            },
            self.souris,
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
        self.coque_texture = None;
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

        // Le fond de coque n'a pas de masque : c'est la forme de l'oeuf qui le
        // borne, et le masque sert a decouper le papier autour de l'ecran.
        self.coque_texture = charger(&self.fond.coque_fichier).map(|papier| {
            let image = fond::composer(&papier, &self.fond.coque_cadrage, None);
            ctx.load_texture("fond_coque", image, egui::TextureOptions::LINEAR)
        });
    }

    /// Habillage de la coque : papiers, masque, mot imprime, vitre, couleurs.
    ///
    /// Tout y est retenu par console, comme le papier de la vraie machine suit
    /// la coque et non le Tamagotchi.
    fn dessiner_l_habillage(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        // Le modele de coque ouvre la personnalisation : c'est le choix dont
        // tous les autres dependent, puisqu'il donne les couleurs de depart.
        // Il etait dans la barre du haut, ou il n'avait rien a voir avec le
        // reste et poussait la barre hors de l'ecran.
        ui.group(|ui| {
            ui.label(egui::RichText::new(self.i18n.choisir("Modele de coque", "Shell model")).strong());
            ui.horizontal_wrapped(|ui| {
                for coque in ShellColor::TOUTES {
                    if ui.selectable_label(self.shell_color == coque, coque.nom()).clicked() {
                        self.shell_color = coque;
                    }
                }
            });
            ui.label(
                egui::RichText::new(self.i18n.choisir(
                    "Donne les couleurs de depart. Chaque reglage ci dessous s'y substitue.",
                    "Provides the base colors. Each setting below overrides them.",
                ))
                    .small(),
            );
        });
        ui.add_space(8.0);

        ui.label(egui::RichText::new(self.i18n.choisir("Habillage de la coque", "Shell appearance")).strong());
        let Some(dossier) = self.dossier_du_papier() else {
            ui.label(egui::RichText::new(self.i18n.choisir("Charge une console d'abord.", "Load a console first.")).small());
            return;
        };
        let mut change = false;
        let mut recomposer = false;

        // --- le fond de coque
        ui.label(egui::RichText::new(self.i18n.choisir("Fond de la coque", "Shell background")).strong());
        ui.horizontal(|ui| {
            if ui
                .button(self.i18n.choisir("Image...", "Image..."))
                .on_hover_text(self.i18n.choisir("Couvre l'oeuf entier, derriere tout le reste", "Covers the whole egg behind every other layer"))
                .clicked()
            {
                if let Some(chemin) = choisir_une_image(&self.i18n) {
                    match crate::gui::fond::adopter_image(&chemin, &dossier, "coque") {
                        Ok(nom) => {
                            self.fond.coque_fichier = nom;
                            self.fond.coque_cadrage = Default::default();
                            change = true;
                            recomposer = true;
                        }
                        Err(e) => self.status_msg = Some(format!(
                            "{} : {}",
                            self.i18n.choisir("Image refusee", "Image rejected"),
                            e
                        )),
                    }
                }
            }
            if !self.fond.coque_fichier.is_empty() && ui.button(self.i18n.choisir("Retirer", "Remove")).clicked() {
                self.fond.retirer_le_fond(&dossier);
                recomposer = true;
            }
        });
        if !self.fond.coque_fichier.is_empty() {
            let (c, r) = cadrage_reglable(ui, &mut self.fond.coque_cadrage, self.i18n.choisir("fond", "background"), &self.i18n);
            change |= c;
            recomposer |= r;
            change |= ui
                .checkbox(&mut self.fond.inclut_le_chapeau, self.i18n.choisir("monte jusque sur le chapeau", "extends over the cap"))
                .changed();
        }

        ui.separator();

        // --- le papier autour de l'ecran
        ui.label(egui::RichText::new(self.i18n.choisir("Autour de l'ecran", "Around the screen")).strong());
        ui.horizontal(|ui| {
            if ui
                .button(self.i18n.choisir("Image...", "Image..."))
                .on_hover_text(
                    self.i18n.choisir(
                        "L'image se glisse sous la fenetre transparente, comme le papier imprime de la vraie console.",
                        "The image slides under the transparent window, like the printed paper of the real console.",
                    ),
                )
                .clicked()
            {
                if let Some(chemin) = choisir_une_image(&self.i18n) {
                    match crate::gui::fond::adopter_image(&chemin, &dossier, "fond") {
                        Ok(nom) => {
                            self.fond.fichier = nom;
                            self.fond.papier = Default::default();
                            change = true;
                            recomposer = true;
                        }
                        Err(e) => self.status_msg = Some(format!(
                            "{} : {}",
                            self.i18n.choisir("Image refusee", "Image rejected"),
                            e
                        )),
                    }
                }
            }
            if !self.fond.fichier.is_empty() && ui.button(self.i18n.choisir("Retirer", "Remove")).clicked() {
                self.fond.retirer_le_papier(&dossier);
                recomposer = true;
            }
        });
        if !self.fond.fichier.is_empty() {
            let (c, r) = cadrage_reglable(ui, &mut self.fond.papier, self.i18n.choisir("papier", "paper"), &self.i18n);
            change |= c;
            recomposer |= r;
        }

        // --- le masque, qui decoupe ce papier la
        ui.horizontal(|ui| {
            if ui
                .button(self.i18n.choisir("Masque...", "Mask..."))
                .on_hover_text(
                    self.i18n.choisir(
                        "Decoupe l'image autour de l'ecran. Noir et blanc : le noir laisse voir l'image, le blanc la cache, et ce qui est hors de l'image est cache aussi.",
                        "Cuts the image around the screen. Black and white: black shows the image, white hides it, and anything outside the mask is hidden too.",
                    ),
                )
                .clicked()
            {
                if let Some(chemin) = choisir_une_image(&self.i18n) {
                    match crate::gui::fond::adopter_image(&chemin, &dossier, "masque") {
                        Ok(nom) => {
                            self.fond.masque = nom;
                            self.fond.masque_cadrage = Default::default();
                            change = true;
                            recomposer = true;
                        }
                        Err(e) => self.status_msg = Some(format!(
                            "{} : {}",
                            self.i18n.choisir("Image refusee", "Image rejected"),
                            e
                        )),
                    }
                }
            }
            if !self.fond.masque.is_empty() && ui.button(self.i18n.choisir("Retirer", "Remove")).clicked() {
                self.fond.retirer_le_masque(&dossier);
                recomposer = true;
            }
        });
        if !self.fond.masque.is_empty() {
            let (c, r) = cadrage_reglable(ui, &mut self.fond.masque_cadrage, self.i18n.choisir("masque", "mask"), &self.i18n);
            change |= c;
            recomposer |= r;
        }

        // La fenetre elle meme, taille et position, independamment de l'ecran.
        change |= ui
            .add(egui::Slider::new(&mut self.fond.fenetre_taille, 0.3..=2.2).text(self.i18n.choisir("taille", "size")).fixed_decimals(3).step_by(0.001))
            .changed();
        change |= ui
            .add(egui::Slider::new(&mut self.fond.fenetre_dy, -0.4..=0.4).text(self.i18n.choisir("haut / bas", "up / down")).fixed_decimals(3).step_by(0.001))
            .changed();
        change |= ui
            .add(
                egui::Slider::new(&mut self.fond.fenetre_rotation, -180.0..=180.0)
                    .text(self.i18n.choisir("rotation du calque", "layer rotation"))
                    .fixed_decimals(2)
                    .step_by(0.1),
            )
            .changed();
        change |= ui
            .checkbox(&mut self.fond.fenetre_deborde, self.i18n.choisir("peut deborder de la coque", "may extend beyond the shell"))
            .changed();
        if ui.button(self.i18n.choisir("Fenetre d'origine", "Original window")).clicked() {
            self.fond.fenetre_taille = 1.0;
            self.fond.fenetre_dy = 0.0;
            change = true;
        }

        ui.separator();

        // --- le chapeau, quand il n'est pas couvert par le papier general
        if !(!self.fond.coque_fichier.is_empty() && self.fond.inclut_le_chapeau) {
            ui.label(egui::RichText::new(self.i18n.choisir("Chapeau", "Cap")).strong());
            ui.horizontal(|ui| {
                let mut teinte = self.fond.chapeau_couleur.is_some();
                if ui.checkbox(&mut teinte, self.i18n.choisir("couleur", "color")).changed() {
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
                    ui.label(egui::RichText::new(self.i18n.choisir("celle de la coque", "shell default")).small());
                }
            });
            ui.horizontal(|ui| {
                if ui.button(self.i18n.choisir("Image du chapeau...", "Cap image...")).clicked() {
                    if let Some(chemin) = choisir_une_image(&self.i18n) {
                        match crate::gui::fond::adopter_image(&chemin, &dossier, "chapeau") {
                            Ok(nom) => {
                                self.fond.chapeau_fichier = nom;
                                self.fond.chapeau_cadrage = Default::default();
                                change = true;
                                recomposer = true;
                            }
                            Err(e) => self.status_msg = Some(format!(
                                "{} : {}",
                                self.i18n.choisir("Image refusee", "Image rejected"),
                                e
                            )),
                        }
                    }
                }
                if !self.fond.chapeau_fichier.is_empty() && ui.button(self.i18n.choisir("Retirer", "Remove")).clicked() {
                    self.fond.retirer_le_chapeau(&dossier);
                    recomposer = true;
                }
            });
            if !self.fond.chapeau_fichier.is_empty() {
                let (c, r) = cadrage_reglable(ui, &mut self.fond.chapeau_cadrage, self.i18n.choisir("chapeau", "cap"), &self.i18n);
                change |= c;
                recomposer |= r;
            }
            ui.separator();
        }

        // --- le mot imprime au dessus de l'ecran
        change |= ui
            .checkbox(&mut self.fond.titre_visible, self.i18n.choisir("Mot imprime", "Printed title"))
            .changed();
        if self.fond.titre_visible {
            change |= ui
                .add(
                    egui::TextEdit::singleline(&mut self.fond.titre)
                        .hint_text("CAPYBARA")
                        .desired_width(180.0),
                )
                .changed();
            change |= ui
                .add(egui::Slider::new(&mut self.fond.titre_taille, 0.3..=3.0).text(self.i18n.choisir("taille", "size")).fixed_decimals(3).step_by(0.001))
                .changed();
            change |= ui
                .add(
                    egui::Slider::new(&mut self.fond.titre_dy, -0.6..=0.6)
                        .text(self.i18n.choisir("haut / bas", "up / down"))
                        .fixed_decimals(3)
                        .step_by(0.001),
                )
                .changed();
            ui.horizontal(|ui| {
                let mut choisie = self.fond.titre_couleur.is_some();
                if ui.checkbox(&mut choisie, self.i18n.choisir("couleur", "color")).changed() {
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
                    ui.label(egui::RichText::new(self.i18n.choisir("celle de la coque", "shell default")).small());
                }
                change |= ui
                    .add(
                        egui::DragValue::new(&mut self.fond.titre_opacite)
                            .speed(0.005)
                            .range(0.0..=1.0)
                            .fixed_decimals(2),
                    )
                    .on_hover_text(self.i18n.choisir("opacite", "opacity"))
                    .changed();
            });
        }

        ui.separator();

        // --- la vitre autour de la dalle
        change |= ui
            .checkbox(&mut self.fond.vitre_visible, self.i18n.choisir("Vitre autour de l'ecran", "Glass around the screen"))
            .changed();
        if self.fond.vitre_visible {
            change |= ui
                .add(
                    egui::Slider::new(&mut self.fond.vitre_epaisseur, 0.0..=0.10)
                        .text(self.i18n.choisir("epaisseur", "thickness"))
                        .fixed_decimals(4)
                        .step_by(0.0002),
                )
                .changed();
            change |= ui
                .color_edit_button_srgb(&mut self.fond.vitre_couleur)
                .changed();
        } else {
            ui.label(
                egui::RichText::new(self.i18n.choisir("Sans vitre, c'est la dalle qui prend l'arrondi.", "Without glass, the display itself gets rounded corners."))
                    .small()
                    .color(egui::Color32::GRAY),
            );
        }

        ui.separator();

        // --- la dalle
        ui.label(egui::RichText::new(self.i18n.choisir("Ecran", "Screen")).strong());
        change |= ui
            .add(egui::Slider::new(&mut self.fond.ecran_taille, 0.3..=2.0).text(self.i18n.choisir("taille", "size")).fixed_decimals(3).step_by(0.001))
            .changed();
        change |= ui
            .add(egui::Slider::new(&mut self.fond.ecran_dy, -0.4..=0.4).text(self.i18n.choisir("haut / bas", "up / down")).fixed_decimals(3).step_by(0.001))
            .changed();
        if ui.button(self.i18n.choisir("Ecran d'origine", "Original screen")).clicked() {
            self.fond.ecran_taille = 1.0;
            self.fond.ecran_dy = 0.0;
            change = true;
        }

        ui.separator();

        // --- les trois boutons
        ui.label(egui::RichText::new(self.i18n.choisir("Boutons", "Buttons")).strong());
        change |= ui
            .add(
                egui::Slider::new(&mut self.fond.boutons_dy, -0.4..=0.4)
                    .text(self.i18n.choisir("haut / bas", "up / down"))
                    .fixed_decimals(3)
                    .step_by(0.001),
            )
            .changed();
        change |= ui
            .add(
                egui::Slider::new(&mut self.fond.boutons_ecart, 0.2..=2.5)
                    .text(self.i18n.choisir("ecartement", "spacing"))
                    .fixed_decimals(3)
                    .step_by(0.001),
            )
            .changed();
        change |= ui
            .add(
                egui::Slider::new(&mut self.fond.boutons_taille, 0.3..=2.5)
                    .text(self.i18n.choisir("taille", "size"))
                    .fixed_decimals(3)
                    .step_by(0.001),
            )
            .changed();

        ui.separator();

        // --- le relief et les ombres
        ui.label(egui::RichText::new(self.i18n.choisir("Relief", "Depth")).strong());
        change |= ui
            .add(
                egui::Slider::new(&mut self.fond.relief_coque, -1.0..=1.0)
                    .text(self.i18n.choisir("relief de la coque", "shell depth"))
                    .fixed_decimals(3)
                    .step_by(0.001),
            )
            .on_hover_text(self.i18n.choisir(
                "Degrade et reflet, comme sur un plastique bombe. En negatif, la lumiere vient d'en bas.",
                "Gradient and highlight, like curved plastic. Negative lights it from below.",
            ))
            .changed();
        change |= ui
            .add(
                egui::Slider::new(&mut self.fond.ombre_fenetre, -0.25..=0.25)
                    .text(self.i18n.choisir("ombre du calque", "layer shadow"))
                    .fixed_decimals(3)
                    .step_by(0.001),
            )
            .changed();
        change |= ui
            .add(
                egui::Slider::new(&mut self.fond.ombre_ecran, -0.25..=0.25)
                    .text(self.i18n.choisir("ombre de l'ecran", "screen shadow"))
                    .fixed_decimals(3)
                    .step_by(0.001),
            )
            .changed();

        ui.separator();

        // --- les couleurs de la coque et des commandes
        ui.label(egui::RichText::new(self.i18n.choisir("Couleurs", "Colors")).strong());
        for (etiquette, defaut, champ) in [
            (self.i18n.choisir("corps", "body"), self.shell_color.couleurs().corps, 0usize),
            (self.i18n.choisir("autour de l'ecran", "around screen"), self.shell_color.couleurs().motif, 1),
            (self.i18n.choisir("traits", "outlines"), self.shell_color.couleurs().ombre, 2),
            (self.i18n.choisir("boutons", "buttons"), self.shell_color.couleurs().bouton, 3),
            (self.i18n.choisir("molette", "wheel"), self.shell_color.couleurs().accent, 4),
        ] {
            ui.horizontal(|ui| {
                let actuel = match champ {
                    0 => &mut self.fond.corps_couleur,
                    1 => &mut self.fond.motif_couleur,
                    2 => &mut self.fond.bordure_couleur,
                    3 => &mut self.fond.bouton_couleur,
                    _ => &mut self.fond.molette_couleur,
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
                    ui.label(egui::RichText::new(self.i18n.choisir("celle de la coque", "shell default")).small());
                }
                // L'opacite vit a part de la couleur : une piece qui suit
                // l'edition peut quand meme devenir translucide.
                let opacite = match champ {
                    0 => &mut self.fond.corps_opacite,
                    1 => &mut self.fond.motif_opacite,
                    2 => &mut self.fond.bordure_opacite,
                    3 => &mut self.fond.bouton_opacite,
                    _ => &mut self.fond.molette_opacite,
                };
                change |= ui
                    .add(
                        egui::DragValue::new(opacite)
                            .speed(0.005)
                            .range(0.0..=1.0)
                            .fixed_decimals(2),
                    )
                    .on_hover_text(self.i18n.choisir("opacite", "opacity"))
                    .changed();
            });
        }

        ui.separator();
        if ui
            .button(self.i18n.choisir("Tout remettre par defaut", "Restore all defaults"))
            .on_hover_text(
                self.i18n.choisir(
                    "Rend a la coque son apparence d'origine. Les images importees sont effacees.",
                    "Gives the shell back its original look. Imported images are erased.",
                ),
            )
            .clicked()
        {
            self.fond.retirer_le_fond(&dossier);
            self.fond.retirer_le_papier(&dossier);
            self.fond.retirer_le_masque(&dossier);
            self.fond.retirer_le_chapeau(&dossier);
            self.fond = crate::gui::fond::Habillage::default();
            change = true;
            recomposer = true;
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
    /// Fenetre de confirmation avant d'effacer un emplacement.
    ///
    /// Une sauvegarde effacee ne se recupere pas : elle merite une question,
    /// posee avec le nom sous les yeux.
    fn dessiner_la_suppression(&mut self, ctx: &Context) {
        let Some(nom) = self.suppression_demandee.clone() else {
            return;
        };
        let mut ouverte = true;
        let mut effacer = false;
        let mut annuler = false;
        egui::Window::new(self.i18n.choisir("Supprimer cette sauvegarde", "Delete this save"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut ouverte)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(&nom).strong());
                ui.label(
                    egui::RichText::new(self.i18n.choisir(
                        "La partie et tous ses points de reprise seront effaces. C'est definitif.",
                        "The game and all its recovery points will be erased. This cannot be undone.",
                    ))
                    .small(),
                );
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            egui::RichText::new(self.i18n.choisir("Supprimer", "Delete"))
                                .color(egui::Color32::from_rgb(240, 140, 130)),
                        )
                        .clicked()
                    {
                        effacer = true;
                    }
                    if ui.button(self.i18n.choisir("Annuler", "Cancel")).clicked() {
                        annuler = true;
                    }
                });
            });

        if effacer {
            self.suppression_demandee = None;
            self.supprimer_l_emplacement(nom);
        } else if annuler || !ouverte {
            self.suppression_demandee = None;
        }
    }

    /// Efface un emplacement, apres confirmation.
    fn supprimer_l_emplacement(&mut self, nom: String) {
        let Some(empreinte) = self.machine.empreinte.clone() else {
            return;
        };
        // On cesse d'ecrire dedans avant d'effacer : sinon la prochaine
        // sauvegarde automatique le recreerait dans la foulee. La console, elle,
        // continue de tourner : effacer un fichier n'est pas une raison de
        // couper la partie en cours sous les doigts de qui joue. Seule une
        // nouvelle partie redemarre, et elle demande un nom avant.
        let c_etait_la_partie_ouverte = self.emplacement_choisi == nom;
        if c_etait_la_partie_ouverte {
            self.machine.fermer_sauvegarde();
            self.reprises.fermer();
            self.emplacement_choisi.clear();
        }
        if let Err(e) = crate::emulator::sauvegarde::supprimer_emplacement(&empreinte, &nom) {
            self.status_msg = Some(format!(
                "{} : {}",
                self.i18n.choisir("Suppression impossible", "Could not delete"),
                e
            ));
            return;
        }
        self.rafraichir_emplacements();
        self.status_msg = Some(if c_etait_la_partie_ouverte {
            format!(
                "{} {} {}",
                self.i18n.choisir("Sauvegarde supprimee :", "Save deleted:"),
                nom,
                self.i18n.choisir(
                    ". La console continue de tourner mais plus rien ne l'enregistre : ouvrez un emplacement ou choisissez Nouvelle partie.",
                    ". The console keeps running but nothing is being saved: open a slot or choose New game.",
                )
            )
        } else {
            format!(
                "{} {}",
                self.i18n.choisir("Sauvegarde supprimee :", "Save deleted:"),
                nom
            )
        });
    }

    fn dessiner_la_saisie(&mut self, ctx: &Context) {
        let Some(mut nom) = self.saisie_sauvegarde.clone() else {
            return;
        };
        let mut ouverte = true;
        let mut valider = false;
        let mut annuler = false;
        let titre = match self.but_de_la_saisie {
            ButDeLaSaisie::EnregistrerSous => self.i18n.choisir("Nouvelle sauvegarde", "Save as"),
            ButDeLaSaisie::PartieNeuve => self.i18n.choisir("Nouvelle partie", "New game"),
        };
        egui::Window::new(titre)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut ouverte)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(self.i18n.choisir("Nom de la partie", "Game name")).small());
                let champ = ui.add(
                    egui::TextEdit::singleline(&mut nom)
                        .hint_text(self.i18n.choisir("ma partie", "my game"))
                        .desired_width(200.0),
                );
                champ.request_focus();
                if champ.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    valider = true;
                }
                if self.but_de_la_saisie == ButDeLaSaisie::PartieNeuve {
                    ui.label(
                        egui::RichText::new(self.i18n.choisir(
                            "La console repart a neuf et la partie s'enregistrera ici.",
                            "The console starts over and the game will be saved here.",
                        ))
                        .small(),
                    );
                }
                let existe = self.emplacements.iter().any(|e| *e == nettoyer_nom(&nom));
                if existe {
                    ui.label(
                        egui::RichText::new(self.i18n.choisir(
                            "Ce nom existe deja, il sera ouvert tel quel.",
                            "This name already exists and will be opened as-is.",
                        ))
                            .small()
                            .color(egui::Color32::from_rgb(220, 200, 90)),
                    );
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !nettoyer_nom(&nom).is_empty(),
                            egui::Button::new(self.i18n.choisir("Creer", "Create")),
                        )
                        .clicked()
                    {
                        valider = true;
                    }
                    if ui.button(self.i18n.choisir("Annuler", "Cancel")).clicked() {
                        annuler = true;
                    }
                });
            });

        if valider {
            let propre = nettoyer_nom(&nom);
            self.saisie_sauvegarde = None;
            if !propre.is_empty() {
                match self.but_de_la_saisie {
                    // Attacher l'etat en cours a un emplacement neuf.
                    ButDeLaSaisie::EnregistrerSous => {
                        self.creer_emplacement_depuis_partie_courante(propre)
                    }
                    // Ouvrir un emplacement neuf remet la flash a l'image du
                    // dump et redemarre la console : c'est deja une partie
                    // neuve, il n'y a rien a fermer ni a recharger.
                    ButDeLaSaisie::PartieNeuve => self.ouvrir_emplacement(propre),
                }
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
            self.status_msg = Some(self.i18n.choisir("Point de reprise pose.", "Recovery point created.").to_string());
        } else {
            self.status_msg = Some(self.i18n.choisir("Ouvre une partie avant de poser un point.", "Open a game before creating a recovery point.").to_string());
        }
    }

    /// Adopte un instantane venu d'ailleurs.
    fn importer_un_point(&mut self) {
        let Some(chemin) = rfd::FileDialog::new()
            .add_filter("Instantane", &["tamastate"])
            .set_title(self.i18n.choisir("Importer un instantane", "Import a snapshot"))
            .pick_file()
        else {
            return;
        };
        match self.reprises.adopter(&chemin) {
            Ok(()) => self.status_msg = Some(self.i18n.choisir("Instantane importe.", "Snapshot imported.").to_string()),
            Err(e) => self.status_msg = Some(format!("{} : {}", self.i18n.choisir("Instantane refuse", "Snapshot rejected"), e)),
        }
    }

    /// Ecrit l'etat courant de la console dans un fichier choisi.
    ///
    /// Un instantane ne porte que les pages de flash modifiees : il ne veut
    /// rien dire sans son dump, et c'est pour cela qu'il retient le chemin de
    /// celui ci.
    fn exporter_l_etat(&mut self) {
        if self.machine.empreinte.is_none() {
            self.status_msg = Some(self.i18n.choisir("Charge une console avant d'exporter.", "Load a console before exporting.").to_string());
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
            .set_title(self.i18n.choisir("Exporter l'etat de la console", "Export console state"))
            .save_file()
        else {
            return;
        };
        match self.machine.instantane().ecrire(&chemin) {
            Ok(()) => self.status_msg = Some(self.i18n.choisir("Etat exporte.", "State exported.").to_string()),
            Err(e) => self.status_msg = Some(format!("{} : {}", self.i18n.choisir("Export impossible", "Export failed"), e)),
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
            .set_title(self.i18n.choisir("Exporter ce point de reprise", "Export this recovery point"))
            .save_file()
        else {
            return;
        };
        match std::fs::copy(&source, &cible) {
            Ok(_) => self.status_msg = Some(self.i18n.choisir("Point exporte.", "Recovery point exported.").to_string()),
            Err(e) => self.status_msg = Some(format!("{} : {}", self.i18n.choisir("Export impossible", "Export failed"), e)),
        }
    }

    /// Restaure un point de reprise et remet les commandes au repos.
    fn revenir_au_point(&mut self, indice: usize) {
        let Some(etat) = self.reprises.restaurer(indice) else {
            self.status_msg = Some(self.i18n.choisir("Point de reprise illisible.", "Unreadable recovery point.").to_string());
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
        self.status_msg = Some(format!("{} {}.", self.i18n.choisir("Console revenue a", "Console restored to"), quand));
    }

    /// Liste des points de reprise, avec l'heure et l'age de chacun.
    fn dessiner_les_reprises(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(self.i18n.choisir("Points de reprise", "Recovery points")).strong());
            ui.label(
                egui::RichText::new(format!("{} points", self.reprises.points().len())).small(),
            );
        });
        if !self.reprises.actif() {
            ui.label(egui::RichText::new(self.i18n.choisir("Ouvre une partie pour en garder.", "Open a game to keep recovery points.")).small());
            return;
        }
        ui.horizontal(|ui| {
            if ui.button(self.i18n.choisir("Poser un point", "Create recovery point")).clicked() {
                self.poser_un_point();
            }
            if ui
                .button(self.i18n.choisir("Importer...", "Import..."))
                .on_hover_text(self.i18n.choisir("Ajoute un fichier .tamastate a la liste ci dessous", "Adds a .tamastate file to the list below"))
                .clicked()
            {
                self.importer_un_point();
            }
            if ui.button(self.i18n.choisir("Exporter l'etat", "Export state")).clicked() {
                self.exporter_l_etat();
            }
        });
        if self.reprises.points().is_empty() {
            ui.label(
                egui::RichText::new(self.i18n.choisir("Le premier point est pris apres une minute de jeu.", "The first recovery point is created after one minute of play."))
                    .small()
                    .color(egui::Color32::GRAY),
            );
            return;
        }
        ui.label(
            egui::RichText::new(self.i18n.choisir(
                "Cliquez sur une heure pour y ramener la console. Un point est pris chaque minute et garde jusqu'a douze heures.",
                "Click a time to restore the console. A point is created every minute and kept for up to twelve hours.",
            ))
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
                            .on_hover_text(self.i18n.choisir("Ramener la console a cet instant", "Restore the console to this point"))
                            .clicked()
                        {
                            a_restaurer = Some(indice);
                        }
                        let age = if self.i18n.language() == Language::En {
                            point.age_lisible_en()
                        } else {
                            point.age_lisible()
                        };
                        ui.label(egui::RichText::new(age).small());
                        ui.label(
                            egui::RichText::new(point.quand.format("%d/%m").to_string())
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        if ui
                            .small_button("^")
                            .on_hover_text(self.i18n.choisir("Exporter ce point vers un fichier", "Export this point to a file"))
                            .clicked()
                        {
                            a_exporter = Some(indice);
                        }
                        if ui.small_button("x").on_hover_text(self.i18n.choisir("Effacer ce point", "Delete this point")).clicked() {
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
            .unwrap_or_else(|| self.i18n.choisir("aucune", "none").to_string());
        let mut voulue = None;
        ui.menu_button(format!("Console : {}", courant), |ui| {
            if connus.is_empty() {
                ui.label(egui::RichText::new(self.i18n.choisir(
                    "Aucun dump dans le dossier de donnees.",
                    "No dump in the data folder.",
                )).small());
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
            if ui.button(self.i18n.choisir("Importer un dump...", "Import a dump...")).clicked() {
                if let Some(chemin) = rfd::FileDialog::new()
                    .add_filter("Dump de flash", &["bin", "rom", "dump", "raw"])
                    .set_title(self.i18n.choisir("Choisir un dump de flash", "Choose a flash dump"))
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

    fn dessiner_langue(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(self.i18n.choisir("Langue :", "Language:"));
            for (langue, nom) in [(Language::Fr, "FR"), (Language::En, "EN")] {
                if ui
                    .selectable_label(self.i18n.language() == langue, nom)
                    .clicked()
                {
                    self.i18n.set_language(langue);
                    self.retenir_la_partie();
                }
            }
        });
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
            self.dessiner_langue(ui);
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Capybara").size(28.0).strong());
                ui.label(
                    egui::RichText::new(self.i18n.choisir(
                        "emulateur du SoC Sonix SNC7340",
                        "Sonix SNC7340 SoC emulator",
                    ))
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
                        egui::RichText::new(self.i18n.choisir(
                            "Aucun dump connu. Importes-en un.",
                            "No known dump. Import one.",
                        ))
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
                    if ui.button(self.i18n.choisir("Importer un dump...", "Import a dump...")).clicked() {
                        if let Some(chemin) = rfd::FileDialog::new()
                            .add_filter("Dump de flash", &["bin", "rom", "dump", "raw"])
                            .set_title(self.i18n.choisir("Choisir un dump de flash", "Choose a flash dump"))
                            .pick_file()
                        {
                            // Le dump est recopie dans le dossier de donnees :
                            // il reste disponible meme si l'original bouge.
                            let range = crate::emulator::sauvegarde::adopter_firmware(&chemin);
                            self.load_firmware(range);
                        }
                    }
                    if ui.small_button(self.i18n.choisir("Ouvrir le dossier", "Open folder")).clicked() {
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
                ui.label(egui::RichText::new(self.i18n.choisir("Partie", "Game")).strong());
                if self.machine.empreinte.is_none() {
                    ui.label(egui::RichText::new(self.i18n.choisir("Charge d'abord un dump.", "Load a dump first.")).small());
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
                                .hint_text(self.i18n.choisir("nouvelle partie", "new game"))
                                .desired_width(180.0),
                        );
                        if ui.button(self.i18n.choisir("Creer", "Create")).clicked() && !self.nouvel_emplacement.is_empty() {
                            let nom = self.nouvel_emplacement.clone();
                            self.nouvel_emplacement.clear();
                            self.creer_emplacement_depuis_partie_courante(nom);
                        }
                    });
                    ui.label(
                        egui::RichText::new(
                            self.i18n.choisir(
                                "La partie s'ecrit toute seule et vieillit en temps reel, meme ordinateur eteint.",
                                "The game saves automatically and keeps aging in real time while the computer is off.",
                            ),
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
                        egui::Button::new(egui::RichText::new(self.i18n.choisir("Jouer", "Play")).size(20.0).strong())
                            .min_size(egui::vec2(220.0, 44.0)),
                    )
                    .clicked()
                {
                    self.mode = Mode::Jeu;
                }
                ui.add_space(6.0);
                if ui
                    .add(egui::Button::new(self.i18n.choisir("Reglages", "Settings")).min_size(egui::vec2(220.0, 30.0)))
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
                if fond.secondary_clicked() {
                    self.uart_bridge.refresh_ports();
                }

                self.rafraichir_la_texture(ctx);
                self.dessiner_la_console(ctx, ui, zone);

                // Clic droit sur la coque : le menu de la fenetre.
                let mut mode_voulu = None;
                // Sortie du champ pour la duree du menu : la fermeture qui
                // suit ne peut pas emprunter `self` deux fois.
                let mut section_ouverte = self.section_menu;
                // Vrai tant que le menu est dessine. C'est le seul temoin sur
                // par lequel savoir qu'il est ouvert : `any_popup_open` ne
                // compte pas les menus contextuels, s'y fier remettait la
                // section a zero a chaque image et rien ne se depliait plus.
                let mut menu_dessine = false;
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
                let mut port_uart_voulu = None;
                let mut deconnecter_uart = false;
                let mut langue_voulue = None;
                fond.context_menu(|ui| {
                    menu_dessine = true;
                    // Des sections qui se deplient sur place, et non des sous
                    // menus. La fenetre du mode jeu fait quatre cents pixels de
                    // large : un sous menu n'y tient pas a droite, egui le
                    // renvoie a gauche, et il recouvre le menu qui l'a ouvert.
                    ui.set_min_width(210.0);
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        section(ui, &mut section_ouverte, 0, self.i18n.choisir("Partie", "Game"), |ui| {
                            if self.machine.empreinte.is_none() {
                                ui.label(egui::RichText::new(self.i18n.choisir("Aucune console chargee.", "No console loaded.")).small());
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
                                .button(self.i18n.choisir("Nouvelle partie", "New game"))
                                .on_hover_text(self.i18n.choisir("Cree une partie nommee toute seule", "Starts a fresh game with an automatic name"))
                                .clicked()
                            {
                                partie_voulue = Some(self.nom_de_partie_libre());
                                ui.close_menu();
                            }
                            if ui
                                .button(self.i18n.choisir("Nouvelle sauvegarde...", "Save as..."))
                                .on_hover_text(self.i18n.choisir("Enregistre la partie courante sous un nouveau nom", "Saves the current game under a new name"))
                                .clicked()
                            {
                                ouvrir_la_saisie = true;
                                ui.close_menu();
                            }
                        });

                        section(ui, &mut section_ouverte, 1, "Console", |ui| {
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

                        section(ui, &mut section_ouverte, 4, "UART", |ui| {
                            if self.uart_bridge.available_ports.is_empty() {
                                ui.label(
                                    egui::RichText::new(self.i18n.choisir("Aucun port COM detecte.", "No COM port detected.")).small(),
                                );
                            }
                            for port in self.uart_bridge.available_ports.clone() {
                                let courant = self.uart_bridge.is_connected
                                    && self.uart_bridge.port_name == port;
                                if ui.selectable_label(courant, &port).clicked() {
                                    if !courant {
                                        port_uart_voulu = Some(port);
                                    }
                                    ui.close_menu();
                                }
                            }
                            if ui.button(self.i18n.choisir("Actualiser les ports", "Refresh ports")).clicked() {
                                self.uart_bridge.refresh_ports();
                            }
                            if self.uart_bridge.is_connected {
                                ui.separator();
                                if ui.button(self.i18n.choisir("Deconnecter", "Disconnect")).clicked() {
                                    deconnecter_uart = true;
                                    ui.close_menu();
                                }
                            }
                        });

                        section(ui, &mut section_ouverte, 2, self.i18n.choisir("Ramener la console a...", "Restore console to..."), |ui| {
                            if self.reprises.points().is_empty() {
                                ui.label(
                                    egui::RichText::new(self.i18n.choisir("Aucun point pour l'instant.", "No recovery point yet.")).small(),
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
                                    if self.i18n.language() == Language::En {
                                        point.age_lisible_en()
                                    } else {
                                        point.age_lisible()
                                    }
                                );
                                if ui.button(etiquette).clicked() {
                                    point_voulu = Some(indice);
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            if ui.button(self.i18n.choisir("Poser un point maintenant", "Create recovery point now")).clicked() {
                                poser_un_point = true;
                                ui.close_menu();
                            }
                            if ui.button(self.i18n.choisir("Importer un instantane...", "Import snapshot...")).clicked() {
                                importer_un_point = true;
                                ui.close_menu();
                            }
                            if ui.button(self.i18n.choisir("Exporter l'etat courant...", "Export current state...")).clicked() {
                                exporter_l_etat = true;
                                ui.close_menu();
                            }
                            if ui.button(self.i18n.choisir("Voir tous les points...", "View all recovery points...")).clicked() {
                                mode_voulu = Some(Mode::Inspection);
                                ui.close_menu();
                            }
                        });

                        section(ui, &mut section_ouverte, 3, self.i18n.choisir("Taille", "Size"), |ui| {
                            if ui.button(self.i18n.choisir("Agrandir de 25 %", "Increase by 25%" )).clicked() {
                                zoom_voulu = Some((self.zoom_jeu * 1.25).min(3.0));
                                ui.close_menu();
                            }
                            if ui.button(self.i18n.choisir("Reduire de 25 %", "Decrease by 25%" )).clicked() {
                                zoom_voulu = Some((self.zoom_jeu / 1.25).max(0.5));
                                ui.close_menu();
                            }
                            if ui.button(self.i18n.choisir("Taille d'origine", "Original size")).clicked() {
                                zoom_voulu = Some(1.0);
                                ui.close_menu();
                            }
                        });

                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label(self.i18n.choisir("Langue :", "Language:"));
                            for (langue, nom) in [(Language::Fr, "FR"), (Language::En, "EN")] {
                                if ui.selectable_label(self.i18n.language() == langue, nom).clicked() {
                                    langue_voulue = Some(langue);
                                    ui.close_menu();
                                }
                            }
                        });

                        ui.separator();

                        if ui
                            .button(if self.audio.enabled {
                                self.i18n.choisir("Couper le son", "Mute")
                            } else {
                                self.i18n.choisir("Remettre le son", "Unmute")
                            })
                            .clicked()
                        {
                            basculer_le_son = true;
                            ui.close_menu();
                        }
                        if ui
                            .button(if self.toujours_devant {
                                self.i18n.choisir("Ne plus rester au dessus", "Stop staying on top")
                            } else {
                                self.i18n.choisir("Rester au dessus", "Stay on top")
                            })
                            .clicked()
                        {
                            basculer_le_dessus = true;
                            ui.close_menu();
                        }

                        ui.separator();

                        if ui.button(self.i18n.choisir("Reglages", "Settings")).clicked() {
                            mode_voulu = Some(Mode::Inspection);
                            ui.close_menu();
                        }
                        if ui.button(self.i18n.choisir("Accueil", "Home")).clicked() {
                            mode_voulu = Some(Mode::Accueil);
                            ui.close_menu();
                        }
                        if ui.button(self.i18n.choisir("Reduire", "Minimise")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            ui.close_menu();
                        }
                        if ui.button(self.i18n.choisir("Fermer", "Close")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
                if ouvrir_la_saisie {
                    self.but_de_la_saisie = ButDeLaSaisie::EnregistrerSous;
                    self.saisie_sauvegarde = Some(String::new());
                }
                // Menu referme : la prochaine ouverture repart repliee.
                self.section_menu = if menu_dessine { section_ouverte } else { None };
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
                if let Some(langue) = langue_voulue {
                    self.i18n.set_language(langue);
                    self.retenir_la_partie();
                }
                if deconnecter_uart {
                    self.uart_bridge.disconnect();
                }
                if let Some(port) = port_uart_voulu {
                    self.machine.periph.uart.vider_la_ligne();
                    if let Err(e) = self.uart_bridge.connect(&port) {
                        self.status_msg = Some(format!("UART : {e}"));
                    }
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
        self.uart_bridge.disconnect();
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
        if let Some(action) = self.tray.as_ref().and_then(|tray| tray.action()) {
            match action {
                crate::tray::ActionTray::Afficher => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                crate::tray::ActionTray::Reduire => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                crate::tray::ActionTray::Quitter => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        // Fermer ferme. La fenetre se cachait ici au lieu de se fermer, en
        // comptant sur l'icone de la zone de notification pour la faire
        // revenir ou quitter. Mais une fenetre invisible ne recoit plus de
        // demandes de dessin : cette methode cessait d'etre appelee, le choix
        // du menu de l'icone n'etait jamais relu, et l'application devenait
        // infermable. L'icone reste, elle met la fenetre au premier plan ou
        // quitte, et la fenetre ne disparait plus sous le tapis.
        let diagnostic_uart = self.mode == Mode::Inspection && self.onglet == Onglet::Uart;
        self.machine.regler_diagnostic_uart(diagnostic_uart);
        self.uart_bridge.regler_diagnostic(diagnostic_uart);
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
        let mut interface_occupee = ctx.wants_keyboard_input()
            || self.saisie_sauvegarde.is_some()
            || self.suppression_demandee.is_some()
            || ctx.memory(|m| m.any_popup_open())
            || ctx.is_pointer_over_area();
        if interface_occupee {
            // Les broches tenues sont relachees : sans cela un bouton reste
            // enfonce au moment ou le menu s'ouvre le resterait pour toujours.
            self.maintenus.clear();
            self.appliquer_entrees();
        }
        // Une touche attendue pour un remappage est prise ici, avant que le
        // reste ne la lise : sans cela, la frappe qui choisit le bouton A
        // appuierait aussi dessus.
        if let Some(commande) = self.capture_touche {
            let frappee = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key { key, pressed: true, .. } => Some(*key),
                    _ => None,
                })
            });
            if let Some(touche) = frappee {
                self.capture_touche = None;
                if touche != Key::Escape {
                    self.touches.ajouter(commande, touche);
                    self.retenir_la_partie();
                }
            }
            interface_occupee = true;
        }
        let key_f10 = !interface_occupee && ctx.input(|i| i.key_pressed(Key::F10));
        // Fleche haut tourne vers la droite, fleche bas vers la gauche, comme
        // la molette de la console.
        let molette = if interface_occupee {
            0
        } else {
            let droite = self.touches.cles(crate::touches::Commande::TournerDroite);
            let gauche = self.touches.cles(crate::touches::Commande::TournerGauche);
            ctx.input(|i| {
                (droite.iter().any(|k| i.key_pressed(*k)) as i32)
                    - (gauche.iter().any(|k| i.key_pressed(*k)) as i32)
            })
        };
        // Chaque touche tient sa broche tant qu'elle est enfoncee, et plusieurs
        // touches tenues ensemble donnent les combinaisons de la console :
        // molette maintenue plus B pour le menu special, A plus C pour la
        // remise a zero.
        let touches = [
            (Machine::BOUTON_A, crate::touches::Commande::BoutonA),
            (Machine::BOUTON_B, crate::touches::Commande::BoutonB),
            (Machine::BOUTON_C, crate::touches::Commande::BoutonC),
            (Machine::BOUTON_MOLETTE, crate::touches::Commande::Molette),
        ];
        for (broche, commande) in touches {
            let keys = self.touches.cles(commande);
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
        let commande_web_recue = !recues.is_empty();
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
                crate::web::Commande::Temps(pourcent) => {
                    self.vitesse = pourcent.min(400) as f32 / 100.0;
                    self.cycles_dus = 0.0;
                }
                crate::web::Commande::Son(actif) => {
                    self.audio.enabled = actif;
                    if !actif {
                        self.audio.silence_buzzer();
                    }
                    self.retenir_la_partie();
                }
                crate::web::Commande::Volume(volume) => {
                    self.audio.volume = volume.min(100) as f32 / 100.0;
                    self.retenir_la_partie();
                }
                crate::web::Commande::SauverEtat(chemin) => {
                    let etat = self.machine.instantane();
                    self.status_msg = Some(match etat.ecrire(std::path::Path::new(&chemin)) {
                        Ok(()) => format!(
                            "{} {}",
                            self.i18n.choisir("Etat ecrit dans", "State written to"),
                            chemin
                        ),
                        Err(e) => format!(
                            "{} : {}",
                            self.i18n.choisir("Ecriture impossible", "Write failed"),
                            e
                        ),
                    });
                }
                crate::web::Commande::ChargerEtat(chemin) => {
                    self.status_msg =
                        Some(self.restaurer_fichier(std::path::Path::new(&chemin)));
                }
            }
        }
        // Quand l'emulation est en pause, aucune nouvelle trame d'ecran ne
        // vient republier les reglages. Une commande web doit pourtant voir
        // son nouvel etat des la requete suivante.
        if commande_web_recue {
            self.publier();
        }
        for broche in self.tenus_distants.clone() {
            self.maintenir(broche);
        }

        self.appliquer_entrees();
        // Les octets deja arrives sont remis au controleur avant sa tranche
        // d'execution. Le port reste non bloquant, donc une liaison silencieuse
        // ne ralentit pas l'emulation.
        self.uart_bridge.poll_serial(&mut self.machine.periph.uart);
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
                // Le lien serie est servi ici, pas seulement autour de la
                // tranche. L'outil de transfert attend un acquittement avant
                // d'envoyer la suite, et il abandonne vite : le faire attendre
                // une image entiere de chaque cote suffisait a lui faire
                // repeter son ordre, que la console refusait alors a juste
                // titre puisqu'elle attendait des donnees. Le pont ne bloque
                // plus sur le port, cet appel ne coute qu'un verrou.
                self.uart_bridge.poll_serial(&mut self.machine.periph.uart);
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
        // Recopie sans attendre les octets que le firmware vient d'emettre.
        self.uart_bridge.poll_serial(&mut self.machine.periph.uart);
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

        // La table des scenes, une fois pour toutes, des que la fenetre XIP est
        // programmee. Avant cela les pointeurs de noms ne visent rien.
        if self.table_scenes.is_none() && self.machine.periph.xip.is_enabled() {
            self.table_scenes = crate::emulator::scenes::TableScenes::reperer(
                &self.machine.bus.flash.data,
                self.machine.periph.xip.base,
            );
        }

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
        self.dessiner_la_suppression(ctx);
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
                ui.label(egui::RichText::new("Capybara").strong());

                ui.separator();

                if ui.button(self.i18n.choisir("Retour au jeu", "Back to game")).clicked() {
                    self.mode = Mode::Jeu;
                }
                if ui.button(self.i18n.choisir("Accueil", "Home")).clicked() {
                    self.mode = Mode::Accueil;
                }
                self.menu_des_consoles(ui);
                ui.separator();
                self.dessiner_langue(ui);
                // Le son, la coque et l'inspecteur de flash sont partis dans
                // les onglets qui les concernent. Entasses ici, ils poussaient
                // la barre hors de l'ecran des que la fenetre n'etait pas en
                // plein ecran, et rien ne laissait deviner ce qui manquait.
            });
        });

        // 4. Panneau lateral : l'essentiel toujours, l'inspection sur demande.
        {
            SidePanel::right("debug_panel")
                // Redimensionnable, et une largeur minimale qui tient sur un
                // ecran d'ordinateur portable. A 420 le panneau imposait une
                // fenetre si large qu'il fallait passer en plein ecran.
                .resizable(true)
                .min_width(300.0)
                .default_width(460.0)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    // La barre d'onglets passe a la ligne quand le panneau est
                    // etroit, au lieu de deborder hors de la vue.
                    ui.horizontal_wrapped(|ui| {
                        for (onglet, nom) in [
                            (Onglet::Console, "Console"),
                            (Onglet::Uart, "UART"),
                            (Onglet::Sauvegardes, self.i18n.choisir("Sauvegardes", "Saves")),
                            (Onglet::Inspection, self.i18n.choisir("Avance", "Advanced")),
                            (Onglet::Personnalisation, self.i18n.choisir("Personnalisation", "Appearance")),
                            (Onglet::Aide, self.i18n.choisir("Aide", "Help")),
                        ] {
                            if ui.selectable_label(self.onglet == onglet, nom).clicked() {
                                self.onglet = onglet;
                            }
                        }
                    });
                    ui.separator();

                    // Tout le contenu defile. Sans cela le bas du panneau, les
                    // points de reprise et les panneaux d'inspection, restait
                    // hors d'atteinte des que la fenetre n'etait pas en plein
                    // ecran, et aucune barre ne le laissait deviner.
                    egui::ScrollArea::vertical()
                        .id_salt("panneau_lateral")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {

                    // L'habillage occupe plus de place que le panneau n'en a :
                    // il a son onglet, et il defile.
                    if self.onglet == Onglet::Personnalisation {
                        egui::ScrollArea::vertical().id_salt("personnalisation").show(
                            ui,
                            |ui| {
                                self.dessiner_l_habillage(ui, ctx);
                                ui.add_space(12.0);
                                ui.group(|ui| {
                                    ui.label(
                                        egui::RichText::new(self.i18n.choisir("Fenetre", "Window"))
                                            .strong(),
                                    );
                                    // Le reglage n'existait que dans le menu du
                                    // clic droit, qui n'est pas l'endroit ou on
                                    // pense a le chercher.
                                    if ui
                                        .checkbox(
                                            &mut self.toujours_devant,
                                            self.i18n.choisir(
                                                "Rester au dessus des autres fenetres",
                                                "Stay on top of other windows",
                                            ),
                                        )
                                        .changed()
                                    {
                                        // Le niveau est pose au changement de
                                        // mode : on force la reprise pour qu'il
                                        // s'applique tout de suite.
                                        self.mode_applique = None;
                                        self.retenir_la_partie();
                                    }
                                    // L'etat est relu sur le systeme a chaque
                                    // image : c'est lui qui fait foi, l'entree
                                    // pouvant avoir ete retiree par ailleurs.
                                    // Une case a cocher qui ment est pire que
                                    // pas de case du tout.
                                    let mut au_demarrage =
                                        crate::demarrage::actif();
                                    if ui
                                        .checkbox(
                                            &mut au_demarrage,
                                            self.i18n.choisir(
                                                "Demarrer avec le systeme",
                                                "Start with the system",
                                            ),
                                        )
                                        .on_hover_text(self.i18n.choisir(
                                            "Ouvre Capybara a l'ouverture de votre session. Desactive par defaut.",
                                            "Opens Capybara when you log in. Off by default.",
                                        ))
                                        .changed()
                                    {
                                        if let Err(e) =
                                            crate::demarrage::regler(au_demarrage)
                                        {
                                            self.status_msg = Some(format!(
                                                "{} : {}",
                                                self.i18n.choisir(
                                                    "Demarrage automatique impossible",
                                                    "Could not set autostart",
                                                ),
                                                e
                                            ));
                                        }
                                    }
                                });
                                ui.add_space(12.0);
                                if crate::ui::touches_panel::dessiner(
                                    ui,
                                    &mut self.touches,
                                    &mut self.souris,
                                    &mut self.capture_touche,
                                    &self.i18n,
                                ) {
                                    self.retenir_la_partie();
                                }
                                ui.add_space(12.0);
                            },
                        );
                        return;
                    }

                    if self.onglet == Onglet::Aide {
                        crate::ui::aide::dessiner(ui, &self.i18n, &self.maj);
                        return;
                    }

                    // Chaque onglet sort par un retour anticipe : le contenu
                    // reste ecrit a la suite, et un seul bloc s'execute.
                    if self.onglet == Onglet::Console {

                    // Firmware File Loader Box
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(self.i18n.choisir("Firmware / Dump de flash (.bin) :", "Firmware / Flash dump (.bin):")).strong());
                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new(self.i18n.choisir("📂 Parcourir...", "📂 Browse...")).strong()).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Firmware Binary (*.bin, *.rom, *.hex, *.elf)", &["bin", "rom", "hex", "elf", "raw", "dump"])
                                    .set_title(self.i18n.choisir("Selectionner un dump de firmware", "Select a firmware dump"))
                                    .pick_file()
                                {
                                    self.load_firmware(path.clone());
                                }
                            }

                            ui.text_edit_singleline(&mut self.load_path_input);
                            if ui.button(self.i18n.choisir("Charger", "Load")).clicked() && !self.load_path_input.is_empty() {
                                self.load_firmware(std::path::PathBuf::from(self.load_path_input.clone()));
                            }
                        });
                        if let Some(msg) = &self.status_msg {
                            ui.label(egui::RichText::new(msg).small().color(egui::Color32::from_rgb(255, 230, 80)));
                        }
                    });

                    } // fin du chargement du dump, onglet Console

                    // La sauvegarde de la console rejoint les points de reprise
                    // dans son onglet : les deux parlent de ce qui survit a
                    // l'extinction, meme si l'une est la flash du jeu et
                    // l'autre un instantane de mise au point.
                    if self.onglet == Onglet::Sauvegardes {

                    // Sauvegarde de la console. Elle n'a rien a voir avec les
                    // instantanes : ici on ne garde que ce que le jeu a ecrit
                    // dans sa flash, sa vraie memoire, et elle survit a
                    // l'extinction de l'ordinateur.
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(self.i18n.choisir("Sauvegarde de la console :", "Console save:")).strong());
                        let suivie = self.machine.sauvegarde_active.is_some();
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("emplacement_sauvegarde")
                                .selected_text(if self.emplacement_choisi.is_empty() {
                                    self.i18n.choisir("aucune", "none").to_string()
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
                            if ui
                                .button(self.i18n.choisir("Nouvelle partie", "New game"))
                                .on_hover_text(self.i18n.choisir(
                                    "Demande un nom, puis repart de zero. La partie en cours reste intacte.",
                                    "Asks for a name, then starts over. The current game is left untouched.",
                                ))
                                .clicked()
                            {
                                // Une partie neuve va quelque part. Repartir du
                                // dump sans emplacement laissait la console
                                // tourner sans rien qui l'enregistre, et il
                                // fallait deviner qu'il fallait en creer un.
                                let propose = self.nom_de_partie_libre();
                                self.but_de_la_saisie = ButDeLaSaisie::PartieNeuve;
                                self.saisie_sauvegarde = Some(propose);
                            }
                        });
                        ui.horizontal(|ui| {
                            let choisie = !self.emplacement_choisi.is_empty();
                            if ui
                                .add_enabled(
                                    choisie,
                                    egui::Button::new(
                                        egui::RichText::new(self.i18n.choisir(
                                            "Supprimer cette sauvegarde",
                                            "Delete this save",
                                        ))
                                        .color(egui::Color32::from_rgb(240, 140, 130)),
                                    ),
                                )
                                .on_hover_text(self.i18n.choisir(
                                    "Efface la partie et tous ses points de reprise",
                                    "Erases the game and all its recovery points",
                                ))
                                .clicked()
                            {
                                self.suppression_demandee = Some(self.emplacement_choisi.clone());
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(self.i18n.choisir("Nouvel emplacement :", "New slot:"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.nouvel_emplacement)
                                    .desired_width(120.0),
                            );
                            if ui.button(self.i18n.choisir("Creer", "Create")).clicked() {
                                let nom: String = self
                                    .nouvel_emplacement
                                    .trim()
                                    .chars()
                                    .filter(|c| {
                                        c.is_ascii_alphanumeric() || *c == '-' || *c == '_'
                                    })
                                    .collect();
                                if !nom.is_empty() {
                                    self.creer_emplacement_depuis_partie_courante(nom);
                                    self.nouvel_emplacement.clear();
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(if suivie {
                                match &self.machine.sauvegarde_active {
                                    Some(c) => format!("{} {}", self.i18n.choisir("Enregistree dans", "Saved in"), c.display()),
                                    None => String::new(),
                                }
                            } else {
                                self.i18n.choisir("Partie non enregistree", "Unsaved game").to_string()
                            })
                            .small(),
                        );
                    });

                        ui.separator();
                        ui.group(|ui| {
                            self.dessiner_les_reprises(ui);
                        });
                        return;
                    } // fin de l'onglet Sauvegardes

                    if self.onglet == Onglet::Console {
                    ui.separator();

                    // Vitesse, son et serveur : tout ce qui sert a jouer, au
                    // meme endroit. Le son etait dans la barre du haut, loin de
                    // la vitesse alors que les deux reglent la meme chose, la
                    // facon dont la console se rend.
                    ui.group(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(self.i18n.choisir("Son :", "Sound:")).strong());
                            if ui
                                .selectable_label(
                                    self.audio.enabled,
                                    if self.audio.enabled {
                                        self.i18n.choisir("actif", "on")
                                    } else {
                                        self.i18n.choisir("coupe", "off")
                                    },
                                )
                                .clicked()
                            {
                                self.audio.enabled = !self.audio.enabled;
                            }
                            ui.add(
                                egui::Slider::new(&mut self.audio.volume, 0.0..=1.0)
                                    .show_value(false)
                                    .text(self.i18n.choisir("volume", "volume")),
                            );
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(self.i18n.choisir("Hauteur :", "Pitch:")).strong());
                            for (nom, h) in [("/2", 0.5_f32), ("x1", 1.0), ("x2", 2.0), ("x4", 4.0)]
                            {
                                if ui
                                    .selectable_label((self.audio.hauteur - h).abs() < 0.01, nom)
                                    .clicked()
                                {
                                    self.audio.hauteur = h;
                                }
                            }
                        });
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(self.i18n.choisir("Vitesse :", "Speed:")).strong());
                            // Deux positions, et pas davantage. L'emulation
                            // tient tout juste le temps reel : au dessus, la
                            // machine donne deja tout et le reglage ne change
                            // rien. Le diagnostic affiche la vitesse atteinte,
                            // c'est le chiffre a regarder.
                            for (nom, v) in [
                                (self.i18n.choisir("Pause", "Pause"), 0.0_f32),
                                (self.i18n.choisir("Temps reel", "Real time"), 1.0),
                            ] {
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
                                .button(self.i18n.choisir("Annuler les 2 dernieres secondes", "Undo the last 2 seconds"))
                                .on_hover_text(
                                    self.i18n.choisir(
                                        "Filet de mise au point : revient a l'instantane automatique precedent, pris toutes les deux secondes d'emulation. Pour remonter plus loin, servez vous des points de reprise.",
                                        "Debugging net: goes back to the previous automatic snapshot, taken every two seconds of emulation. To rewind further, use the recovery points.",
                                    ),
                                )
                                .clicked()
                            {
                                self.reculer();
                            }
                            if ui
                                .button(self.i18n.choisir("Rallumer la console", "Restart console"))
                                .on_hover_text(
                                    self.i18n.choisir(
                                        "Recharge le dump et remet la console a son demarrage. La partie sauvegardee n'est pas touchee : elle est relue et le jeu reprend ou il en etait.",
                                        "Reloads the dump and returns the console to its startup. The saved game is untouched: it is read back and play resumes where it was.",
                                    ),
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
                                        "{} {}",
                                        self.historique.len(),
                                        self.i18n.choisir("instantanes automatiques", "automatic snapshots")
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
                                    if ui.small_button(self.i18n.choisir("Arreter", "Stop")).clicked() {
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
                                if ui.button(self.i18n.choisir("Demarrer le serveur local", "Start local server")).clicked() {
                                    match crate::web::demarrer(
                                        std::sync::Arc::clone(&self.partage),
                                        7340,
                                    ) {
                                        Ok((port, temoin)) => {
                                            self.port_web = Some(port);
                                            self.serveur_actif = Some(temoin);
                                            self.publier();
                                        }
                                        Err(e) => {
                                            self.status_msg = Some(format!(
                                                "{} : {}",
                                                self.i18n
                                                    .choisir("Serveur local", "Local server"),
                                                e
                                            ));
                                        }
                                    }
                                }
                                ui.label(
                                    egui::RichText::new(
                                        self.i18n.choisir(
                                            "Suivi dans le navigateur, eteint. Une fois allume, il le reste jusqu'a la fermeture.",
                                            "Browser remote is off. Once started, it stays active until the application closes.",
                                        ),
                                    )
                                    .small(),
                                );
                            }
                        }
                        ui.label(
                            egui::RichText::new(
                                self.i18n.choisir(
                                    "Clavier : A ou Fleche gauche, B ou Espace, C ou Fleche droite, Entree pour l'appui de molette, Fleche haut et Fleche bas pour la tourner. Les touches tenues se combinent : molette plus B ouvre le menu special, A plus C reinitialise.",
                                    "Keyboard: A or Left, B or Space, C or Right, Enter presses the wheel, Up and Down rotate it. Held keys combine: wheel plus B opens the special menu, A plus C resets.",
                                ),
                            )
                            .small(),
                        );

                    });
                    return;
                    } // fin de l'onglet Console

                    if self.onglet == Onglet::Uart {
                        ConsolePanel::render(
                            ui,
                            &mut self.machine.periph.uart,
                            &mut self.uart_bridge,
                            self.machine.trace_refus.as_ref(),
                            &self.i18n,
                        );
                        return;
                    }

                    // Le diagnostic sert a rapporter un blocage : sa place est
                    // avec les registres et la memoire, pas avec les commandes
                    // de jeu. L'inspecteur de flash le suit, pour la meme
                    // raison.
                    if self.onglet == Onglet::Inspection {
                    ui.group(|ui| {
                        if ui.button(self.i18n.choisir("💾 Inspecteur Flash", "💾 Flash inspector")).clicked() {
                            self.active_modal = ActiveModal::FlashInspector;
                        }
                        let rapport = self.diagnostic();
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Diagnostic").strong());
                            if ui.button(self.i18n.choisir("Copier", "Copy")).clicked() {
                                ui.output_mut(|o| o.copied_text = rapport.clone());
                                self.status_msg = Some(
                                    self.i18n
                                        .choisir(
                                            "Diagnostic copie dans le presse-papiers.",
                                            "Diagnostic copied to the clipboard.",
                                        )
                                        .to_string(),
                                );
                            }
                        });
                        egui::ScrollArea::vertical()
                            .max_height(170.0)
                            .id_salt("diagnostic")
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(egui::RichText::new(&rapport).monospace().small())
                                        .wrap(),
                                );
                            });
                    });

                    } // fin du diagnostic, onglet Inspection

                    // Les panneaux qui suivent, registres, memoire et
                    // desassemblage, coutent plus cher a dessiner que
                    // l'emulation n'en gagne a tourner. Leur onglet suffit a ne
                    // les payer que quand on les regarde, et le debit affiche
                    // dit tout de suite ce qu'ils prennent.
                    if self.onglet != Onglet::Inspection {
                        return;
                    }

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
                        &self.i18n,
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
                        &self.i18n,
                    );

                        });
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
