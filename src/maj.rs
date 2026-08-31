//! Verification des mises a jour, a la demande de l'utilisateur.
//!
//! Rien n'est verifie tout seul au demarrage : le logiciel n'a pas a joindre
//! un serveur sans qu'on le lui demande. Le bouton interroge la derniere
//! publication du depot, compare son numero a celui de l'executable, et se
//! contente de le dire. Aucun telechargement, aucune installation.
//!
//! L'appel se fait dans un fil dedie : une liaison lente ou coupee ne doit pas
//! figer l'interface, et le delai est borne.

use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Page du projet, celle qu'ouvre le lien.
pub const DEPOT: &str = "https://github.com/infinition/capybara";
/// Page de soutien.
pub const SOUTIEN: &str = "https://buymeacoffee.com/infinition";
/// Derniere publication, au format que rend l'interface du depot.
const DERNIERE_PUBLICATION: &str =
    "https://api.github.com/repos/infinition/capybara/releases/latest";
/// Au dela, on renonce plutot que de laisser le fil pendre.
const DELAI: Duration = Duration::from_secs(10);

/// Ou en est la verification.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EtatMaj {
    #[default]
    Jamais,
    EnCours,
    /// Rien de plus recent que la version installee.
    AJour,
    /// Une publication porte un numero superieur.
    Disponible {
        version: String,
        page: String,
    },
    Echec(String),
}

/// Verificateur de mises a jour. Il ne garde qu'un etat partage avec son fil.
pub struct Maj {
    etat: Arc<Mutex<EtatMaj>>,
}

impl Default for Maj {
    fn default() -> Self {
        Self {
            etat: Arc::new(Mutex::new(EtatMaj::Jamais)),
        }
    }
}

impl Maj {
    /// Version de l'executable, celle du manifeste.
    pub fn version_installee() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    pub fn etat(&self) -> EtatMaj {
        self.etat.lock().map(|e| e.clone()).unwrap_or_default()
    }

    /// Lance la verification, sauf si une est deja en route.
    pub fn verifier(&self) {
        {
            let Ok(mut etat) = self.etat.lock() else {
                return;
            };
            if *etat == EtatMaj::EnCours {
                return;
            }
            *etat = EtatMaj::EnCours;
        }
        let partage = Arc::clone(&self.etat);
        std::thread::spawn(move || {
            let resultat = interroger();
            if let Ok(mut etat) = partage.lock() {
                *etat = resultat;
            }
        });
    }
}

fn interroger() -> EtatMaj {
    // L'interface du depot exige un nom d'agent, sans quoi elle refuse.
    let reponse = ureq::get(DERNIERE_PUBLICATION)
        .set("User-Agent", "Capybara")
        .set("Accept", "application/vnd.github+json")
        .timeout(DELAI)
        .call();
    let corps = match reponse {
        Ok(r) => match r.into_string() {
            Ok(texte) => texte,
            Err(e) => return EtatMaj::Echec(e.to_string()),
        },
        Err(e) => return EtatMaj::Echec(e.to_string()),
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&corps) else {
        return EtatMaj::Echec("reponse illisible".to_string());
    };
    let Some(marque) = json.get("tag_name").and_then(|v| v.as_str()) else {
        // Un depot sans publication rend un objet d'erreur, pas une version.
        return EtatMaj::Echec("aucune publication".to_string());
    };
    let page = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or(DEPOT)
        .to_string();
    if plus_recente(marque, Maj::version_installee()) {
        EtatMaj::Disponible {
            version: marque.trim_start_matches(['v', 'V']).to_string(),
            page,
        }
    } else {
        EtatMaj::AJour
    }
}

/// Les trois premiers nombres d'un numero de version.
///
/// Les marques de publication s'ecrivent `v1.2.3` aussi souvent que `1.2.3`, et
/// ce qui suit un tiret est un pre-lancement dont on ne tient pas compte.
fn numeros(version: &str) -> [u32; 3] {
    let nettoye = version.trim().trim_start_matches(['v', 'V']);
    let base = nettoye.split(['-', '+']).next().unwrap_or(nettoye);
    let mut sortie = [0u32; 3];
    for (i, part) in base.split('.').take(3).enumerate() {
        sortie[i] = part.parse().unwrap_or(0);
    }
    sortie
}

fn plus_recente(distante: &str, installee: &str) -> bool {
    numeros(distante) > numeros(installee)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_numeros_se_comparent_dans_le_bon_ordre() {
        assert!(plus_recente("v1.5.0", "1.4.9"));
        assert!(plus_recente("2.0.0", "v1.99.99"));
        assert!(plus_recente("1.4.10", "1.4.9"));
        assert!(!plus_recente("1.4.1", "1.4.1"));
        assert!(!plus_recente("v1.0.0", "1.0.1"));
    }

    #[test]
    fn une_marque_mal_formee_ne_declenche_rien() {
        assert!(!plus_recente("nightly", "0.1.0"));
        assert_eq!(numeros("v1.2"), [1, 2, 0]);
        assert_eq!(numeros("1.2.3-rc1"), [1, 2, 3]);
    }
}
