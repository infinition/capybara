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
    /// Si l'alarme etait armee et que son echeance est passee pendant ce temps,
    /// on la fait sonner : le firmware retrouvera au demarrage la trace d'un
    /// reveil, et reprendra la partie au lieu de croire a une pile neuve.
    fn remettre_l_horloge(&self, machine: &mut Machine) {
        use super::peripherals::SnSysRegisters as Sys;
        if self.horodatage == 0 && self.compteur == 0 {
            return;
        }
        let ecoule = maintenant().saturating_sub(self.horodatage);
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

/// Dossier des sauvegardes, a cote de l'executable.
///
/// C'est ce que demande l'usage : une copie du logiciel emporte ses parties
/// avec elle. Si l'emplacement de l'executable n'est pas lisible, on se rabat
/// sur le dossier courant plutot que d'echouer.
pub fn dossier_racine() -> PathBuf {
    let base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("sauvegardes")
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

/// Chemin d'un emplacement de sauvegarde.
pub fn chemin(empreinte: &str, nom: &str) -> PathBuf {
    dossier_du_dump(empreinte).join(format!("{}.{}", nom, EXTENSION))
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
