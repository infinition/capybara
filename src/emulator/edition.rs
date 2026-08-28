//! Les cinq editions du Tamagotchi Paradise.
//!
//! Elles partagent le meme SoC, la meme cle de dechiffrement et le meme
//! firmware de base ; ce qui change est le jeu de ressources et la couleur de
//! la coque. Rien dans l'image ne les nomme : la table de biomes en `0x978E8`
//! contient LAND, WATER et SKY dans les cinq dumps, ce n'est donc pas un
//! marqueur. On reconnait donc l'edition au nom du fichier, ce que les dumps
//! publies respectent tous, et l'interface laisse choisir a la main quand le
//! nom ne dit rien.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Edition {
    Earth,
    Water,
    Sky,
    Land,
    JadeForest,
    /// Nom de fichier muet, ou dump d'une autre provenance.
    #[default]
    Inconnue,
}

impl Edition {
    /// Reconnait l'edition au nom du fichier, sans tenir compte de la casse.
    ///
    /// L'ordre compte : « jade forest » doit etre teste avant « land », sinon
    /// un nom qui contient les deux mots serait mal classe.
    pub fn depuis_le_nom(chemin: &Path) -> Self {
        let nom = chemin
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        for (motif, edition) in [
            ("jade", Edition::JadeForest),
            ("forest", Edition::JadeForest),
            ("water", Edition::Water),
            ("earth", Edition::Earth),
            ("sky", Edition::Sky),
            ("land", Edition::Land),
        ] {
            if nom.contains(motif) {
                return edition;
            }
        }
        Edition::Inconnue
    }

    pub fn nom(&self) -> &'static str {
        match self {
            Edition::Earth => "Earth",
            Edition::Water => "Water",
            Edition::Sky => "Sky",
            Edition::Land => "Land",
            Edition::JadeForest => "Jade Forest",
            Edition::Inconnue => "edition inconnue",
        }
    }
}
