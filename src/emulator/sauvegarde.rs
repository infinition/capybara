//! Sauvegardes persistantes, celles de la console elle meme.
//!
//! A ne pas confondre avec les instantanes de `etat.rs`. Un instantane fige
//! toute la machine, coeur et peripheriques compris, pour revenir en arriere
//! pendant la mise au point. Une sauvegarde ne retient que ce que le jeu a
//! ecrit dans sa flash, exactement comme la memoire d'un vrai Tamagotchi : le
//! personnage, son age, ses jauges, l'heure de sa derniere mise a jour.
//!
//! Le firmware ecrit son etat dans deux pages de 4 Ko, en `0xEFE000` et
//! `0xEFF000`, et touche aussi quelques pages de donnees de jeu. On garde donc
//! toutes les pages salies, sans avoir a savoir a quoi chacune sert.
//!
//! Un fichier de sauvegarde appartient a un dump precis : les cinq editions
//! n'ont ni les memes ressources ni la meme disposition. Les fichiers sont donc
//! ranges par empreinte du dump, et le selecteur ne propose que celles qui vont
//! avec le firmware charge.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::emulator::etat::PAGE_FLASH;
use crate::emulator::Machine;

/// En-tete du fichier, pour refuser tout de suite ce qui n'en est pas un.
const MAGIE: &[u8; 8] = b"TAMASAVE";
/// Version du format. Elle changera si la disposition change.
const VERSION: u32 = 3;
/// Extension des fichiers de sauvegarde.
pub const EXTENSION: &str = "tamasave";
/// Nom de l'emplacement pris quand l'utilisateur n'en choisit pas.
pub const EMPLACEMENT_PAR_DEFAUT: &str = "partie";

/// Une sauvegarde lue ou a ecrire.
///
/// Elle porte les pages de flash du jeu, mais aussi l'horloge de la console :
/// sans elle, un Tamagotchi range dans un tiroir ne vieillirait pas. Le
/// compteur de secondes est celui du bloc d'horloge, `0x45000304` ; il repart a
/// la valeur enregistree, augmentee du temps reellement passe depuis, mesure a
/// l'horloge de l'ordinateur.
#[derive(Clone, Default)]
pub struct Sauvegarde {
    pub pages: BTreeMap<usize, Vec<u8>>,
    /// Date de l'ecriture, en secondes depuis 1970.
    pub horodatage: u64,
    /// Compteur de secondes de la console au moment de l'ecriture.
    pub compteur: u32,
    /// Comparateur d'alarme, `0x45000230`.
    pub alarme: u32,
    /// Statut d'alarme, `0x45000234`, avec son temoin d'armement.
    pub statut_alarme: u32,
    /// Tous les registres de la zone systeme SN_SYS0. Elle est alimentee en
    /// permanence sur la puce et garde son contenu coeur eteint : sans elle, la
    /// console rallumee ne saurait pas d'ou vient son reveil.
    pub registres_systeme: Vec<(u32, u32)>,
}

