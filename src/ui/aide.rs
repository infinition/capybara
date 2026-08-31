//! Mode d'emploi, et liens du projet.
//!
//! Le texte vit dans les deux langues cote a cote plutot que dans un fichier a
//! part : il est court, il suit le logiciel, et rien ne peut se desynchroniser
//! entre une version et l'autre.

use egui::{Color32, CollapsingHeader, RichText, Ui};

use crate::i18n::I18n;
use crate::maj::{EtatMaj, Maj, DEPOT, SOUTIEN};

/// Titre francais, titre anglais, corps francais, corps anglais.
type Section = (&'static str, &'static str, &'static str, &'static str);

const SECTIONS: &[Section] = &[
    (
        "Le dump de votre console",
        "Your console dump",
        "Capybara n'est livre avec aucun firmware. Il vous faut l'image memoire de votre propre console, un fichier .bin lu sur sa puce de flash. C'est la seule facon honnete de s'en servir : vous emulez la machine que vous possedez.\n\nLes images sont chiffrees. La cle se pose une fois pour toutes dans le dossier de donnees, ou a cote du dump dans un fichier portant le meme nom suivi de .key. Elle n'est ni fournie ni enregistree dans le depot.\n\nUn dump importe est recopie dans le dossier de donnees : il reste trouvable au prochain demarrage meme si l'original a bouge.",
        "Capybara ships with no firmware. You need the memory image of your own console, a .bin file read from its flash chip. That is the only honest way to use it: you emulate the machine you own.\n\nImages are encrypted. The key goes once into the data folder, or next to the dump in a file with the same name plus .key. It is neither shipped nor stored in the repository.\n\nAn imported dump is copied into the data folder, so it stays available next time even if the original moved.",
    ),
    (
        "Parties et emplacements",
        "Games and slots",
        "Un emplacement est une partie. Il porte ce que le jeu a ecrit dans sa flash : le personnage, son age, ses jauges, son journal. Chaque dump a ses propres emplacements, parce que les cinq editions n'ont ni les memes ressources ni la meme disposition.\n\nNouvelle partie demande un nom, puis repart de l'image du dump. La partie en cours n'est pas touchee, elle reste sur le disque.\n\nSupprimer cette sauvegarde efface la partie et tous ses points de reprise, apres confirmation. La console continue de tourner : plus rien ne l'enregistre tant que vous n'ouvrez pas un emplacement.\n\nAu lancement, la console reprend sa derniere partie toute seule, comme un vrai boitier qu'on rallume.",
        "A slot is a game. It holds what the game wrote to its flash: the character, its age, its gauges, its diary. Each dump has its own slots, because the five editions share neither resources nor layout.\n\nNew game asks for a name, then starts over from the dump image. The current game is untouched and stays on disk.\n\nDelete this save erases the game and all its recovery points, after confirmation. The console keeps running, but nothing is saved until you open a slot.\n\nOn launch the console resumes its last game by itself, like a real device switched back on.",
    ),
    (
        "Points de reprise",
        "Recovery points",
        "A ne pas confondre avec une sauvegarde. Une sauvegarde ne retient que la flash du jeu. Un point de reprise fige toute la machine, coeur et peripheriques compris, et permet de revenir en arriere a la seconde pres.\n\nUn point est pris chaque minute et garde jusqu'a douze heures. Cliquez sur une heure pour y ramener la console. Poser un point en cree un tout de suite.\n\nExporter l'etat ecrit un fichier .tamastate, Importer en adopte un venu d'ailleurs. Un point appartient a son dump : il lui faut le meme firmware sous les pieds.",
        "Not to be confused with a save. A save keeps only the game flash. A recovery point freezes the whole machine, core and peripherals included, and lets you rewind to the second.\n\nA point is taken every minute and kept for twelve hours. Click a time to bring the console back to it. Create recovery point makes one right away.\n\nExport state writes a .tamastate file, Import adopts one from elsewhere. A point belongs to its dump: it needs the same firmware underneath.",
    ),
    (
        "Jouer",
        "Playing",
        "A ou Fleche gauche, B ou Espace, C ou Fleche droite. Fleche haut et Fleche bas tournent la molette. Les touches tenues se combinent : molette plus B ouvre le menu special, A plus C reinitialise.\n\nLa vitesse se regle par mode. Temps reel suit la console, la pause fige tout, et Annuler les deux dernieres secondes revient en arriere sans passer par les points de reprise.\n\nL'ecran devient noir apres un moment sans rien faire : c'est la mise en veille de la console. Un appui la reveille, elle redemarre et reprend la partie.",
        "A or Left arrow, B or Space, C or Right arrow. Up and Down turn the dial. Held keys combine: dial plus B opens the special menu, A plus C resets.\n\nSpeed is set per mode. Real time follows the console, pause freezes everything, and Undo the last two seconds rewinds without using recovery points.\n\nThe screen goes black after a while of inactivity: that is the console going to sleep. A button press wakes it, it reboots and resumes the game.",
    ),
    (
        "Liaison serie UART",
        "UART serial link",
        "La console parle par un port serie a 460800 bauds. C'est par la qu'un vrai boitier recoit des objets ou joue a deux.\n\nCapybara n'ayant pas de fil, il lui faut une paire de ports serie virtuels : ce programme ouvre un cote, l'outil de transfert ouvre l'autre. Choisissez le port dans l'onglet UART, puis Connecter.\n\nTout l'echange est capture dans le dossier de donnees, sous liaison, dans les deux sens. C'est ce qui permet de comprendre un transfert qui echoue.",
        "The console speaks over a serial port at 460800 baud. That is how a real device receives items or plays with another one.\n\nCapybara has no wire, so it needs a virtual serial port pair: this program opens one end, the transfer tool opens the other. Pick the port in the UART tab, then Connect.\n\nThe whole exchange is captured in the data folder, under liaison, in both directions. That is what makes a failed transfer understandable.",
    ),
    (
        "Habillage",
        "Appearance",
        "La coque, ses couleurs, le titre imprime dessus et le papier glisse derriere se reglent dans Personnalisation. Le fond suit la coque et non la console : il ne change pas quand vous changez de partie.\n\nLe reste, son, volume, hauteur, zoom, fenetre au dessus et langue, est retenu d'un lancement a l'autre.",
        "The shell, its colours, the title printed on it and the paper tucked behind are set in Appearance. The background follows the shell and not the console: it does not change when you switch games.\n\nThe rest, sound, volume, pitch, zoom, always on top and language, is remembered between launches.",
    ),
    (
        "Ou vont les fichiers",
        "Where files go",
        "Tout vit dans le dossier de donnees du systeme, jamais a cote de l'executable : un programme deplace garde ses parties, et un dossier en lecture seule ne bloque rien.\n\nSous Windows dans %APPDATA%\\Capybara\\data, sous Mac dans Library/Application Support/Capybara, sous Linux dans .local/share/capybara. On y trouve les dumps importes, les sauvegardes, les points de reprise et les captures de la liaison.",
        "Everything lives in the system data folder, never next to the executable: a moved program keeps its games, and a read only folder blocks nothing.\n\nOn Windows in %APPDATA%\\Capybara\\data, on Mac in Library/Application Support/Capybara, on Linux in .local/share/capybara. It holds imported dumps, saves, recovery points and link captures.",
    ),
];

