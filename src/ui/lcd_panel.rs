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

/// Decoupe de la fenetre transparente qui entoure l'ecran, en morceaux
/// convexes a superposer.
///
/// La forme de la console n'est ni un rectangle ni un octogone : le haut est
/// droit et plus etroit que le reste, les flancs avancent a mi hauteur, et le
/// bas se resserre sur une pointe. Elle est donc concave, ce qu'egui ne sait
/// pas remplir d'un coup. Trois morceaux convexes de la meme couleur suffisent,
/// et comme rien n'est trace par dessus, leurs jointures ne se voient pas.
///
/// Les coordonnees sont donnees en fractions du cadre, l'origine au centre,
/// pour que la forme suive la taille de la fenetre sans se deformer.
fn silhouette_fenetre(cadre: Rect) -> Vec<Vec<Pos2>> {
    let c = cadre.center();
    let dx = cadre.width() * 0.5;
    let dy = cadre.height() * 0.5;
    let p = |x: f32, y: f32| pos2(c.x + dx * x, c.y + dy * y);

    // Le corps : haut droit et resserre, flancs droits, base resserree.
    let corps = vec![
        p(-0.62, -1.00),
        p(0.62, -1.00),
        p(0.82, -0.74),
        p(0.82, 0.62),
        p(0.52, 0.94),
        p(-0.52, 0.94),
        p(-0.82, 0.62),
        p(-0.82, -0.74),
    ];

    // Les joues : ce qui avance a mi hauteur, de part et d'autre.
    let joues = vec![
        p(-0.70, -0.46),
        p(0.70, -0.46),
        p(1.00, -0.16),
        p(1.00, 0.20),
        p(0.70, 0.50),
        p(-0.70, 0.50),
        p(-1.00, 0.20),
        p(-1.00, -0.16),
    ];

    // La pointe, sous l'ecran.
    let pointe = vec![p(-0.34, 0.80), p(0.34, 0.80), p(0.0, 1.16)];

    vec![corps, joues, pointe]
}

/// Habillage de la coque, tel que le panneau le recoit.
///
/// Les textures sont deja composees, papier et masque cuits ensemble, dans le
/// repere carre de la coque : le panneau n'a plus qu'a les poser.
pub struct Habits<'a> {
    pub reglages: &'a crate::gui::fond::Habillage,
    /// Papier de la fenetre, ou de la coque entiere.
    pub papier: Option<&'a TextureHandle>,
    /// Papier propre a la calotte, quand elle a le sien.
    pub chapeau: Option<&'a TextureHandle>,
}

/// Peint un polygone avec une texture composee dans le repere de la coque.
///
/// egui ne sait pas rogner sur un polygone : on fabrique le maillage a la main,
/// en eventail depuis le premier sommet, et les coordonnees de texture viennent
/// de la position dans le carre de reference. Ce carre est le meme que celui de
/// la composition, ce qui suffit a faire correspondre les deux sans autre
/// calcul.
fn peindre_texture(morceau: &[Pos2], repere: Rect, texture: egui::TextureId) -> Shape {
    let mut maillage = egui::Mesh::with_texture(texture);
    for point in morceau {
        maillage.colored_vertex(*point, Color32::WHITE);
        let dernier = maillage.vertices.len() - 1;
        maillage.vertices[dernier].uv = pos2(
            (point.x - repere.min.x) / repere.width(),
            (point.y - repere.min.y) / repere.height(),
        );
    }
    for i in 1..morceau.len().saturating_sub(1) {
        maillage.add_triangle(0, i as u32, i as u32 + 1);
    }
    Shape::mesh(maillage)
}