/// Secondes depuis 1970, ou zero si l'horloge du systeme est illisible.
fn maintenant() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Sauvegarde {
    /// Recolte les pages que le jeu a ecrites depuis le chargement.
    pub fn depuis(machine: &Machine) -> Self {
        let mut pages = BTreeMap::new();
        for &page in &machine.bus.flash.pages_salies {
            let debut = page * PAGE_FLASH;
            let fin = (debut + PAGE_FLASH).min(machine.bus.flash.data.len());
            if debut < fin {
                pages.insert(page, machine.bus.flash.data[debut..fin].to_vec());
            }
        }
        use super::peripherals::SnSysRegisters as Sys;
        let compteur = machine.periph.snsys.secondes;
        let alarme;
        let mut statut = machine.periph.snsys.read_reg(Sys::STATUT_ALARME);
        // Ranger la console revient a l'endormir. Si le firmware n'avait pas
        // arme son reveil, parce qu'on ferme la fenetre en pleine partie, on
        // l'arme pour lui, a l'instant : la prochaine ouverture ressemblera
        // alors a une sortie de veille et non a une pile neuve, et le
        // personnage reprendra sa vie au lieu de repasser par le reglage de
        // l'heure. L'echeance est posee juste avant le compteur, pour que le
        // reveil soit acquis des la premiere seconde ecoulee.
        alarme = compteur.saturating_sub(1);
        statut |= Sys::ALARME_ARMEE;
        let mut registres_systeme = machine.periph.snsys.registres();
        for (offset, valeur) in registres_systeme.iter_mut() {
            if *offset == Sys::ALARME {
                *valeur = alarme;
            }
            if *offset == Sys::STATUT_ALARME {
                *valeur = statut;
            }
        }
        Self {
            pages,
            horodatage: maintenant(),
            compteur,
            alarme,
            statut_alarme: statut,
            registres_systeme,
        }
    }

    /// Recopie les pages dans la flash de la machine.
    ///
    /// A appeler juste apres le chargement du dump : la reference des
    /// instantanes est alors deja figee sur l'image d'origine, et les pages
    /// posees ici deviennent des pages salies, comme si le jeu venait de les
    /// ecrire lui meme.
    pub fn appliquer(&self, machine: &mut Machine) {
        for (&page, contenu) in &self.pages {
            let debut = page * PAGE_FLASH;
            let fin = (debut + contenu.len()).min(machine.bus.flash.data.len());
            if debut >= fin {
                continue;
            }
            machine.bus.flash.data[debut..fin].copy_from_slice(&contenu[..(fin - debut)]);
            machine.bus.flash.pages_salies.insert(page);
        }
        self.remettre_l_horloge(machine);
    }

    /// Repose l'horloge de la console, avancee du temps reellement ecoule.
    ///
    /// Une console rangee continue de compter les secondes : c'est ainsi qu'un
    /// Tamagotchi a faim quand on le retrouve. On ajoute donc au compteur
    /// enregistre l'ecart entre l'horodatage du fichier et l'heure courante.
    ///
    /// Unless `machine.temps_hors_ligne` is false: the counter then resumes
    /// exactly where it stopped, and closing the window pauses the world. The
    /// alarm is armed either way, otherwise the firmware would take the reopen
    /// for a fresh battery and ask for the time again.
    ///
    /// Si l'alarme etait armee et que son echeance est passee pendant ce temps,
    /// on la fait sonner : le firmware retrouvera au demarrage la trace d'un
    /// reveil, et reprendra la partie au lieu de croire a une pile neuve.
    fn remettre_l_horloge(&self, machine: &mut Machine) {
        use super::peripherals::SnSysRegisters as Sys;
        if self.horodatage == 0 && self.compteur == 0 {
            return;
        }
        let ecoule = if machine.temps_hors_ligne {
            maintenant().saturating_sub(self.horodatage)
        } else {
            0
        };
        machine.periph.snsys.secondes =
            self.compteur.saturating_add(ecoule.min(u32::MAX as u64) as u32);
        machine.periph.snsys.poser_registres(&self.registres_systeme);
        machine.periph.snsys.write_reg(Sys::ALARME, self.alarme);
        machine.periph.snsys.write_reg(Sys::STATUT_ALARME, self.statut_alarme);
        let armee = self.statut_alarme & Sys::ALARME_ARMEE != 0;
        if armee && self.alarme != 0 && machine.periph.snsys.secondes > self.alarme {
            machine.periph.snsys.declencher_reveil();
            // Le coeur n'est pas encore parti : le reveil ne doit pas
            // declencher de remise a zero, seulement laisser sa trace.
            machine.periph.snsys.reveil_demande = false;
        }
    }

    pub fn est_vide(&self) -> bool {
        self.pages.is_empty()
    }

    /// Serialise en un bloc compact : en-tete, puis chaque page precedee de son
    /// numero et de sa longueur.
    pub fn encoder(&self) -> Vec<u8> {
        let mut octets = Vec::with_capacity(self.pages.len() * (PAGE_FLASH + 8) + 16);
        octets.extend_from_slice(MAGIE);
        octets.extend_from_slice(&VERSION.to_le_bytes());
        octets.extend_from_slice(&self.horodatage.to_le_bytes());
        octets.extend_from_slice(&self.compteur.to_le_bytes());
        octets.extend_from_slice(&self.alarme.to_le_bytes());
        octets.extend_from_slice(&self.statut_alarme.to_le_bytes());
        octets.extend_from_slice(&(self.registres_systeme.len() as u32).to_le_bytes());
        for &(offset, valeur) in &self.registres_systeme {
            octets.extend_from_slice(&offset.to_le_bytes());
            octets.extend_from_slice(&valeur.to_le_bytes());
        }
        octets.extend_from_slice(&(self.pages.len() as u32).to_le_bytes());
        for (&page, contenu) in &self.pages {
            octets.extend_from_slice(&(page as u32).to_le_bytes());
            octets.extend_from_slice(&(contenu.len() as u32).to_le_bytes());
            octets.extend_from_slice(contenu);
        }
        octets
    }

    pub fn decoder(octets: &[u8]) -> Result<Self, String> {
        if octets.len() < 16 || &octets[..8] != MAGIE {
            return Err("ce n'est pas un fichier de sauvegarde".into());
        }
        let mot = |i: usize| -> u32 {
            u32::from_le_bytes([octets[i], octets[i + 1], octets[i + 2], octets[i + 3]])
        };
        let version = mot(8);
        if version != VERSION {
            return Err(format!("version de sauvegarde inconnue : {}", version));
        }
        if octets.len() < 36 {
            return Err("fichier de sauvegarde tronque".into());
        }
        let horodatage = u64::from_le_bytes([
            octets[12], octets[13], octets[14], octets[15], octets[16], octets[17], octets[18],
            octets[19],
        ]);
        let compteur = mot(20);
        let alarme = mot(24);
        let statut_alarme = mot(28);
        let nombre_registres = mot(32) as usize;
        let mut registres_systeme = Vec::with_capacity(nombre_registres);
        let mut i = 36;
        for _ in 0..nombre_registres {
            if i + 8 > octets.len() {
                return Err("fichier de sauvegarde tronque".into());
            }
            registres_systeme.push((mot(i), mot(i + 4)));
            i += 8;
        }
        if i + 4 > octets.len() {
            return Err("fichier de sauvegarde tronque".into());
        }
        let nombre = mot(i) as usize;
        i += 4;
        let mut pages = BTreeMap::new();
        for _ in 0..nombre {
            if i + 8 > octets.len() {
                return Err("fichier de sauvegarde tronque".into());
            }
            let page = mot(i) as usize;
            let longueur = mot(i + 4) as usize;
            i += 8;
            if i + longueur > octets.len() {
                return Err("fichier de sauvegarde tronque".into());
            }
            pages.insert(page, octets[i..i + longueur].to_vec());
            i += longueur;
        }
        Ok(Self { pages, horodatage, compteur, alarme, statut_alarme, registres_systeme })
    }

    pub fn lire(chemin: &Path) -> Result<Self, String> {
        let octets = std::fs::read(chemin).map_err(|e| e.to_string())?;
        Self::decoder(&octets)
    }

    /// Ecrit le fichier, en passant par un fichier temporaire.
    ///
    /// La console sauvegarde souvent, et l'ordinateur peut s'eteindre pendant
    /// l'ecriture. Le renommage final est atomique : on ne peut pas se
    /// retrouver avec un fichier a moitie ecrit a la place d'une partie.
    pub fn ecrire(&self, chemin: &Path) -> Result<(), String> {
        if let Some(parent) = chemin.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let provisoire = chemin.with_extension("tamasave.tmp");
        std::fs::write(&provisoire, self.encoder()).map_err(|e| e.to_string())?;
        std::fs::rename(&provisoire, chemin).map_err(|e| e.to_string())
    }
}

