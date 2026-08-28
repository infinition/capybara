use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, TextureHandle, Ui};

use crate::emulator::DisplayController;
use crate::gui::ShellColor;

pub struct LcdPanel;

/// Etat d'un bouton pour une image.
///
/// Les deux cas sont distincts : `maintenu` dit que le pointeur reste enfonce,
/// et c'est ce qui permet l'appui long que le jeu attend pour ouvrir son menu.
/// `clique` ne signale qu'un appui bref, qui doit tout de meme durer assez pour
/// que le firmware le voie.
#[derive(Default, Clone, Copy)]
pub struct EtatBouton {
    pub maintenu: bool,
    pub clique: bool,
}

/// Etat des commandes rendu par le panneau pour une image.
#[derive(Default, Clone, Copy)]
pub struct Commandes {
    pub bouton_a: EtatBouton,
    pub bouton_b: EtatBouton,
    pub bouton_c: EtatBouton,
    pub molette: EtatBouton,
    pub molette_tournee: i32,
}

impl LcdPanel {
    /// Dessine la console et rend l'etat de ses commandes.
    ///
    /// L'ecran est une texture, pas seize mille rectangles : le tesseler a
    /// chaque image mangeait tout le temps qui devrait aller a l'emulation.
    pub fn render(
        ui: &mut Ui,
        available_rect: Rect,
        display: &DisplayController,
        ecran: Option<&TextureHandle>,
        shell_color: ShellColor,
    ) -> Commandes {
        let (body_col, shadow_col, accent_col) = shell_color.palette();
        let mut commandes = Commandes::default();

        // La coque suit la fenetre, mais l'ecran et les boutons se placent les
        // uns par rapport aux autres : rien ne doit se chevaucher, quelle que
        // soit la taille.
        let largeur = (available_rect.width() * 0.88).min(400.0);
        let hauteur = (available_rect.height() * 0.94).min(620.0);
        let coque = Rect::from_center_size(available_rect.center(), vec2(largeur, hauteur));
        let arrondi = largeur * 0.16;

        let painter = ui.painter();

        // Anneau porte-cles, puis corps de la coque.
        let anneau = pos2(coque.center().x, coque.min.y + 10.0);
        painter.circle_filled(anneau, 15.0, shadow_col);
        painter.circle_filled(anneau, 7.0, Color32::from_rgb(240, 240, 240));
        painter.rect_filled(coque, arrondi, body_col);
        painter.rect_stroke(coque, arrondi, Stroke::new(5.0_f32, shadow_col));

        painter.text(
            pos2(coque.center().x, coque.min.y + 38.0),
            egui::Align2::CENTER_CENTER,
            "TAMAGOTCHI PARADISE",
            egui::FontId::proportional(12.0),
            Color32::from_rgb(255, 240, 180),
        );

        // L'ecran occupe le haut de la coque, les commandes le bas.
        let marge = 22.0;
        let cote = (largeur - 2.0 * marge)
            .min(hauteur * 0.52)
            .max(64.0);
        let ecran_rect = Rect::from_center_size(
            pos2(coque.center().x, coque.min.y + 58.0 + cote / 2.0),
            vec2(cote, cote),
        );

        painter.rect_filled(ecran_rect.expand(8.0), 10.0, Color32::from_rgb(226, 228, 234));
        painter.rect_stroke(ecran_rect.expand(8.0), 10.0, Stroke::new(3.0_f32, shadow_col));
        match ecran {
            Some(texture) => {
                painter.image(
                    texture.id(),
                    ecran_rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                painter.rect_filled(ecran_rect, 0.0, Color32::BLACK);
            }
        }
        painter.rect_stroke(ecran_rect, 0.0, Stroke::new(1.5_f32, Color32::from_rgb(70, 70, 70)));

        let _ = display;

        // Commandes, sous l'ecran et dans la coque.
        let rayon = (largeur * 0.055).clamp(14.0, 22.0);
        let ligne = (ecran_rect.max.y + 8.0 + (coque.max.y - ecran_rect.max.y) * 0.45)
            .min(coque.max.y - rayon - 16.0);
        let ecart = (largeur * 0.24).min(72.0);

        let bouton = |ui: &mut Ui, centre, etiquette: &str| -> EtatBouton {
            let zone = Rect::from_center_size(centre, vec2(rayon * 2.0, rayon * 2.0));
            let reponse = ui.allocate_rect(zone, Sense::click_and_drag());
            // `is_pointer_button_down_on` reste vrai tant que le pointeur est
            // enfonce sur le bouton : c'est lui qui porte l'appui long.
            let etat = EtatBouton {
                maintenu: reponse.is_pointer_button_down_on(),
                clique: reponse.clicked(),
            };
            let couleur = if etat.maintenu { shadow_col } else { accent_col };
            let peintre = ui.painter();
            peintre.circle_filled(centre, rayon, couleur);
            peintre.circle_stroke(centre, rayon, Stroke::new(2.0_f32, shadow_col));
            peintre.text(
                centre,
                egui::Align2::CENTER_CENTER,
                etiquette,
                egui::FontId::monospace(13.0),
                Color32::BLACK,
            );
            etat
        };

        commandes.bouton_a = bouton(ui, pos2(coque.center().x - ecart, ligne), "A");
        commandes.molette = bouton(ui, pos2(coque.center().x, ligne + 14.0), "OK");
        commandes.bouton_c = bouton(ui, pos2(coque.center().x + ecart, ligne), "C");
        commandes.bouton_b = bouton(
            ui,
            pos2(coque.center().x, (ligne + 14.0 + rayon * 2.6).min(coque.max.y - rayon - 8.0)),
            "B",
        );

        // Molette laterale : elle se tourne a la glissade ou a la roulette.
        let molette = Rect::from_center_size(
            pos2(coque.max.x - 8.0, ecran_rect.center().y),
            vec2(24.0, cote * 0.4),
        );
        let reponse = ui.allocate_rect(molette, Sense::click_and_drag());
        if reponse.dragged() {
            let dy = reponse.drag_delta().y;
            if dy.abs() > 2.0 {
                commandes.molette_tournee += if dy > 0.0 { 1 } else { -1 };
            }
        }
        let roulette = ui.input(|i| i.raw_scroll_delta.y);
        let survol = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| coque.contains(p));
        if roulette != 0.0 && survol {
            commandes.molette_tournee += if roulette > 0.0 { 1 } else { -1 };
        }
        let peintre = ui.painter();
        peintre.rect_filled(molette, 5.0, Color32::from_rgb(220, 225, 235));
        peintre.rect_stroke(molette, 5.0, Stroke::new(2.0_f32, shadow_col));

        commandes
    }
}