/// Dessine l'onglet d'aide, sections repliees et bloc a propos.
pub fn dessiner(ui: &mut Ui, i18n: &I18n, maj: &Maj) {
    ui.label(
        RichText::new(i18n.choisir("Mode d'emploi", "User guide"))
            .strong()
            .size(16.0),
    );
    ui.label(
        RichText::new(i18n.choisir(
            "L'essentiel, dans l'ordre ou l'on s'en sert.",
            "The essentials, in the order you need them.",
        ))
        .small(),
    );
    ui.add_space(6.0);

    for (titre_fr, titre_en, corps_fr, corps_en) in SECTIONS {
        CollapsingHeader::new(RichText::new(i18n.choisir(titre_fr, titre_en)).strong())
            .default_open(false)
            .show(ui, |ui| {
                ui.label(i18n.choisir(corps_fr, corps_en));
            });
    }

    ui.add_space(10.0);
    a_propos(ui, i18n, maj);
}

/// Version installee, verification des mises a jour et liens du projet.
fn a_propos(ui: &mut Ui, i18n: &I18n, maj: &Maj) {
    ui.group(|ui| {
        ui.label(RichText::new(i18n.choisir("A propos", "About")).strong());
        ui.label(RichText::new(format!("Capybara {}", Maj::version_installee())).small());
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(i18n.choisir("Verifier les mises a jour", "Check for updates"))
                .on_hover_text(i18n.choisir(
                    "Interroge le depot et dit seulement s'il existe une version plus recente. Rien n'est telecharge.",
                    "Asks the repository and only says whether a newer version exists. Nothing is downloaded.",
                ))
                .clicked()
            {
                maj.verifier();
            }
            ui.hyperlink_to(i18n.choisir("Depot GitHub", "GitHub repository"), DEPOT);
            ui.hyperlink_to(
                i18n.choisir("Vous aimez ? Soutenir mon travail", "You like it? Support my work"),
                SOUTIEN,
            );
        });
        match maj.etat() {
            EtatMaj::Jamais => {}
            EtatMaj::EnCours => {
                ui.label(RichText::new(i18n.choisir("Verification...", "Checking...")).small());
            }
            EtatMaj::AJour => {
                ui.label(
                    RichText::new(i18n.choisir(
                        "Vous avez la derniere version.",
                        "You have the latest version.",
                    ))
                    .small()
                    .color(Color32::from_rgb(140, 220, 150)),
                );
            }
            EtatMaj::Disponible { version, page } => {
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        i18n.choisir("Version disponible :", "Version available:"),
                        version
                    ))
                    .small()
                    .color(Color32::from_rgb(240, 200, 100)),
                );
                ui.hyperlink_to(i18n.choisir("Voir la publication", "See the release"), page);
            }
            EtatMaj::Echec(raison) => {
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        i18n.choisir("Verification impossible :", "Check failed:"),
                        raison
                    ))
                    .small()
                    .color(Color32::from_rgb(230, 150, 140)),
                );
            }
        }
    });
}