/// Dossier de donnees du logiciel, celui que le systeme reserve a ce genre de
/// contenu.
///
/// `%APPDATA%\\Capybara\\data` sur Windows,
/// `~/Library/Application Support/Capybara` sur Mac,
/// `~/.local/share/capybara` sur Linux. Le logiciel se distribue en
/// un seul executable : ses parties, ses reglages, ses points de reprise et les
/// dumps importes n'ont rien a faire a cote de lui, ou un deplacement du
/// fichier les perdrait et ou un dossier en lecture seule les empecherait.
///
/// Faute de dossier systeme lisible, on se rabat sur celui de l'executable,
/// puis sur le dossier courant : mieux vaut ecrire quelque part que pas du tout.
/// Le dossier s'appelait `TamagotchiParadise`. Il est deplace au premier
/// lancement, une seule fois : personne ne doit perdre ses parties parce que le
/// logiciel a change de nom. Si le deplacement echoue, parce qu'un fichier est
/// ouvert ou que les deux dossiers ne sont pas sur le meme volume, on continue
/// sur l'ancien plutot que de repartir sur un dossier vide.
pub fn dossier_donnees() -> PathBuf {
    static DOSSIER: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DOSSIER.get_or_init(calculer_le_dossier_de_donnees).clone()
}

/// Nom du logiciel avant qu'il ne s'appelle Capybara.
const ANCIEN_NOM: &str = "TamagotchiParadise";
const NOM: &str = "Capybara";

