//! Habillage de la coque : papier glisse sous la fenetre transparente, masque
//! de decoupe, mot de marque, vitre autour de l'ecran et couleur des commandes.
//!
//! La console a un cache plastique clair autour de l'ecran, et Bandai livre avec
//! chaque edition des papiers imprimes qu'on glisse dessous pour changer son
//! apparence. C'est prevu par le fabricant, pas un detournement. Ici une image
//! quelconque tient ce role.
//!
//! Le masque va plus loin que la vraie machine : c'est une image en noir et
//! blanc dont le noir dit ou le papier se voit. Il permet de donner a la
//! decoupe la forme qu'on veut, et de couvrir la coque entiere si on le
//! souhaite. Papier et masque ont chacun leur zoom, leur decalage et leur
//! rotation, et l'ensemble est cuit dans une seule texture a chaque changement :
//! egui ne sait pas rogner une image sur une autre.
//!
//! Tout suit la console et non la partie, comme le papier de la vraie machine
//! suit la coque et non le Tamagotchi. Le fichier garde son ancien nom,
//! `fond.json`, pour ne pas perdre les reglages deja poses.

use std::path::{Path, PathBuf};

/// Cadrage d'une image posee sur la coque.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Cadrage {
    /// Agrandissement. Un remplit la zone au plus juste.
    #[serde(default = "un")]
    pub zoom: f32,
    /// Decalage, en fraction de la zone.
    #[serde(default)]
    pub dx: f32,
    #[serde(default)]
    pub dy: f32,
    /// Rotation, en degres.
    #[serde(default)]
    pub rotation: f32,
}

impl Default for Cadrage {
    fn default() -> Self {
        Self { zoom: 1.0, dx: 0.0, dy: 0.0, rotation: 0.0 }
    }
}

impl Cadrage {
    fn recentrer(&mut self) {
        *self = Self::default();
    }
}

