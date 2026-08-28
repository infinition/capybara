//! Papiers de personnalisation glisses sous la fenetre transparente.
//!
//! La console a un cache plastique clair autour de l'ecran, et Bandai livre
//! avec chaque edition des papiers imprimes qu'on glisse dessous pour changer
//! son apparence. C'est prevu par le fabricant, pas un detournement.
//!
//! Ici, une image quelconque tient ce role. Elle est recopiee dans le dossier
//! de la console, avec son cadrage : un zoom et un decalage, ce qui suffit a
//! placer n'importe quelle image dans la decoupe. Le reglage suit la console et
//! non la partie, comme le papier suit la coque et non le Tamagotchi.

use std::path::{Path, PathBuf};

/// Cadrage d'un papier sous la fenetre.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Fond {
    /// Nom du fichier image, dans le dossier de la console.
    pub fichier: String,
    /// Agrandissement. Un remplit la decoupe au plus juste.
    pub zoom: f32,
    /// Decalage, en fraction de la decoupe.
    pub dx: f32,
    pub dy: f32,
}

impl Default for Fond {
    fn default() -> Self {
        Self { fichier: String::new(), zoom: 1.0, dx: 0.0, dy: 0.0 }
    }
}

/// Fichier de reglage, a cote des sauvegardes de la console.
fn chemin_reglage(dossier: &Path) -> PathBuf {
    dossier.join("fond.json")
}

impl Fond {
    pub fn lire(dossier: &Path) -> Option<Self> {
        let texte = std::fs::read_to_string(chemin_reglage(dossier)).ok()?;
        let fond: Fond = serde_json::from_str(&texte).ok()?;
        if fond.fichier.is_empty() || !dossier.join(&fond.fichier).is_file() {
            return None;
        }
        Some(fond)
    }

    pub fn ecrire(&self, dossier: &Path) {
        let _ = std::fs::create_dir_all(dossier);
        if let Ok(texte) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(chemin_reglage(dossier), texte);
        }
    }

    /// Efface le reglage et l'image recopiee.
    pub fn effacer(dossier: &Path, fichier: &str) {
        let _ = std::fs::remove_file(chemin_reglage(dossier));
        if !fichier.is_empty() {
            let _ = std::fs::remove_file(dossier.join(fichier));
        }
    }
}

/// Recopie une image dans le dossier de la console et rend son nom.
///
/// L'image est gardee telle quelle : on ne la reencode pas, ce qui evite d'en
/// perdre la qualite et de choisir un format a la place de l'utilisateur.
pub fn adopter_image(source: &Path, dossier: &Path) -> Result<String, String> {
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let nom = format!("fond.{}", extension);
    std::fs::create_dir_all(dossier).map_err(|e| e.to_string())?;
    // Les anciens fonds d'un autre format n'ont plus lieu d'etre.
    for autre in ["png", "jpg", "jpeg", "bmp", "gif", "webp"] {
        if autre != extension {
            let _ = std::fs::remove_file(dossier.join(format!("fond.{}", autre)));
        }
    }
    std::fs::copy(source, dossier.join(&nom)).map_err(|e| e.to_string())?;
    Ok(nom)
}

/// Charge une image et la rend prete pour egui, avec son rapport largeur sur
/// hauteur.
///
/// Une image immense n'apporte rien dans une decoupe de deux cents pixels : on
/// la reduit avant d'en faire une texture, ce qui epargne la memoire video.
pub fn charger_image(chemin: &Path) -> Result<(egui::ColorImage, f32), String> {
    const COTE_MAX: u32 = 1024;
    let image = image::open(chemin).map_err(|e| e.to_string())?;
    let (largeur, hauteur) = (image.width().max(1), image.height().max(1));
    let rapport = largeur as f32 / hauteur as f32;
    let image = if largeur > COTE_MAX || hauteur > COTE_MAX {
        image.resize(COTE_MAX, COTE_MAX, image::imageops::FilterType::Lanczos3)
    } else {
        image
    };
    let rgba = image.to_rgba8();
    let taille = [rgba.width() as usize, rgba.height() as usize];
    Ok((egui::ColorImage::from_rgba_unmultiplied(taille, rgba.as_raw()), rapport))
}
