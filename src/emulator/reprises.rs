//! Points de reprise horodates : revenir a une heure anterieure.
//!
//! L'anneau d'instantanes de `etat.rs` est un filet de mise au point : il garde
//! les dernieres secondes et meurt avec la session. Ce journal la est fait pour
//! le joueur. Il prend un point par minute, l'ecrit sur le disque a cote des
//! sauvegardes, et l'elague avec l'age : toutes les minutes sur le dernier quart
//! d'heure, toutes les cinq minutes sur l'heure, toutes les demi heures au dela.
//! Un Tamagotchi mort se rattrape donc a l'heure pres, meme apres avoir eteint
//! l'ordinateur.
//!
//! Le dossier est celui de l'emplacement de sauvegarde : les points suivent la
//! partie et non la console. Deux parties menees sur le meme dump ont chacune
//! leur passe, et revenir en arriere sur l'une ne propose jamais les points de
//! l'autre.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Local};

use crate::emulator::etat::Instantane;
use crate::emulator::Machine;

/// Un point de reprise, tel qu'il est retenu dans l'index.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PointDeReprise {
    /// Heure de l'ordinateur au moment de la prise.
    pub quand: DateTime<Local>,
    /// Pas executes par la console, pour se reperer dans l'emulation.
    pub cycles: u64,
    /// Nom du fichier dans le dossier du journal.
    pub fichier: String,
}

impl PointDeReprise {
    /// Age du point, tel qu'on l'annonce dans la liste.
    pub fn age(&self) -> Duration {
        Local::now().signed_duration_since(self.quand)
    }

    /// Age en toutes lettres, court : « il y a 5 min », « il y a 2 h 10 ».
    pub fn age_lisible(&self) -> String {
        let age = self.age();
        let minutes = age.num_minutes().max(0);
        if minutes < 1 {
            return "a l'instant".to_string();
        }
        if minutes < 60 {
            return format!("il y a {} min", minutes);
        }
        let heures = minutes / 60;
        let reste = minutes % 60;
        if reste == 0 {
            format!("il y a {} h", heures)
        } else {
            format!("il y a {} h {:02}", heures, reste)
        }
    }
}

/// Journal des points de reprise d'une console.
pub struct Journal {
    /// Dossier du journal. Sans lui rien n'est ecrit : c'est le cas tant
    /// qu'aucun dump n'est charge.
    dossier: Option<PathBuf>,
    points: Vec<PointDeReprise>,
    derniere_prise: Option<std::time::Instant>,
}

/// Ecart entre deux prises.
const CADENCE: std::time::Duration = std::time::Duration::from_secs(60);

/// Au dela de cet age, un point est efface.
fn age_maximal() -> Duration {
    Duration::hours(12)
}

/// Ecart minimal entre deux points gardes, selon leur age.
///
/// Le passe proche vaut d'etre fin, le passe lointain non : on garde tout a la
/// minute sur le dernier quart d'heure, puis on espace.
fn pas_minimal(age: Duration) -> Duration {
    if age < Duration::minutes(15) {
        Duration::seconds(55)
    } else if age < Duration::hours(1) {
        Duration::minutes(5)
    } else {
        Duration::minutes(30)
    }
}

impl Default for Journal {
    fn default() -> Self {
        Self { dossier: None, points: Vec::new(), derniere_prise: None }
    }
}

impl Journal {
    /// Ouvre le journal d'une console et relit son index.
    ///
    /// Un point dont le fichier a disparu est oublie : le dossier peut avoir
    /// ete nettoye a la main entre deux lancements.
    pub fn ouvrir(&mut self, dossier: PathBuf) {
        let _ = std::fs::create_dir_all(&dossier);
        let index = dossier.join("index.json");
        let points: Vec<PointDeReprise> = std::fs::read_to_string(&index)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        self.points = points
            .into_iter()
            .filter(|p| dossier.join(&p.fichier).is_file())
            .collect();
        self.points.sort_by_key(|p| p.quand);
        self.dossier = Some(dossier);
        // Le compteur repart : on ne veut pas d'une prise immediate au
        // chargement, la console vient a peine de reprendre.
        self.derniere_prise = Some(std::time::Instant::now());
    }

    /// Ferme le journal, sans rien effacer sur le disque.
    pub fn fermer(&mut self) {
        self.dossier = None;
        self.points.clear();
        self.derniere_prise = None;
    }

    pub fn points(&self) -> &[PointDeReprise] {
        &self.points
    }

    pub fn actif(&self) -> bool {
        self.dossier.is_some()
    }