/// Habillage d'une console.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Habillage {
    /// Nom du fichier du papier, dans le dossier de la console. Vide s'il n'y
    /// en a pas : le reste de l'habillage vaut quand meme.
    #[serde(default)]
    pub fichier: String,
    #[serde(default)]
    pub papier: Cadrage,

    /// Nom du fichier du masque de decoupe. Vide, c'est la fenetre transparente
    /// de la console qui donne la forme.
    #[serde(default)]
    pub masque: String,
    #[serde(default)]
    pub masque_cadrage: Cadrage,

    /// Fond de la coque entiere, sous la fenetre et derriere tout le reste.
    /// C'est une couche a part : elle et le papier de la fenetre se voient
    /// ensemble, l'une derriere l'autre.
    #[serde(default)]
    pub coque_fichier: String,
    #[serde(default)]
    pub coque_cadrage: Cadrage,
    /// La calotte, la moitie haute de la coque, est couverte elle aussi.
    #[serde(default)]
    pub inclut_le_chapeau: bool,
    /// Papier propre a la calotte, quand elle n'est pas couverte par l'autre.
    #[serde(default)]
    pub chapeau_fichier: String,
    #[serde(default)]
    pub chapeau_cadrage: Cadrage,
    /// Couleur unie de la calotte. Absente, elle suit la coque.
    #[serde(default)]
    pub chapeau_couleur: Option<[u8; 3]>,

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

    /// Taille de la dalle seule, en facteur de celle de la console.
    #[serde(default = "un")]
    pub ecran_taille: f32,
    /// Position verticale de la dalle, en fraction de la hauteur de coque.
    #[serde(default)]
    pub ecran_dy: f32,
    /// Taille de la fenetre transparente autour de la dalle. Elle a ses propres
    /// reglages : agrandir l'ecran ne doit pas agrandir son cadre.
    #[serde(default = "un")]
    pub fenetre_taille: f32,
    #[serde(default)]
    pub fenetre_dy: f32,

    /// Couleurs des commandes. Absentes, elles suivent la coque.
    #[serde(default)]
    pub bouton_couleur: Option<[u8; 3]>,
    #[serde(default)]
    pub molette_couleur: Option<[u8; 3]>,
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
            papier: Cadrage::default(),
            masque: String::new(),
            masque_cadrage: Cadrage::default(),
            coque_fichier: String::new(),
            coque_cadrage: Cadrage::default(),
            inclut_le_chapeau: false,
            chapeau_fichier: String::new(),
            chapeau_cadrage: Cadrage::default(),
            chapeau_couleur: None,
            titre_visible: true,
            titre: titre_par_defaut(),
            titre_taille: 1.0,
            titre_couleur: None,
            vitre_visible: true,
            vitre_epaisseur: epaisseur_par_defaut(),
            vitre_couleur: couleur_vitre_par_defaut(),
            ecran_taille: 1.0,
            ecran_dy: 0.0,
            fenetre_taille: 1.0,
            fenetre_dy: 0.0,
            bouton_couleur: None,
            molette_couleur: None,
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
    /// Une image disparue du dossier est oubliee, le reste de l'habillage
    /// reste : on ne perd pas un titre parce qu'un fichier a bouge.
    pub fn lire(dossier: &Path) -> Self {
        let mut habillage: Habillage = std::fs::read_to_string(chemin_reglage(dossier))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if !habillage.fichier.is_empty() && !dossier.join(&habillage.fichier).is_file() {
            habillage.fichier.clear();
        }
        if !habillage.masque.is_empty() && !dossier.join(&habillage.masque).is_file() {
            habillage.masque.clear();
        }
        if !habillage.chapeau_fichier.is_empty()
            && !dossier.join(&habillage.chapeau_fichier).is_file()
        {
            habillage.chapeau_fichier.clear();
        }
        if !habillage.coque_fichier.is_empty()
            && !dossier.join(&habillage.coque_fichier).is_file()
        {
            habillage.coque_fichier.clear();
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
        self.papier.recentrer();
        self.ecrire(dossier);
    }

    /// Retire le fond de coque.
    pub fn retirer_le_fond(&mut self, dossier: &Path) {
        if !self.coque_fichier.is_empty() {
            let _ = std::fs::remove_file(dossier.join(&self.coque_fichier));
        }
        self.coque_fichier.clear();
        self.coque_cadrage.recentrer();
        self.ecrire(dossier);
    }

    /// Retire le papier de la calotte.
    pub fn retirer_le_chapeau(&mut self, dossier: &Path) {
        if !self.chapeau_fichier.is_empty() {
            let _ = std::fs::remove_file(dossier.join(&self.chapeau_fichier));
        }
        self.chapeau_fichier.clear();
        self.chapeau_cadrage.recentrer();
        self.ecrire(dossier);
    }

    /// Retire le masque : la decoupe revient a celle de la console.
    pub fn retirer_le_masque(&mut self, dossier: &Path) {
        if !self.masque.is_empty() {
            let _ = std::fs::remove_file(dossier.join(&self.masque));
        }
        self.masque.clear();
        self.masque_cadrage.recentrer();
        self.ecrire(dossier);
    }
}

/// Recopie une image dans le dossier de la console sous un nom donne, et rend
/// le nom complet avec son extension.
///
/// L'image est gardee telle quelle : on ne la reencode pas, ce qui evite d'en
/// perdre la qualite et de choisir un format a la place de l'utilisateur.
pub fn adopter_image(source: &Path, dossier: &Path, base: &str) -> Result<String, String> {
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let nom = format!("{}.{}", base, extension);
    std::fs::create_dir_all(dossier).map_err(|e| e.to_string())?;
    // Les anciennes versions d'un autre format n'ont plus lieu d'etre.
    for autre in ["png", "jpg", "jpeg", "bmp", "gif", "webp"] {
        if autre != extension {
            let _ = std::fs::remove_file(dossier.join(format!("{}.{}", base, autre)));
        }
    }
    std::fs::copy(source, dossier.join(&nom)).map_err(|e| e.to_string())?;
    Ok(nom)
}

