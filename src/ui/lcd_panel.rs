use egui::{pos2, vec2, Color32, Pos2, Rect, Sense, Shape, Stroke, TextureHandle, Ui};

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

/// Contour d'un oeuf, du sommet et dans le sens des aiguilles.
///
/// La coque de la Paradise reprend celle de la Tamagotchi Pix : un oeuf plus
/// etroit en haut qu'en bas. Le facteur de largeur suit le cosinus de l'angle,
/// ce qui resserre le sommet et elargit la base sans casser la convexite, dont
/// depend le remplissage d'egui.
fn contour_oeuf(centre: Pos2, largeur: f32, hauteur: f32, cotes: usize) -> Vec<Pos2> {
    (0..cotes)
        .map(|i| {
            let t = i as f32 / cotes as f32 * std::f32::consts::TAU;
            let resserrement = 1.0 - 0.16 * t.cos();
            pos2(
                centre.x + largeur * 0.5 * t.sin() * resserrement,
                centre.y - hauteur * 0.5 * t.cos(),
            )
        })
        .collect()
}

impl LcdPanel {
    /// Dessine la console et rend l'etat de ses commandes.
    ///
    /// L'ecran est une texture, pas seize mille rectangles : le tesseler a
    /// chaque image mangeait tout le temps qui devrait aller a l'emulation.
    ///
    /// `angle_molette` sert a animer les deux fleches de la fenetre
    /// transparente. C'est l'appelant qui le garde d'une image a l'autre, la
    /// molette continuant de tourner un peu apres le geste.
    pub fn render(
        ui: &mut Ui,
        available_rect: Rect,
        display: &DisplayController,
        ecran: Option<&TextureHandle>,
        shell_color: ShellColor,
        angle_molette: f32,
    ) -> Commandes {
        let couleurs = shell_color.couleurs();
        let (corps_col, calotte_col, ombre_col, bouton_col) =
            (couleurs.corps, couleurs.calotte, couleurs.ombre, couleurs.bouton);
        let (accent_col, motif_col) = (couleurs.accent, couleurs.motif);
        let mut commandes = Commandes::default();

        // La coque suit la fenetre, mais l'ecran et les boutons se placent les
        // uns par rapport aux autres : rien ne doit se chevaucher, quelle que
        // soit la taille. On laisse de la place a droite pour l'antenne.
        // La console mesure 7,5 cm de haut pour 6,5 de large. Les deux cotes
        // etaient calcules separement, chacun sur sa dimension de fenetre : la
        // coque s'ecrasait ou s'etirait selon la forme de celle ci. La hauteur
        // commande maintenant, la largeur en decoule, et la molette qui deborde
        // a droite est comptee dans la place a tenir.
        const RAPPORT: f32 = 6.5 / 7.5;
        const DEBORD_MOLETTE: f32 = 1.15;
        let hauteur = (available_rect.height() * 0.96)
            .min(available_rect.width() * 0.96 / (RAPPORT * DEBORD_MOLETTE))
            .min(680.0);
        let largeur = hauteur * RAPPORT;
        let centre = pos2(available_rect.center().x - largeur * 0.06, available_rect.center().y);
        let coque = Rect::from_center_size(centre, vec2(largeur, hauteur));

        // Largeur de l'oeuf a une hauteur donnee, pour poser l'antenne contre
        // le flanc plutot que contre la boite englobante, ou elle flotterait.
        let demi_largeur = |y: f32| -> f32 {
            let t = ((centre.y - y) / (hauteur * 0.5)).clamp(-1.0, 1.0).acos();
            largeur * 0.5 * t.sin() * (1.0 - 0.16 * t.cos())
        };

        // Antenne laterale, dessinee avant le corps pour qu'il la recouvre a sa
        // base. C'est elle qui porte la molette de zoom, propre a la Paradise.
        let antenne_haut = centre.y - hauteur * 0.26;
        let antenne_bas = centre.y + hauteur * 0.02;
        let flanc = demi_largeur((antenne_haut + antenne_bas) * 0.5);
        let antenne = Rect::from_min_max(
            pos2(centre.x + flanc - largeur * 0.06, antenne_haut),
            pos2(centre.x + flanc + largeur * 0.13, antenne_bas),
        );
        // La molette n'est pas de la couleur du corps sur la console : elle
        // tranche, vert d'eau sur la rose, violet sur la bleue. Elle est
        // cannelee, ce que rendent quelques traits horizontaux.
        let rayon_molette = largeur * 0.055;
        ui.painter().rect_filled(antenne.translate(vec2(0.0, 2.5)), rayon_molette, ombre_col);
        ui.painter().rect_filled(antenne, rayon_molette, accent_col);
        ui.painter().rect_stroke(antenne, rayon_molette, Stroke::new(1.5, ombre_col));
        let cannelures = 7;
        for i in 1..cannelures {
            let y = antenne.min.y
                + antenne.height() * i as f32 / cannelures as f32;
            ui.painter().line_segment(
                [
                    pos2(antenne.min.x + antenne.width() * 0.28, y),
                    pos2(antenne.max.x - antenne.width() * 0.12, y),
                ],
                Stroke::new(1.0, ombre_col.gamma_multiply(0.5)),
            );
        }

        // Corps de l'oeuf, puis sa calotte : la moitie haute de la vraie coque
        // porte un motif de coquille fendue et n'est pas de la meme couleur.
        let contour = contour_oeuf(centre, largeur, hauteur, 96);
        ui.painter().add(Shape::convex_polygon(
            contour.clone(),
            corps_col,
            Stroke::new(3.0, ombre_col),
        ));

        // La calotte se construit en balayant l'angle de part et d'autre du
        // sommet, pas en filtrant le contour : un filtre garderait les points
        // dans l'ordre du tour complet, et la forme obtenue se croiserait.
        //
        // La fente passe au tiers haut, au dessus de l'ecran, comme sur la
        // console : c'est la moitie ouvrante, celle qui cache le port de
        // connexion.
        const COS_FENTE: f32 = 0.70;
        let ligne_fente = centre.y - hauteur * 0.5 * COS_FENTE;
        let angle_fente = COS_FENTE.acos();
        let pas_calotte = 48;
        let mut calotte = Vec::with_capacity(pas_calotte + 1);
        for i in 0..=pas_calotte {
            let t = -angle_fente + 2.0 * angle_fente * i as f32 / pas_calotte as f32;
            let resserrement = 1.0 - 0.16 * t.cos();
            calotte.push(pos2(
                centre.x + largeur * 0.5 * t.sin() * resserrement,
                centre.y - hauteur * 0.5 * t.cos(),
            ));
        }
        ui.painter().add(Shape::convex_polygon(calotte, calotte_col, Stroke::NONE));

        // La fente elle meme : une ligne de dents alternees, la coquille
        // cassee. Les dents pointant vers le bas sont de la couleur de la
        // calotte, celles pointant vers le haut de celle du corps.
        let etendue = largeur * 0.5 * angle_fente.sin() * (1.0 - 0.16 * COS_FENTE);
        let _ = &demi_largeur;
        let dents = 11;
        let pas = etendue * 2.0 / dents as f32;
        let creux = hauteur * 0.022;
        for i in 0..dents {
            let x0 = centre.x - etendue + pas * i as f32;
            let x1 = x0 + pas;
            let bas = if i % 2 == 0 { ligne_fente + creux } else { ligne_fente - creux };
            let couleur = if i % 2 == 0 { calotte_col } else { corps_col };
            ui.painter().add(Shape::convex_polygon(
                vec![pos2(x0, ligne_fente), pos2(x1, ligne_fente), pos2((x0 + x1) * 0.5, bas)],
                couleur,
                Stroke::NONE,
            ));
        }

        // L'ecran occupe le haut de la coque, les commandes le bas.
        let marge = largeur * 0.17;
        let cote = (largeur - 2.0 * marge).min(hauteur * 0.38).max(64.0);
        let ecran_rect =
            Rect::from_center_size(pos2(centre.x, centre.y - hauteur * 0.04), vec2(cote, cote));

        // Le tour d'ecran de la console n'est pas un rectangle arrondi : c'est
        // une plaque imprimee a huit cotes, coins coupes, avec une pointe vers
        // le bas sous l'ecran. C'est aussi elle qui porte le cache transparent
        // ou se glissent les papiers de personnalisation.
        let cadre = ecran_rect.expand(cote * 0.22);
        let coupe = cadre.width() * 0.20;
        let pointe = cadre.height() * 0.16;
        let plaque = vec![
            pos2(cadre.min.x + coupe, cadre.min.y),
            pos2(cadre.max.x - coupe, cadre.min.y),
            pos2(cadre.max.x, cadre.min.y + coupe),
            pos2(cadre.max.x, cadre.max.y - coupe),
            pos2(cadre.max.x - coupe, cadre.max.y),
            pos2(centre.x, cadre.max.y + pointe),
            pos2(cadre.min.x + coupe, cadre.max.y),
            pos2(cadre.min.x, cadre.max.y - coupe),
            pos2(cadre.min.x, cadre.min.y + coupe),
        ];
        ui.painter().add(Shape::convex_polygon(
            plaque.clone(),
            motif_col,
            Stroke::new(2.0, ombre_col),
        ));
        // Liseré interieur, plus clair : la plaque imprimee a un bord.
        let interieur: Vec<Pos2> = plaque
            .iter()
            .map(|p| {
                let v = *p - cadre.center();
                cadre.center() + v * 0.88
            })
            .collect();
        ui.painter().add(Shape::convex_polygon(
            interieur,
            calotte_col.gamma_multiply(0.92),
            Stroke::NONE,
        ));

        // Le mot de marque, au dessus de l'ecran, dans la couleur d'accent.
        ui.painter().text(
            pos2(centre.x, cadre.min.y + cote * 0.11),
            egui::Align2::CENTER_CENTER,
            "TAMAGOTCHI",
            egui::FontId::proportional((cote * 0.11).clamp(7.0, 14.0)),
            accent_col,
        );

        // Fond blanc sous la dalle, comme la vitre de la vraie.
        ui.painter().rect_filled(
            ecran_rect.expand(cote * 0.045),
            cote * 0.03,
            Color32::from_rgb(246, 248, 250),
        );
        match ecran {
            Some(texture) => {
                ui.painter().image(
                    texture.id(),
                    ecran_rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                ui.painter().rect_filled(ecran_rect, 0.0, Color32::BLACK);
            }
        }
        ui.painter().rect_stroke(ecran_rect, 0.0, Stroke::new(1.5, Color32::from_rgb(60, 60, 66)));

        // Clic sur l'ecran : gauche vaut A, droit vaut B, molette vaut l'appui
        // de molette. C'est le geste attendu quand on joue a la souris, sans
        // viser les boutons, et le maintien passe aussi, sans quoi l'appui long
        // qui ouvre le laboratoire serait impossible.
        let sur_ecran = ui.allocate_rect(ecran_rect, Sense::click_and_drag());
        if sur_ecran.clicked() {
            commandes.bouton_a.clique = true;
        }
        if sur_ecran.secondary_clicked() {
            commandes.bouton_c.clique = true;
        }
        if sur_ecran.middle_clicked() {
            commandes.molette.clique = true;
        }
        if sur_ecran.is_pointer_button_down_on() {
            let (gauche, droit, milieu) = ui.input(|i| {
                (
                    i.pointer.primary_down(),
                    i.pointer.secondary_down(),
                    i.pointer.middle_down(),
                )
            });
            commandes.bouton_a.maintenu |= gauche;
            commandes.bouton_c.maintenu |= droit;
            commandes.molette.maintenu |= milieu;
        }

        let _ = display;

        // Trois boutons alignes sous l'ecran, comme sur la console.
        let rayon = (largeur * 0.062).clamp(14.0, 24.0);
        let ligne = (cadre.max.y + (coque.max.y - cadre.max.y) * 0.42)
            .min(coque.max.y - rayon - hauteur * 0.06);
        let ecart = (largeur * 0.27).min(90.0);

        let bouton = |ui: &mut Ui, centre: Pos2, etiquette: &str| -> EtatBouton {
            let zone = Rect::from_center_size(centre, vec2(rayon * 2.0, rayon * 2.0));
            let reponse = ui.allocate_rect(zone, Sense::click_and_drag());
            // `is_pointer_button_down_on` reste vrai tant que le pointeur est
            // enfonce sur le bouton : c'est lui qui porte l'appui long.
            let etat = EtatBouton {
                maintenu: reponse.is_pointer_button_down_on(),
                clique: reponse.clicked(),
            };
            let peintre = ui.painter();
            let decalage = if etat.maintenu { 1.5 } else { 0.0 };
            // Creux dans la coque, puis la pastille. Sur la console les trois
            // boutons tranchent sur le corps, ils ne sont pas de sa couleur.
            peintre.circle_filled(pos2(centre.x, centre.y + 2.0), rayon * 1.12, ombre_col);
            peintre.circle_filled(
                pos2(centre.x, centre.y + decalage),
                rayon,
                if etat.maintenu { bouton_col.gamma_multiply(0.75) } else { bouton_col },
            );
            // Reflet en haut a gauche, ce qui donne le relief du plastique.
            if !etat.maintenu {
                peintre.circle_filled(
                    pos2(centre.x - rayon * 0.28, centre.y - rayon * 0.30),
                    rayon * 0.30,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 90),
                );
            }
            peintre.circle_stroke(
                pos2(centre.x, centre.y + decalage),
                rayon,
                Stroke::new(1.5, ombre_col),
            );
            peintre.text(
                pos2(centre.x, centre.y + decalage),
                egui::Align2::CENTER_CENTER,
                etiquette,
                egui::FontId::monospace(12.0),
                ombre_col.gamma_multiply(0.8),
            );
            etat
        };

        commandes.bouton_a = {
            let mut e = bouton(ui, pos2(centre.x - ecart, ligne), "A");
            e.maintenu |= commandes.bouton_a.maintenu;
            e.clique |= commandes.bouton_a.clique;
            e
        };
        commandes.bouton_b = {
            let mut e = bouton(ui, pos2(centre.x, ligne + rayon * 0.5), "B");
            e.maintenu |= commandes.bouton_b.maintenu;
            e.clique |= commandes.bouton_b.clique;
            e
        };
        commandes.bouton_c = {
            // Le clic droit sur l'ecran vaut C : sans cette fusion, le bouton
            // dessine ecrasait ce que l'ecran avait deja signale.
            let mut e = bouton(ui, pos2(centre.x + ecart, ligne), "C");
            e.maintenu |= commandes.bouton_c.maintenu;
            e.clique |= commandes.bouton_c.clique;
            e
        };

        // Molette de zoom, sur l'antenne. Sa fenetre transparente porte deux
        // fleches opposees et sert aussi de bouton : un appui long ouvre le
        // laboratoire, un appui bref vaut B dans certains menus.
        let molette = Rect::from_center_size(
            pos2(antenne.center().x + antenne.width() * 0.12, antenne.center().y),
            vec2(antenne.width() * 0.72, antenne.height() * 0.62),
        );
        let reponse = ui.allocate_rect(molette, Sense::click_and_drag());
        let mut tourne = false;
        if reponse.dragged() {
            let dy = reponse.drag_delta().y;
            if dy.abs() > 2.0 {
                commandes.molette_tournee += if dy > 0.0 { 1 } else { -1 };
                tourne = true;
            }
        }
        // La fenetre transparente est aussi un bouton, comme sur la console :
        // maintenue elle ouvre le laboratoire, breve elle vaut B dans certains
        // menus. On ne compte pas l'appui pendant qu'on tourne, sinon chaque
        // cran vaudrait aussi une pression.
        if !tourne {
            commandes.molette.maintenu |= reponse.is_pointer_button_down_on();
        }
        commandes.molette.clique |= reponse.clicked();

        let roulette = ui.input(|i| i.raw_scroll_delta.y);
        let survol = ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| coque.contains(p) || antenne.contains(p));
        if roulette != 0.0 && survol {
            commandes.molette_tournee += if roulette > 0.0 { 1 } else { -1 };
        }