fn calculer_le_dossier_de_donnees() -> PathBuf {
    let Some(dirs) = directories::ProjectDirs::from("", "", NOM) else {
        return std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
    };
    let neuf = dirs.data_dir().to_path_buf();
    if neuf.exists() {
        return neuf;
    }
    let Some(anciens) = directories::ProjectDirs::from("", "", ANCIEN_NOM) else {
        return neuf;
    };
    let ancien = anciens.data_dir().to_path_buf();
    if !ancien.exists() {
        return neuf;
    }
    if let Some(parent) = neuf.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::rename(&ancien, &neuf) {
        Ok(()) => neuf,
        Err(_) => ancien,
    }
}

/// Dossier des sauvegardes.
pub fn dossier_racine() -> PathBuf {
    dossier_donnees().join("sauvegardes")
}

/// Dossier des dumps de flash connus du logiciel.
///
/// Tout `.bin` qui s'y trouve est propose au lancement. Un dump choisi
/// ailleurs y est recopie : c'est ce qui permet de le retrouver au prochain
/// demarrage meme si l'original a bouge.
pub fn dossier_firmwares() -> PathBuf {
    dossier_donnees().join("firmwares")
}

/// Dumps presents dans le dossier des firmwares, par ordre alphabetique.
pub fn firmwares_connus() -> Vec<PathBuf> {
    let mut trouves = Vec::new();
    let Ok(entrees) = std::fs::read_dir(dossier_firmwares()) else {
        return trouves;
    };
    for entree in entrees.flatten() {
        let chemin = entree.path();
        let extension = chemin
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        if matches!(extension.as_deref(), Some("bin" | "rom" | "dump" | "raw")) {
            trouves.push(chemin);
        }
    }
    trouves.sort();
    trouves
}

/// Recopie un dump dans le dossier des firmwares et rend son nouveau chemin.
///
/// Un dump deja range la n'est pas recopie. Si la copie echoue, on rend le
/// chemin d'origine : mieux vaut jouer sur le fichier ou il est que refuser de
/// le charger.
pub fn adopter_firmware(source: &Path) -> PathBuf {
    let dossier = dossier_firmwares();
    if source.starts_with(&dossier) {
        return source.to_path_buf();
    }
    let Some(nom) = source.file_name() else {
        return source.to_path_buf();
    };
    let cible = dossier.join(nom);
    if cible.is_file() {
        // Deja recopie, mais peut etre sans sa cle : les premieres versions ne
        // la prenaient pas, et le dump chiffre restait alors illisible.
        adopter_la_cle(source);
        return cible;
    }
    if std::fs::create_dir_all(&dossier).is_err() {
        return source.to_path_buf();
    }
    match std::fs::copy(source, &cible) {
        Ok(_) => {
            adopter_la_cle(source);
            cible
        }
        Err(_) => source.to_path_buf(),
    }
}

/// Nom du fichier de cle pose a cote d'un dump.
fn cle_voisine(dump: &Path) -> PathBuf {
    let extension = dump.extension().and_then(|e| e.to_str()).unwrap_or("bin");
    dump.with_extension(format!("{}.key", extension))
}

/// Fichier de cle du dossier de donnees, valable pour tous les dumps.
pub fn chemin_cle_commune() -> PathBuf {
    dossier_donnees().join("cle-device.txt")
}

/// Cle commune deja enregistree, en hexadecimal, si elle existe.
pub fn lire_cle_commune() -> Option<String> {
    std::fs::read_to_string(chemin_cle_commune()).ok().map(|s| s.trim().to_string())
}