/// Charge une image et la reduit si elle est immense.
///
/// Une image de huit mille pixels n'apporte rien dans une decoupe de deux
/// cents : on la reduit avant de s'en servir, ce qui epargne la memoire et le
/// temps de composition.
pub fn charger_image(chemin: &Path) -> Result<image::RgbaImage, String> {
    const COTE_MAX: u32 = 1024;
    let image = image::open(chemin).map_err(|e| e.to_string())?;
    let image = if image.width() > COTE_MAX || image.height() > COTE_MAX {
        image.resize(COTE_MAX, COTE_MAX, image::imageops::FilterType::Lanczos3)
    } else {
        image
    };
    Ok(image.to_rgba8())
}

/// Cote de la texture composee. Assez fin pour une coque de six cents pixels,
/// assez petit pour se recalculer sans qu'on le voie.
pub const COTE_COMPOSE: usize = 512;

/// Cuit le papier et son masque dans une seule image, dans le repere de la
/// coque.
///
/// egui ne sait pas rogner une image sur une autre : la transparence est donc
/// calculee ici, une fois par changement de reglage, et non a chaque image.
/// Le repere est le carre englobant de la coque, l'origine au centre, ce qui
/// permet ensuite de poser la texture telle quelle sur n'importe quelle forme.
///
/// Le noir du masque laisse voir le papier, le blanc le cache, et ce qui tombe
/// hors du masque est cache aussi : c'est lui qui donne la forme.
pub fn composer(
    papier: &image::RgbaImage,
    cadrage: &Cadrage,
    masque: Option<(&image::RgbaImage, &Cadrage)>,
) -> egui::ColorImage {
    let cote = COTE_COMPOSE;
    let mut pixels = vec![egui::Color32::TRANSPARENT; cote * cote];
    for y in 0..cote {
        for x in 0..cote {
            // Position dans le repere de la coque, de moins un demi a un demi.
            let px = x as f32 / cote as f32 - 0.5;
            let py = y as f32 / cote as f32 - 0.5;

            let alpha = match masque {
                Some((masque, cadrage_masque)) => match echantillonner(masque, px, py, cadrage_masque) {
                    Some(c) => {
                        // Le noir laisse passer. On prend la clarte, moyenne
                        // simple des trois composantes, et on l'inverse.
                        let clarte =
                            (c[0] as u32 + c[1] as u32 + c[2] as u32) / 3;
                        let opacite = 255u32.saturating_sub(clarte);
                        // Le masque peut lui meme etre transparent : ce qui est
                        // transparent ne laisse rien passer.
                        (opacite * c[3] as u32 / 255) as u8
                    }
                    None => 0,
                },
                None => 255,
            };
            if alpha == 0 {
                continue;
            }
            let Some(c) = echantillonner(papier, px, py, cadrage) else {
                continue;
            };
            let a = (alpha as u32 * c[3] as u32 / 255) as u8;
            pixels[y * cote + x] = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], a);
        }
    }
    egui::ColorImage { size: [cote, cote], pixels }
}

/// Lit une image a une position du repere de la coque, cadrage applique.
///
/// Rend `None` hors de l'image : c'est ce qui permet d'agrandir un masque au
/// dela de la coque sans que ses bords se repetent.
fn echantillonner(
    image: &image::RgbaImage,
    px: f32,
    py: f32,
    cadrage: &Cadrage,
) -> Option<[u8; 4]> {
    // On defait le cadrage : d'abord le decalage, puis la rotation, puis le
    // zoom. L'image garde ses proportions, sa plus petite dimension remplissant
    // le repere.
    let (mut x, mut y) = (px - cadrage.dx, py - cadrage.dy);
    let angle = -cadrage.rotation.to_radians();
    let (s, c) = angle.sin_cos();
    let (rx, ry) = (x * c - y * s, x * s + y * c);
    let zoom = cadrage.zoom.max(0.02);
    x = rx / zoom;
    y = ry / zoom;

    let (l, h) = (image.width().max(1) as f32, image.height().max(1) as f32);
    let rapport = l / h;
    let (ex, ey) = if rapport >= 1.0 { (rapport, 1.0) } else { (1.0, 1.0 / rapport) };
    let u = x / ex + 0.5;
    let v = y / ey + 0.5;
    if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
        return None;
    }
    let ix = (u * l) as u32;
    let iy = (v * h) as u32;
    Some(image.get_pixel(ix.min(image.width() - 1), iy.min(image.height() - 1)).0)
}
