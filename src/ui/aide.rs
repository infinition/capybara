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
        "Capybara n'est livre avec aucun firmware. Il vous faut l'image memoire de votre propre console, un fichier .bin lu sur sa puce de flash. Tamagotchi Paradise est toujours en vente : rien ici ne remplace son achat, et rien ne permet d'y jouer sans elle. Il faut posseder physiquement un boitier et lire sa propre memoire. C'est la seule facon honnete de s'en servir : vous emulez la machine que vous possedez deja.\n\nCes images sont chiffrees, mais vous n'avez rien a fournir de plus. Importez votre .bin : s'il reste chiffre, Capybara cherche sa cle tout seul et une jauge montre ou il en est. Comptez une minute. La console demarre des que la cle tombe.\n\nComment c'est possible : la cle AES est en clair dans la table de chargement du dump. Il ne manque qu'une valeur de trente deux bits, qui ne sert qu'a masquer un vecteur d'initialisation. Quatre milliards de candidats passent en revue en moins d'une minute, et la table des vecteurs du coeur dit lequel est le bon. Rien de la cle n'est ecrit dans le logiciel ni dans le depot : c'est votre dump qui rend la sienne.\n\nSi vous connaissez deja votre cle, collez la dans le champ. Elle est rangee a cote de son dump, donc un autre dump avec une autre cle ne l'ecrasera pas.\n\nUn dump importe est recopie dans le dossier de donnees : il reste trouvable au prochain demarrage meme si l'original a bouge.",
        "Capybara ships with no firmware. You need the memory image of your own console, a .bin file read from its flash chip. Tamagotchi Paradise is still on sale: nothing here replaces buying it, and nothing lets you play without it. You must physically own a console and read its own memory. That is the only honest way to use it: you emulate the machine you already own.\n\nThose images are encrypted, but you have nothing else to supply. Import your .bin: if it stays encrypted, Capybara looks for its key on its own and a gauge shows how far it has got. Give it a minute. The console starts as soon as the key turns up.\n\nHow that is possible: the AES key sits in the dump's load table in clear. Only a thirty two bit value is missing, and it does nothing but mask an initialisation vector. Four billion candidates go by in under a minute, and the core's vector table says which one is right. Nothing of the key is written into the software or the repository: your dump yields its own.\n\nIf you already know your key, paste it into the field. It is filed next to its dump, so another dump with another key will not overwrite it.\n\nAn imported dump is copied into the data folder, so it stays available next time even if the original moved.",
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
        "A ou Fleche gauche, B ou Espace, C ou Fleche droite. Fleche haut et Fleche bas tournent la molette. Les touches tenues se combinent : molette plus B ouvre le menu special, A plus C reinitialise.\n\nLa vitesse se regle par mode. Temps reel suit la console, la pause fige tout, et Annuler les deux dernieres secondes revient en arriere sans passer par les points de reprise.\n\nL'ecran devient noir apres un moment sans rien faire : c'est la mise en veille de la console. Un appui la reveille, elle redemarre et reprend la partie.\n\nSi un carre noir entoure la coque en mode jeu, c'est votre carte graphique qui refuse de composer une transparence, et aucune demande du programme n'y change rien. Cochez Decouper la fenetre a la forme de la coque, dans Personnalisation : Windows clippe alors la fenetre lui meme. C'est un contournement et non une reparation, le contour devient net au lieu d'etre fondu, mais il marche partout.",
        "A or Left arrow, B or Space, C or Right arrow. Up and Down turn the dial. Held keys combine: dial plus B opens the special menu, A plus C resets.\n\nSpeed is set per mode. Real time follows the console, pause freezes everything, and Undo the last two seconds rewinds without using recovery points.\n\nThe screen goes black after a while of inactivity: that is the console going to sleep. A button press wakes it, it reboots and resumes the game.\n\nIf a black square surrounds the shell in game mode, your graphics card is refusing to compose transparency, and nothing the program asks will change that. Tick Cut the window to the shape of the shell, in Appearance: Windows then clips the window itself. It is a workaround rather than a repair, the outline becomes crisp instead of faded, but it works everywhere.",
    ),
    (
        "Liaison serie UART",
        "UART serial link",
        "La console parle par un port serie a 460800 bauds. C'est par la qu'un vrai boitier recoit des objets ou joue a deux.\n\nCapybara n'ayant pas de fil, il lui faut un logiciel d'appairage de ports serie virtuels, comme Virtual Serial Port Driver ou com0com. Il cree deux ports COM relies dos a dos : choisissez l'un dans l'onglet UART puis Connecter, et donnez l'autre a l'outil de transfert. Un meme port ne s'ouvre pas deux fois.\n\nTout l'echange est capture dans le dossier de donnees, sous liaison, dans les deux sens. C'est ce qui permet de comprendre un transfert qui echoue.",
        "The console speaks over a serial port at 460800 baud. That is how a real device receives items or plays with another one.\n\nCapybara has no wire, so it needs a virtual serial port pairing tool, such as Virtual Serial Port Driver or com0com. It creates two COM ports wired back to back: pick one in the UART tab and press Connect, and give the other to the transfer tool. The same port cannot be opened twice.\n\nThe whole exchange is captured in the data folder, under liaison, in both directions. That is what makes a failed transfer understandable.",
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