/// Enregistre la cle commune du dossier de donnees.
///
/// Elle n'etait fournissable que par une variable d'environnement, qui ne
/// survit pas a un lancement par double-clic, ou par un fichier a poser a la
/// main a cote du dump. Autant dire qu'elle n'etait pas fournissable du tout
/// pour qui ouvre le logiciel pour la premiere fois.
pub fn ecrire_cle_commune(valeur: &str) -> Result<(), String> {
    let propre = valeur.trim().trim_start_matches("0x").trim_start_matches("0X");
    if propre.is_empty() {
        let _ = std::fs::remove_file(chemin_cle_commune());
        return Ok(());
    }
    if u32::from_str_radix(propre, 16).is_err() {
        return Err("huit chiffres hexadecimaux attendus".to_string());
    }
    if let Some(parent) = chemin_cle_commune().parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(chemin_cle_commune(), propre).map_err(|e| e.to_string())
}

/// Enregistre la cle d'un dump precis, a cote de lui.
///
/// Le chargeur consulte ce fichier avant le fichier commun : chaque dump garde
/// donc la sienne. Sans cela, importer un dump dont la cle differe ecraserait
/// celle du precedent et le rendrait illisible.
pub fn ecrire_cle_du_dump(dump: &Path, valeur: &str) -> Result<(), String> {
    let propre = valeur.trim().trim_start_matches("0x").trim_start_matches("0X");
    if u32::from_str_radix(propre, 16).is_err() {
        return Err("huit chiffres hexadecimaux attendus".to_string());
    }
    std::fs::write(cle_voisine(dump), propre).map_err(|e| e.to_string())?;
    // Le fichier commun ne sert que de recours pour un dump arrive sans cle.
    // On ne l'ecrase pas : le premier trouve fait foi.
    let commune = chemin_cle_commune();
    if !commune.is_file() {
        let _ = std::fs::write(commune, propre);
    }
    Ok(())
}

/// Recopie la cle posee a cote d'un dump importe.
///
/// Sans elle, un dump chiffre recopie dans le dossier de donnees devient
/// illisible : la cle etait restee a cote de l'original. Elle est ecrite deux
/// fois, a cote de la copie et une fois pour toutes dans le dossier de
/// donnees, la meme cle valant pour les cinq editions. Un dump importe plus
/// tard sans sa cle se lit alors quand meme.
fn adopter_la_cle(source: &Path) {
    let voisine = cle_voisine(source);
    let Ok(contenu) = std::fs::read_to_string(&voisine) else {
        return;
    };
    let Some(nom) = source.file_name() else {
        return;
    };
    let _ = std::fs::write(cle_voisine(&dossier_firmwares().join(nom)), &contenu);
    let commune = chemin_cle_commune();
    if !commune.is_file() {
        let _ = std::fs::write(commune, &contenu);
    }
}

/// Deplace les donnees ecrites a cote de l'executable vers le dossier systeme.
///
/// Les premieres versions rangeaient les parties a cote du binaire. Elles y
/// sont deplacees une fois, sans quoi une partie en cours serait perdue de vue
/// au premier lancement de cette version ci.
pub fn migrer_les_anciennes_donnees() {
    let Some(voisin) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    else {
        return;
    };
    let ancien = voisin.join("sauvegardes");
    let nouveau = dossier_racine();
    if !ancien.is_dir() || nouveau.is_dir() || ancien == nouveau {
        return;
    }
    if let Some(parent) = nouveau.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Un renommage suffit sur le meme volume ; sinon on recopie.
    if std::fs::rename(&ancien, &nouveau).is_ok() {
        return;
    }
    let _ = copier_recursivement(&ancien, &nouveau);
}

fn copier_recursivement(source: &Path, cible: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(cible)?;
    for entree in std::fs::read_dir(source)? {
        let entree = entree?;
        let vers = cible.join(entree.file_name());
        if entree.file_type()?.is_dir() {
            copier_recursivement(&entree.path(), &vers)?;
        } else {
            std::fs::copy(entree.path(), vers)?;
        }
    }
    Ok(())
}