/// Dessine la dalle avec des coins arrondis.
///
/// `Painter::image` ne pose qu'un rectangle franc. On fabrique donc le maillage
/// a la main, en eventail depuis le centre, les coordonnees de texture venant
/// de la position dans le rectangle.
fn dalle_arrondie(rect: Rect, rayon: f32, texture: egui::TextureId) -> Shape {
    let rayon = rayon.min(rect.width() * 0.5).min(rect.height() * 0.5);
    let mut contour = Vec::with_capacity(4 * 7);
    let coins = [
        (pos2(rect.max.x - rayon, rect.max.y - rayon), 0.0_f32),
        (pos2(rect.min.x + rayon, rect.max.y - rayon), 90.0),
        (pos2(rect.min.x + rayon, rect.min.y + rayon), 180.0),
        (pos2(rect.max.x - rayon, rect.min.y + rayon), 270.0),
    ];
    for (centre, depart) in coins {
        for pas in 0..=6 {
            let angle = (depart + 90.0 * pas as f32 / 6.0).to_radians();
            contour.push(pos2(
                centre.x + rayon * angle.cos(),
                centre.y + rayon * angle.sin(),
            ));
        }
    }

    let mut maillage = egui::Mesh::with_texture(texture);
    let uv = |p: Pos2| {
        pos2(
            (p.x - rect.min.x) / rect.width(),
            (p.y - rect.min.y) / rect.height(),
        )
    };
    let centre = rect.center();
    maillage.colored_vertex(centre, Color32::WHITE);
    let dernier = maillage.vertices.len() - 1;
    maillage.vertices[dernier].uv = uv(centre);
    for point in &contour {
        maillage.colored_vertex(*point, Color32::WHITE);
        let dernier = maillage.vertices.len() - 1;
        maillage.vertices[dernier].uv = uv(*point);
    }
    let nombre = contour.len() as u32;
    for i in 0..nombre {
        maillage.add_triangle(0, 1 + i, 1 + (i + 1) % nombre);
    }
    Shape::mesh(maillage)
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
        habits: &Habits<'_>,
    ) -> Commandes {
        let habillage = habits.reglages;
        let couleurs = shell_color.couleurs();
        let (corps_col, calotte_col, ombre_col, bouton_col) =
            (couleurs.corps, couleurs.calotte, couleurs.ombre, couleurs.bouton);
        let (accent_col, motif_col) = (couleurs.accent, couleurs.motif);
        // Les commandes peuvent avoir leur propre couleur, sinon elles suivent
        // la coque.
        let rvb = |c: Option<[u8; 3]>, defaut: Color32| {
            c.map(|[r, v, b]| Color32::from_rgb(r, v, b)).unwrap_or(defaut)
        };
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
        ui.painter().rect_filled(
            antenne,
            rayon_molette,
            rvb(habits.reglages.molette_couleur, accent_col),
        );
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

        // Repere carre de la coque : c'est celui dans lequel les papiers ont ete
        // composes, et il ne depend pas de la largeur de la fenetre.
        let repere = Rect::from_center_size(centre, vec2(hauteur, hauteur));

        // Corps de l'oeuf, puis sa calotte : la moitie haute de la vraie coque
        // porte un motif de coquille fendue et n'est pas de la meme couleur.
        let contour = contour_oeuf(centre, largeur, hauteur, 96);
        ui.painter().add(Shape::convex_polygon(
            contour.clone(),
            corps_col,
            Stroke::new(3.0, ombre_col),
        ));

        // Le papier peut couvrir la coque entiere. Il est pose ici, sur le
        // corps : ce qui vient apres, calotte et fenetre, le recouvre ou non
        // selon les reglages, ce qui suffit a tout ordonner.
        if habillage.couvre_tout {
            if let Some(texture) = habits.papier {
                ui.painter()
                    .add(peindre_texture(&contour, repere, texture.id()));
            }
        }

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
        // Couverte par le papier general, la calotte n'est pas repeinte : c'est
        // ce qui la laisse apparaitre. Sinon elle prend sa couleur, la sienne
        // ou celle de la coque, puis son propre papier s'il y en a un.
        let chapeau_couvert = habillage.couvre_tout && habillage.inclut_le_chapeau;
        if !chapeau_couvert {
            let couleur = habillage
                .chapeau_couleur
                .map(|[r, v, b]| Color32::from_rgb(r, v, b))
                .unwrap_or(calotte_col);
            ui.painter().add(Shape::convex_polygon(calotte.clone(), couleur, Stroke::NONE));
            if let Some(texture) = habits.chapeau {
                ui.painter()
                    .add(peindre_texture(&calotte, repere, texture.id()));
            }
        }

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
        let cote = ((largeur - 2.0 * marge).min(hauteur * 0.38).max(64.0)
            * habillage.ecran_taille.clamp(0.4, 1.8))
            .min(largeur * 0.98);
        let ecran_rect = Rect::from_center_size(
            pos2(
                centre.x,
                centre.y - hauteur * 0.04 + hauteur * habillage.ecran_dy.clamp(-0.4, 0.4),
            ),
            vec2(cote, cote),
        );

        // La fenetre transparente qui entoure l'ecran. Ce n'est pas un cadre :
        // il n'y a aucun trait sur la console, c'est la decoupe du plastique
        // clair qui fait la forme, et le papier de personnalisation se voit au
        // travers. Elle ne touche pas l'ecran, elle l'entoure a distance.
        //
        // La silhouette n'est pas convexe : ses flancs avancent puis se
        // reculent. On la compose donc de morceaux convexes de la meme
        // couleur, poses l'un sur l'autre. Sans trait, les jointures ne se
        // voient pas, et egui ne sait remplir que du convexe.
        let cadre = ecran_rect.expand(cote * 0.30);
        if !habillage.couvre_tout {
            for morceau in silhouette_fenetre(cadre) {
                match habits.papier {
                    Some(texture) => {
                        ui.painter()
                            .add(peindre_texture(&morceau, repere, texture.id()));
                    }
                    None => {
                        ui.painter().add(Shape::convex_polygon(
                            morceau,
                            motif_col,
                            Stroke::NONE,
                        ));
                    }
                }
            }
        }

        // Le mot imprime au dessus de l'ecran. Sans couleur choisie il prend
        // celle d'accent de la coque, comme sur la console.
        if habillage.titre_visible && !habillage.titre.trim().is_empty() {
            let couleur = habillage
                .titre_couleur
                .map(|[r, v, b]| Color32::from_rgb(r, v, b))
                .unwrap_or(accent_col);
            let taille = (cote * 0.10 * habillage.titre_taille.clamp(0.3, 3.0))
                .clamp(5.0, 40.0);
            ui.painter().text(
                pos2(centre.x, cadre.min.y + taille * 1.05),
                egui::Align2::CENTER_CENTER,
                habillage.titre.trim(),
                egui::FontId::proportional(taille),
                couleur,
            );
        }

        // La vitre, un liseré clair autour de la dalle, le seul bord franc de
        // cette zone sur la vraie console. Sans elle, c'est la dalle qui prend
        // l'arrondi : autrement le rectangle nu tomberait dans la decoupe.
        let arrondi = if habillage.vitre_visible {
            let epaisseur = cote * habillage.vitre_epaisseur.clamp(0.0, 0.10);
            let [r, v, b] = habillage.vitre_couleur;
            ui.painter().rect_filled(
                ecran_rect.expand(epaisseur),
                epaisseur.max(cote * 0.012),
                Color32::from_rgb(r, v, b),
            );
            0.0
        } else {
            cote * 0.055
        };
        match ecran {
            Some(texture) if arrondi > 0.0 => {
                ui.painter()
                    .add(dalle_arrondie(ecran_rect, arrondi, texture.id()));
            }
            Some(texture) => {
                ui.painter().image(
                    texture.id(),
                    ecran_rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                ui.painter().rect_filled(ecran_rect, arrondi, Color32::BLACK);
            }
        }
        ui.painter().rect_stroke(
            ecran_rect,
            arrondi,
            Stroke::new(1.5, Color32::from_rgb(60, 60, 66)),
        );

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

        let teinte_bouton = rvb(habits.reglages.bouton_couleur, bouton_col);
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
                if etat.maintenu { teinte_bouton.gamma_multiply(0.75) } else { teinte_bouton },
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