    /// Prend un point si la cadence est atteinte. A appeler a chaque image.
    ///
    /// La prise recopie la memoire vive et l'ecrit en JSON, ce qui prend
    /// quelques dizaines de millisecondes. Une fois par minute, c'est
    /// imperceptible ; plus souvent, cela se verrait.
    pub fn suivre(&mut self, machine: &Machine) {
        let Some(dossier) = self.dossier.clone() else {
            return;
        };
        if !machine.is_running {
            return;
        }
        let maintenant = std::time::Instant::now();
        if let Some(derniere) = self.derniere_prise {
            if maintenant.duration_since(derniere) < CADENCE {
                return;
            }
        }
        self.derniere_prise = Some(maintenant);

        let quand = Local::now();
        let fichier = format!("{}.tamastate", quand.format("%Y%m%d-%H%M%S"));
        let etat = machine.instantane();
        if etat.ecrire(&dossier.join(&fichier)).is_err() {
            return;
        }
        self.points.push(PointDeReprise { quand, cycles: etat.cycles, fichier });
        self.elaguer(&dossier);
        self.ecrire_index(&dossier);
    }

    /// Prend un point tout de suite, sans attendre la cadence.
    ///
    /// C'est le geste qu'on fait avant une betise : juste avant de tenter
    /// quelque chose, on pose un repere.
    pub fn prendre_maintenant(&mut self, machine: &Machine) -> bool {
        let Some(dossier) = self.dossier.clone() else {
            return false;
        };
        let quand = Local::now();
        let fichier = format!("{}.tamastate", quand.format("%Y%m%d-%H%M%S"));
        let etat = machine.instantane();
        if etat.ecrire(&dossier.join(&fichier)).is_err() {
            return false;
        }
        self.points.push(PointDeReprise { quand, cycles: etat.cycles, fichier });
        self.derniere_prise = Some(std::time::Instant::now());
        self.ecrire_index(&dossier);
        true
    }

    /// Recopie un instantane venu d'ailleurs dans le journal.
    ///
    /// Il est relu avant d'etre adopte : un fichier illisible ne doit pas
    /// entrer dans la liste, on ne s'en apercevrait qu'au moment de s'en
    /// servir. Son heure est celle du fichier, a defaut celle du jour.
    pub fn adopter(&mut self, source: &Path) -> Result<(), String> {
        let Some(dossier) = self.dossier.clone() else {
            return Err("aucune partie ouverte".to_string());
        };
        let etat = Instantane::lire(source)?;
        let quand: DateTime<Local> = std::fs::metadata(source)
            .and_then(|m| m.modified())
            .map(DateTime::from)
            .unwrap_or_else(|_| Local::now());
        let fichier = format!("importe-{}.tamastate", quand.format("%Y%m%d-%H%M%S"));
        std::fs::copy(source, dossier.join(&fichier)).map_err(|e| e.to_string())?;
        self.points.push(PointDeReprise { quand, cycles: etat.cycles, fichier });
        self.points.sort_by_key(|p| p.quand);
        self.ecrire_index(&dossier);
        Ok(())
    }

    /// Relit le point demande.
    pub fn restaurer(&self, indice: usize) -> Option<Instantane> {
        let dossier = self.dossier.as_ref()?;
        let point = self.points.get(indice)?;
        Instantane::lire(&dossier.join(&point.fichier)).ok()
    }

    /// Efface un point, fichier compris.
    pub fn oublier(&mut self, indice: usize) {
        let Some(dossier) = self.dossier.clone() else {
            return;
        };
        if indice >= self.points.len() {
            return;
        }
        let point = self.points.remove(indice);
        let _ = std::fs::remove_file(dossier.join(&point.fichier));
        self.ecrire_index(&dossier);
    }

    /// Espace les points selon leur age, et efface ceux qui tombent.
    fn elaguer(&mut self, dossier: &Path) {
        let maintenant = Local::now();
        let mut gardes: Vec<PointDeReprise> = Vec::new();
        let mut jetes: Vec<PointDeReprise> = Vec::new();
        // Du plus recent au plus ancien : chaque point garde doit etre assez
        // loin du precedent pour l'age auquel il se trouve.
        let mut precedent: Option<DateTime<Local>> = None;
        for point in self.points.iter().rev() {
            let age = maintenant.signed_duration_since(point.quand);
            if age > age_maximal() {
                jetes.push(point.clone());
                continue;
            }
            let assez_loin = match precedent {
                None => true,
                Some(p) => p.signed_duration_since(point.quand) >= pas_minimal(age),
            };
            if assez_loin {
                precedent = Some(point.quand);
                gardes.push(point.clone());
            } else {
                jetes.push(point.clone());
            }
        }
        for point in jetes {
            let _ = std::fs::remove_file(dossier.join(&point.fichier));
        }
        gardes.reverse();
        self.points = gardes;
    }

    fn ecrire_index(&self, dossier: &Path) {
        if let Ok(texte) = serde_json::to_string_pretty(&self.points) {
            let _ = std::fs::write(dossier.join("index.json"), texte);
        }
    }
}