/// Derniere partie ouverte, retenue d'un lancement a l'autre.
///
/// La sauvegarde `.tamasave` survivait deja a l'extinction de l'ordinateur,
/// mais il fallait redesigner le dump et l'emplacement a chaque demarrage.
/// Un vrai Tamagotchi qu'on rallume reprend ou il en etait, sans rien demander.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DernierePartie {
    pub dump: String,
    pub emplacement: String,
    /// Mode de la fenetre : `accueil`, `jeu` ou `inspection`. Quitter en mode
    /// jeu sur une edition doit rallumer dessus, sans repasser par l'accueil.
    #[serde(default)]
    pub mode: String,
    /// Reglages de son. Les champs manquants prennent leur valeur par defaut :
    /// un fichier ecrit par une version anterieure reste lisible.
    #[serde(default = "vrai")]
    pub son: bool,
    #[serde(default = "volume_par_defaut")]
    pub volume: f32,
    #[serde(default = "un")]
    pub hauteur: f32,
    /// Coque choisie a la main, quand elle ne suit pas l'edition.
    #[serde(default)]
    pub coque: String,
    /// Taille de la fenetre du mode jeu, en fraction de sa taille de base.
    #[serde(default = "un")]
    pub zoom_jeu: f32,
    /// Fenetre du mode jeu maintenue au dessus des autres.
    #[serde(default)]
    pub toujours_devant: bool,
    /// Langue de l'interface, le francais restant la valeur par defaut.
    #[serde(default = "langue_par_defaut")]
    pub langue: String,
    /// Correspondance clavier, reglee par l'utilisateur.
    #[serde(default)]
    pub touches: crate::touches::Touches,
    /// Ce que font les boutons de la souris sur l'ecran.
    #[serde(default)]
    pub souris: crate::touches::Souris,
    /// Fond du mode jeu decoupe sur le bureau. Certaines cartes graphiques
    /// refusent la transparence par pixel et rendent un carre noir : la
    /// decouper devient alors un defaut, pas un agrement.
    #[serde(default = "vrai")]
    pub fond_transparent: bool,
    /// Repli de Windows : une couleur devient transparente a l'affichage. Il
    /// sert quand la carte refuse la composition par pixel et rend un carre
    /// noir a la place.
    #[serde(default)]
    pub couleur_cle_active: bool,
    #[serde(default = "magenta")]
    pub couleur_cle: [u8; 3],
    /// Time keeps passing for the console while the window is closed.
    ///
    /// This is the real device's behaviour and the default: a Tamagotchi left
    /// in a drawer ages. Turning it off pauses the world on close, which is
    /// what someone wants who cannot leave the emulator running and would
    /// rather not find their character neglected.
    #[serde(default = "vrai")]
    pub temps_hors_ligne: bool,
    /// The console must not stay in deep sleep.
    ///
    /// False by default: sleeping is the real device's behaviour.
    #[serde(default)]
    pub veille_interdite: bool,
}

/// Couleur de transparence par defaut. Un magenta franc, qui n'apparait ni
/// dans les coques ni dans l'ecran de la console : tout pixel de cette teinte
/// devient un trou, autant en choisir une qu'on ne dessine jamais.
fn magenta() -> [u8; 3] {
    [255, 0, 255]
}

fn vrai() -> bool {
    true
}

fn volume_par_defaut() -> f32 {
    0.5
}

fn un() -> f32 {
    1.0
}

fn langue_par_defaut() -> String {
    "fr".to_string()
}

impl Default for DernierePartie {
    fn default() -> Self {
        Self {
            dump: String::new(),
            emplacement: String::new(),
            mode: String::new(),
            son: true,
            volume: 0.5,
            hauteur: 1.0,
            coque: String::new(),
            zoom_jeu: 1.0,
            toujours_devant: false,
            touches: crate::touches::Touches::default(),
            souris: crate::touches::Souris::default(),
            fond_transparent: true,
            couleur_cle_active: false,
            couleur_cle: magenta(),
            langue: langue_par_defaut(),
            temps_hors_ligne: true,
            veille_interdite: false,
        }
    }
}

/// Fichier qui la porte, a cote des sauvegardes.
pub fn chemin_derniere_partie() -> PathBuf {
    dossier_donnees().join("derniere-partie.json")
}

