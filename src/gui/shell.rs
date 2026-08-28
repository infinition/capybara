use egui::Color32;

use crate::emulator::Edition;

/// Couleur de coque, une par edition sortie.
///
/// Bandai a sorti six coques entre juillet 2025 et juillet 2026 : Pink Land,
/// Blue Water et Purple Sky pour la premiere vague, Jade Forest pour la
/// deuxieme, Orange Tropics et White Glacier pour la troisieme. La coque
/// ressemble a celle de la Tamagotchi Pix : sa moitie haute porte un motif de
/// coquille fendue et n'est pas de la meme couleur que le corps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellColor {
    PinkLand,
    BlueWater,
    PurpleSky,
    JadeForest,
    OrangeTropics,
    WhiteGlacier,
}

/// Les couleurs d'une coque : le corps, sa moitie haute, l'ombre des reliefs et
/// la teinte des boutons.
pub struct Palette {
    pub corps: Color32,
    pub calotte: Color32,
    pub ombre: Color32,
    pub bouton: Color32,
}

impl ShellColor {
    pub fn palette(&self) -> (Color32, Color32, Color32) {
        let p = self.couleurs();
        (p.corps, p.ombre, p.bouton)
    }

    pub fn couleurs(&self) -> Palette {
        match self {
            ShellColor::PinkLand => Palette {
                corps: Color32::from_rgb(244, 150, 186),
                calotte: Color32::from_rgb(255, 205, 224),
                ombre: Color32::from_rgb(186, 88, 130),
                bouton: Color32::from_rgb(255, 240, 170),
            },
            ShellColor::BlueWater => Palette {
                corps: Color32::from_rgb(78, 165, 226),
                calotte: Color32::from_rgb(160, 216, 246),
                ombre: Color32::from_rgb(40, 105, 165),
                bouton: Color32::from_rgb(255, 235, 150),
            },
            ShellColor::PurpleSky => Palette {
                corps: Color32::from_rgb(160, 136, 214),
                calotte: Color32::from_rgb(210, 194, 244),
                ombre: Color32::from_rgb(102, 78, 158),
                bouton: Color32::from_rgb(255, 236, 170),
            },
            ShellColor::JadeForest => Palette {
                corps: Color32::from_rgb(88, 186, 140),
                calotte: Color32::from_rgb(170, 226, 190),
                ombre: Color32::from_rgb(44, 122, 92),
                bouton: Color32::from_rgb(255, 238, 160),
            },
            ShellColor::OrangeTropics => Palette {
                corps: Color32::from_rgb(244, 154, 78),
                calotte: Color32::from_rgb(255, 208, 150),
                ombre: Color32::from_rgb(186, 96, 32),
                bouton: Color32::from_rgb(255, 244, 190),
            },
            ShellColor::WhiteGlacier => Palette {
                corps: Color32::from_rgb(232, 240, 248),
                calotte: Color32::from_rgb(252, 253, 255),
                ombre: Color32::from_rgb(150, 172, 196),
                bouton: Color32::from_rgb(196, 232, 250),
            },
        }
    }

    pub fn nom(&self) -> &'static str {
        match self {
            ShellColor::PinkLand => "Pink Land",
            ShellColor::BlueWater => "Blue Water",
            ShellColor::PurpleSky => "Purple Sky",
            ShellColor::JadeForest => "Jade Forest",
            ShellColor::OrangeTropics => "Orange Tropics",
            ShellColor::WhiteGlacier => "White Glacier",
        }
    }

    pub const TOUTES: [ShellColor; 6] = [
        ShellColor::PinkLand,
        ShellColor::BlueWater,
        ShellColor::PurpleSky,
        ShellColor::JadeForest,
        ShellColor::OrangeTropics,
        ShellColor::WhiteGlacier,
    ];

    /// Coque d'origine d'une edition.
    ///
    /// Les dumps publies portent les noms de champs, Land, Water, Sky et Jade
    /// Forest ; le dump nomme Earth n'a pas d'equivalent commercial connu, on
    /// lui laisse la coque verte de la foret, la plus proche d'une planete.
    pub fn pour_edition(edition: Edition) -> Self {
        match edition {
            Edition::Land => ShellColor::PinkLand,
            Edition::Water => ShellColor::BlueWater,
            Edition::Sky => ShellColor::PurpleSky,
            Edition::JadeForest => ShellColor::JadeForest,
            Edition::Earth => ShellColor::JadeForest,
            Edition::Inconnue => ShellColor::BlueWater,
        }
    }
}