        let peintre = ui.painter();
        peintre.rect_filled(molette, molette.width() * 0.35, ombre_col);
        let fenetre = molette.shrink(molette.width() * 0.18);
        peintre.rect_filled(fenetre, fenetre.width() * 0.3, Color32::from_rgb(238, 246, 252));
        peintre.rect_stroke(fenetre, fenetre.width() * 0.3, Stroke::new(1.5, ombre_col));

        // Les deux fleches opposees defilent avec l'angle : c'est ce qui donne
        // la sensation de rotation, la molette n'ayant pas d'aiguille.
        let hauteur_utile = fenetre.height();
        let encre = Color32::from_rgb(70, 80, 100);
        for i in 0..2 {
            let phase = (angle_molette * 0.02 + i as f32 * 0.5).rem_euclid(1.0);
            let y = fenetre.min.y + phase * hauteur_utile;
            let vers_le_haut = i == 0;
            let d = fenetre.width() * 0.22;
            let pointe = if vers_le_haut { y - d } else { y + d };
            if pointe > fenetre.min.y && pointe < fenetre.max.y {
                peintre.add(Shape::convex_polygon(
                    vec![
                        pos2(fenetre.center().x - d, y),
                        pos2(fenetre.center().x + d, y),
                        pos2(fenetre.center().x, pointe),
                    ],
                    encre,
                    Stroke::NONE,
                ));
            }
        }

        commandes
    }
}