pub fn lire_derniere_partie() -> Option<DernierePartie> {
    let texte = std::fs::read_to_string(chemin_derniere_partie()).ok()?;
    serde_json::from_str(&texte).ok()
}

/// L'ecrit sans bruit : ne pas pouvoir la retenir n'empeche pas de jouer.
pub fn ecrire_derniere_partie(partie: &DernierePartie) {
    let chemin = chemin_derniere_partie();
    if let Some(parent) = chemin.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(texte) = serde_json::to_string_pretty(partie) {
        let _ = std::fs::write(chemin, texte);
    }
}

/// Empreinte d'un dump : son nom, puis huit chiffres tires de son contenu.
///
/// Le nom seul ne suffit pas, deux copies renommees se confondraient ; le
/// contenu seul donnerait un dossier illisible. Les deux ensemble restent
/// lisibles dans l'explorateur et distinguent les cinq editions.
pub fn empreinte(chemin_dump: &Path, contenu: &[u8]) -> String {
    let nom: String = chemin_dump
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "dump".into())
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    // FNV-1a sur tout le contenu. Seize mega-octets se parcourent en quelques
    // millisecondes, et c'est fait une seule fois au chargement.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &o in contenu {
        h ^= o as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{}-{:08x}", nom, (h ^ (h >> 32)) as u32)
}

/// Dossier des sauvegardes d'un dump donne.
pub fn dossier_du_dump(empreinte: &str) -> PathBuf {
    dossier_racine().join(empreinte)
}

/// Dossier des points de reprise d'un emplacement de sauvegarde.
///
/// Ils suivent la partie et non la console : deux parties menees sur le meme
/// dump ont chacune leur passe, et revenir en arriere sur l'une ne propose
/// jamais les points de l'autre.
/// Efface un emplacement et tout ce qui lui appartient.
///
/// La sauvegarde et ses points de reprise vont ensemble : garder les seconds
/// apres avoir efface la premiere laisserait des instantanes qui ne se
/// rattachent plus a rien, et que rien ne viendrait jamais nettoyer.
pub fn supprimer_emplacement(empreinte: &str, nom: &str) -> Result<(), String> {
    let fichier = chemin(empreinte, nom);
    if fichier.exists() {
        std::fs::remove_file(&fichier).map_err(|e| e.to_string())?;
    }
    let points = dossier_reprises(empreinte, nom);
    if points.exists() {
        std::fs::remove_dir_all(&points).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn dossier_reprises(empreinte: &str, emplacement: &str) -> PathBuf {
    dossier_du_dump(empreinte).join("reprises").join(emplacement)
}

/// Chemin d'un emplacement de sauvegarde.
pub fn chemin(empreinte: &str, nom: &str) -> PathBuf {
    dossier_du_dump(empreinte).join(format!("{}.{}", nom, EXTENSION))
}

/// Retient le dernier emplacement utilise pour un dump donne.
pub fn retenir_emplacement(empreinte: &str, nom: &str) {
    if nom.is_empty() {
        return;
    }
    let dossier = dossier_du_dump(empreinte);
    if std::fs::create_dir_all(&dossier).is_ok() {
        let _ = std::fs::write(dossier.join("dernier-emplacement.txt"), nom);
    }
}

/// Dernier emplacement encore present pour ce dump.
pub fn dernier_emplacement(empreinte: &str) -> Option<String> {
    let nom = std::fs::read_to_string(dossier_du_dump(empreinte).join("dernier-emplacement.txt"))
        .ok()?;
    let nom = nom.trim();
    if nom.is_empty() || !chemin(empreinte, nom).is_file() {
        return None;
    }
    Some(nom.to_string())
}

/// Emplacements existants pour ce dump, par ordre alphabetique.
pub fn emplacements(empreinte: &str) -> Vec<String> {
    let mut noms = Vec::new();
    let Ok(entrees) = std::fs::read_dir(dossier_du_dump(empreinte)) else {
        return noms;
    };
    for entree in entrees.flatten() {
        let chemin = entree.path();
        if chemin.extension().and_then(|e| e.to_str()) != Some(EXTENSION) {
            continue;
        }
        if let Some(nom) = chemin.file_stem().and_then(|s| s.to_str()) {
            noms.push(nom.to_string());
        }
    }
    noms.sort();
    noms
}
