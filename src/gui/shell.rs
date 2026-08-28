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

/// Les couleurs d'une coque.
///
/// Sur la console, les boutons et la molette ne sont pas de la couleur du
/// corps : ils tranchent, en vert d'eau sur la rose, en violet sur la bleue.
/// C'est `accent` qui porte cette seconde couleur. `motif` est celle du decor
/// imprime autour de l'ecran, propre au theme de chaque edition.
pub struct Palette {
    pub corps: Color32,
    pub calotte: Color32,
    pub ombre: Color32,
    pub bouton: Color32,
    pub accent: Color32,
    pub motif: Color32,
}

impl ShellColor {
    pub fn palette(&self) -> (Color32, Color32, Color32) {
        let p = self.couleurs();
        (p.corps, p.ombre, p.bouton)
    }

    pub fn couleurs(&self) -> Palette {
        match self {
            ShellColor::PinkLand => Palette {
                corps: Color32::from_rgb(243, 138, 174),
                calotte: Color32::from_rgb(252, 196, 216),
                ombre: Color32::from_rgb(198, 90, 132),
                bouton: Color32::from_rgb(96, 208, 190),
                accent: Color32::from_rgb(72, 198, 178),
                motif: Color32::from_rgb(226, 96, 140),
            },
            ShellColor::BlueWater => Palette {
                corps: Color32::from_rgb(112, 190, 234),
                calotte: Color32::from_rgb(178, 224, 248),
                ombre: Color32::from_rgb(52, 122, 180),
                bouton: Color32::from_rgb(128, 122, 208),
                accent: Color32::from_rgb(118, 112, 202),
                motif: Color32::from_rgb(74, 152, 206),
            },
            ShellColor::PurpleSky => Palette {
                corps: Color32::from_rgb(174, 152, 224),
                calotte: Color32::from_rgb(216, 202, 246),
                ombre: Color32::from_rgb(112, 88, 168),
                bouton: Color32::from_rgb(244, 154, 196),
                accent: Color32::from_rgb(238, 140, 188),
                motif: Color32::from_rgb(140, 116, 200),
            },
            ShellColor::JadeForest => Palette {
                corps: Color32::from_rgb(96, 190, 146),
                calotte: Color32::from_rgb(178, 228, 196),
                ombre: Color32::from_rgb(48, 128, 96),
                bouton: Color32::from_rgb(250, 214, 96),
                accent: Color32::from_rgb(246, 206, 80),
                motif: Color32::from_rgb(62, 152, 116),
            },
            ShellColor::OrangeTropics => Palette {
                corps: Color32::from_rgb(246, 152, 82),
                calotte: Color32::from_rgb(255, 206, 152),
                ombre: Color32::from_rgb(192, 100, 36),
                bouton: Color32::from_rgb(80, 200, 200),
                accent: Color32::from_rgb(64, 190, 192),
                motif: Color32::from_rgb(226, 116, 60),
            },
            ShellColor::WhiteGlacier => Palette {
                corps: Color32::from_rgb(234, 242, 250),
                calotte: Color32::from_rgb(252, 253, 255),
                ombre: Color32::from_rgb(154, 176, 200),
                bouton: Color32::from_rgb(150, 206, 240),
                accent: Color32::from_rgb(138, 198, 236),
                motif: Color32::from_rgb(168, 200, 228),
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
