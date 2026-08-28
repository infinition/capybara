//! Habillage de la coque : papier glisse sous la fenetre transparente, mot de
//! marque, et vitre autour de l'ecran.
//!
//! La console a un cache plastique clair autour de l'ecran, et Bandai livre avec
//! chaque edition des papiers imprimes qu'on glisse dessous pour changer son
//! apparence. C'est prevu par le fabricant, pas un detournement. Ici une image
//! quelconque tient ce role, avec son cadrage.
//!
//! Le mot imprime au dessus de l'ecran et le liseré clair qui entoure la dalle
//! se reglent de la meme facon. L'ensemble suit la console et non la partie,
//! comme le papier suit la coque et non le Tamagotchi.
//!
//! Le fichier garde son ancien nom, `fond.json`, pour ne pas perdre les
//! reglages deja poses.

use std::path::{Path, PathBuf};

/// Habillage d'une console.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Habillage {
    /// Nom du fichier image, dans le dossier de la console. Vide s'il n'y en a
    /// pas : le reste de l'habillage vaut quand meme.
    #[serde(default)]
    pub fichier: String,
    /// Agrandissement du papier. Un remplit la decoupe au plus juste.
    #[serde(default = "un")]
    pub zoom: f32,
    /// Decalage du papier, en fraction de la decoupe.
    #[serde(default)]
    pub dx: f32,
    #[serde(default)]
    pub dy: f32,

    /// Mot imprime au dessus de l'ecran.
    #[serde(default = "vrai")]
    pub titre_visible: bool,
    #[serde(default = "titre_par_defaut")]
    pub titre: String,
    /// Facteur de taille. Un vaut la taille de la console.
    #[serde(default = "un")]
    pub titre_taille: f32,
    /// Couleur du mot. Absente, il prend la couleur d'accent de la coque.
    #[serde(default)]
    pub titre_couleur: Option<[u8; 3]>,

    /// Liseré clair autour de la dalle. Sur la console c'est le bord de la
    /// vitre, tres fin.
    #[serde(default = "vrai")]
    pub vitre_visible: bool,
    /// Epaisseur, en fraction du cote de l'ecran.
    #[serde(default = "epaisseur_par_defaut")]
    pub vitre_epaisseur: f32,
    #[serde(default = "couleur_vitre_par_defaut")]
    pub vitre_couleur: [u8; 3],
}

fn un() -> f32 {
    1.0
}

fn vrai() -> bool {
    true
}

fn titre_par_defaut() -> String {
    "TAMAGOTCHI".to_string()
}

/// Le liseré de la vraie console est mince : un cinquantieme du cote.
fn epaisseur_par_defaut() -> f32 {
    0.018
}

fn couleur_vitre_par_defaut() -> [u8; 3] {
    [248, 249, 251]
}

impl Default for Habillage {
    fn default() -> Self {
        Self {
            fichier: String::new(),
            zoom: 1.0,
            dx: 0.0,
            dy: 0.0,
            titre_visible: true,
            titre: titre_par_defaut(),
            titre_taille: 1.0,
            titre_couleur: None,
            vitre_visible: true,
            vitre_epaisseur: epaisseur_par_defaut(),
            vitre_couleur: couleur_vitre_par_defaut(),
        }
    }
}

/// Fichier de reglage, a cote des sauvegardes de la console.
fn chemin_reglage(dossier: &Path) -> PathBuf {
    dossier.join("fond.json")
}

impl Habillage {
    /// Relit l'habillage d'une console. Rend celui par defaut s'il n'y en a pas.
    ///
    /// Une image qui a disparu du dossier est oubliee, le reste de l'habillage
    /// reste : on ne perd pas un titre parce qu'un fichier a bouge.
    pub fn lire(dossier: &Path) -> Self {
        let mut habillage: Habillage = std::fs::read_to_string(chemin_reglage(dossier))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if !habillage.fichier.is_empty() && !dossier.join(&habillage.fichier).is_file() {
            habillage.fichier.clear();
        }
        habillage
    }

    pub fn ecrire(&self, dossier: &Path) {
        let _ = std::fs::create_dir_all(dossier);
        if let Ok(texte) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(chemin_reglage(dossier), texte);
        }
    }

    /// Retire le papier, en gardant le reste de l'habillage.
    pub fn retirer_le_papier(&mut self, dossier: &Path) {
        if !self.fichier.is_empty() {
            let _ = std::fs::remove_file(dossier.join(&self.fichier));
        }
        self.fichier.clear();
        self.zoom = 1.0;
        self.dx = 0.0;
        self.dy = 0.0;
        self.ecrire(dossier);
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
    // Les anciens papiers d'un autre format n'ont plus lieu d'etre.
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
