//! Reglage des touches du clavier.
//!
//! Une commande peut porter plusieurs touches, et une touche ne sert qu'a une
//! commande. Les combinaisons de la console, A plus C par exemple, continuent
//! de marcher : chaque broche est lue separement, tenir deux touches tient deux
//! boutons.

use egui::{Color32, RichText, Ui};

use crate::i18n::I18n;
use crate::touches::{Bouton, Commande, Souris, Touches};

/// Dessine le panneau. Rend vrai quand la table vient de changer, pour que
/// l'appelant l'enregistre.
pub fn dessiner(
    ui: &mut Ui,
    touches: &mut Touches,
    souris: &mut Souris,
    capture: &mut Option<Commande>,
    i18n: &I18n,
) -> bool {
    let mut change = false;
    ui.group(|ui| {
        ui.label(RichText::new(i18n.choisir("Commandes clavier", "Keyboard controls")).strong());
        ui.label(
            RichText::new(i18n.choisir(
                "Plusieurs touches par commande. Cliquez sur une touche pour la retirer. Tenir deux boutons ensemble fonctionne, c'est ainsi qu'on ranime le personnage.",
                "Several keys per control. Click a key to remove it. Holding two buttons together works, which is how you revive the character.",
            ))
            .small()
            .color(Color32::GRAY),
        );
        ui.add_space(4.0);

        for commande in Commande::TOUTES {
            let (fr, en) = commande.libelle();
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(i18n.choisir(fr, en)).strong());
                let noms: Vec<String> = touches.noms(commande).to_vec();
                if noms.is_empty() {
                    ui.label(
                        RichText::new(i18n.choisir("aucune", "none"))
                            .small()
                            .color(Color32::from_rgb(230, 180, 90)),
                    );
                }
                for nom in noms {
                    if ui
                        .small_button(&nom)
                        .on_hover_text(i18n.choisir("Retirer", "Remove"))
                        .clicked()
                    {
                        touches.retirer(commande, &nom);
                        change = true;
                    }
                }
                let en_attente = *capture == Some(commande);
                let libelle = if en_attente {
                    i18n.choisir("frappez une touche...", "press a key...")
                } else {
                    i18n.choisir("Ajouter", "Add")
                };
                if ui
                    .selectable_label(en_attente, libelle)
                    .on_hover_text(i18n.choisir(
                        "La prochaine touche frappee sera ajoutee. Echap annule.",
                        "The next key pressed will be added. Escape cancels.",
                    ))
                    .clicked()
                {
                    *capture = if en_attente { None } else { Some(commande) };
                }
                if ui
                    .small_button(i18n.choisir("Par defaut", "Reset"))
                    .clicked()
                {
                    touches.reinitialiser(commande);
                    change = true;
                }
            });
        }
    });

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.label(RichText::new(i18n.choisir("Souris sur l'ecran", "Mouse on the screen")).strong());
        ui.label(
            RichText::new(i18n.choisir(
                "Cliquer n'importe ou sur l'ecran declenche une commande, sans avoir a viser les petits boutons.",
                "Clicking anywhere on the screen triggers a control, with no need to aim at the small buttons.",
            ))
            .small()
            .color(Color32::GRAY),
        );
        for (titre_fr, titre_en, champ) in [
            ("Clic gauche", "Left click", 0usize),
            ("Clic droit", "Right click", 1),
            ("Clic molette", "Middle click", 2),
        ] {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(i18n.choisir(titre_fr, titre_en)).strong());
                let courant = match champ {
                    0 => souris.primaire,
                    1 => souris.secondaire,
                    _ => souris.milieu,
                };
                for bouton in Bouton::TOUS {
                    let (fr, en) = bouton.libelle();
                    if ui
                        .selectable_label(courant == bouton, i18n.choisir(fr, en))
                        .clicked()
                        && courant != bouton
                    {
                        match champ {
                            0 => souris.primaire = bouton,
                            1 => souris.secondaire = bouton,
                            _ => souris.milieu = bouton,
                        }
                        change = true;
                    }
                }
            });
        }
    });
    change
}
