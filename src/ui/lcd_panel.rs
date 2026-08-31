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

/// Etat reel des quatre broches de commande, pour l'animation.
///
/// Le dessin ne peut pas le deviner : un appui venu du clavier ou du navigateur
/// ne laisse aucune trace dans la reponse du pointeur.
#[derive(Debug, Clone, Copy, Default)]
pub struct Enfonces {
    pub a: bool,
    pub b: bool,
    pub c: bool,
    pub molette: bool,
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

    // La crete : le haut etait une droite, plaquee sous la calotte. La console
    // y porte le meme motif de coquille fendue que la fente. Chaque dent est un
    // triangle a part, egui ne remplissant que du convexe, et le papier les
    // traverse sans jointure puisque toutes prennent leur texture du meme
    // repere.
    const DENTS: usize = 7;
    const HAUTEUR_DENT: f32 = 0.085;
    let mut crete = Vec::with_capacity(DENTS);
    let gauche = -0.62;
    let pas = (0.62 - gauche) / DENTS as f32;
    for i in 0..DENTS {
        let x0 = gauche + pas * i as f32;
        crete.push(vec![
            p(x0, -1.00),
            p(x0 + pas, -1.00),
            p(x0 + pas * 0.5, -1.00 - HAUTEUR_DENT),
        ]);
    }

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

    let mut morceaux = vec![corps, joues, pointe];
    morceaux.extend(crete);
    morceaux
}

/// Habillage de la coque, tel que le panneau le recoit.
///
/// Les textures sont deja composees, papier et masque cuits ensemble, dans le
/// repere carre de la coque : le panneau n'a plus qu'a les poser.
pub struct Habits<'a> {
    pub reglages: &'a crate::gui::fond::Habillage,
    /// Fond de la coque entiere, derriere tout le reste.
    pub coque: Option<&'a TextureHandle>,
    /// Papier de la fenetre transparente, autour de l'ecran.
    pub papier: Option<&'a TextureHandle>,
    /// Papier propre a la calotte, quand elle a le sien.
    pub chapeau: Option<&'a TextureHandle>,
    /// Vrai quand un masque importe est en service. Il remplace alors la
    /// decoupe de la console au lieu de s'y ajouter : sans cela, le papier
    /// restait enferme dans la silhouette de la fenetre transparente et le
    /// masque ne pouvait que la reduire.
    pub masque_impose: bool,
}

/// Rogne un polygone convexe sur un autre, convexe lui aussi.
///
/// Methode de Sutherland et Hodgman : on coupe le sujet par chaque cote de la
/// fenetre, l'un apres l'autre. Les deux etant convexes, le resultat l'est
/// aussi, et egui sait donc le remplir.
///
/// Le sens du contour n'est pas suppose : on le deduit en regardant de quel
/// cote de la premiere arete tombe le centre de la fenetre. Le deduire du signe
/// de l'aire demande de savoir dans quel sens l'ordonnee croit, et se tromper
/// rogne tout.
fn rogner_sur(sujet: &[Pos2], fenetre: &[Pos2]) -> Vec<Pos2> {
    if sujet.len() < 3 || fenetre.len() < 3 {
        return sujet.to_vec();
    }
    let cote = |p: Pos2, a: Pos2, b: Pos2| -> f32 {
        (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
    };
    let centre = {
        let n = fenetre.len() as f32;
        let (sx, sy) = fenetre.iter().fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
        pos2(sx / n, sy / n)
    };
    let sens = if cote(centre, fenetre[0], fenetre[1]) >= 0.0 { 1.0 } else { -1.0 };
    let dedans = |p: Pos2, a: Pos2, b: Pos2| sens * cote(p, a, b) >= 0.0;
    let couper = |p: Pos2, q: Pos2, a: Pos2, b: Pos2| -> Pos2 {
        let dp = sens * cote(p, a, b);
        let dq = sens * cote(q, a, b);
        let t = if (dp - dq).abs() < f32::EPSILON { 0.0 } else { dp / (dp - dq) };
        pos2(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t)
    };

    let mut sortie: Vec<Pos2> = sujet.to_vec();
    for i in 0..fenetre.len() {
        if sortie.is_empty() {
            break;
        }
        let a = fenetre[i];
        let b = fenetre[(i + 1) % fenetre.len()];
        let entree = std::mem::take(&mut sortie);
        for j in 0..entree.len() {
            let p = entree[j];
            let q = entree[(j + 1) % entree.len()];
            let (dp, dq) = (dedans(p, a, b), dedans(q, a, b));
            if dp {
                sortie.push(p);
            }
            if dp != dq {
                sortie.push(couper(p, q, a, b));
            }
        }
    }
    sortie
}

/// Peint un polygone avec une texture composee dans le repere de la coque.
///
/// egui ne sait pas rogner sur un polygone : on fabrique le maillage a la main,
/// en eventail depuis le premier sommet, et les coordonnees de texture viennent
/// de la position dans le carre de reference. Ce carre est le meme que celui de
/// la composition, ce qui suffit a faire correspondre les deux sans autre
/// calcul.
fn peindre_texture(morceau: &[Pos2], repere: Rect, texture: egui::TextureId) -> Shape {
    peindre_texture_tournee(morceau, repere, texture, 0.0, repere.center())
}

/// Fait tourner un point autour d'un pivot.
fn tourner(point: Pos2, pivot: Pos2, angle: f32) -> Pos2 {
    if angle == 0.0 {
        return point;
    }
    let (s, c) = angle.sin_cos();
    let dx = point.x - pivot.x;
    let dy = point.y - pivot.y;
    pos2(pivot.x + dx * c - dy * s, pivot.y + dx * s + dy * c)
}

/// Comme `peindre_texture`, mais le morceau tourne autour d'un pivot.
///
/// Les coordonnees de texture sont prises sur le point avant rotation : la
/// texture tourne donc avec la forme au lieu de glisser dessous.
fn peindre_texture_tournee(
    morceau: &[Pos2],
    repere: Rect,
    texture: egui::TextureId,
    angle: f32,
    pivot: Pos2,
) -> Shape {
    let mut maillage = egui::Mesh::with_texture(texture);
    for point in morceau {
        maillage.colored_vertex(tourner(*point, pivot, angle), Color32::WHITE);
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

/// Un ruban entre deux lignes de meme longueur, rendu en triangles.
///
/// Sert la ou deux aplats doivent se toucher sans se voir. Deux formes voisines
/// laissent toujours un lisere : egui adoucit le bord de chacune, et les deux
/// adoucissements se superposent sans se completer. Un papier pose forme par
/// forme s'arrete au meme endroit et ne franchit pas la jointure. Un maillage
/// n'a ni l'un ni l'autre.
///
/// Sans `papier`, le ruban est d'un seul aplat. Avec, il porte la texture,
/// placee comme partout ailleurs d'apres le repere carre de la coque, ce qui la
/// rend continue d'un morceau au suivant.
fn ruban(
    haut: &[Pos2],
    bas: &[Pos2],
    couleur: Color32,
    papier: Option<(egui::TextureId, Rect)>,
) -> Shape {
    let mut maillage =
        egui::Mesh::with_texture(papier.map(|(t, _)| t).unwrap_or_default());
    for (h, b) in haut.iter().zip(bas.iter()) {
        for point in [h, b] {
            maillage.colored_vertex(*point, couleur);
            if let Some((_, repere)) = papier {
                let dernier = maillage.vertices.len() - 1;
                maillage.vertices[dernier].uv = pos2(
                    (point.x - repere.min.x) / repere.width(),
                    (point.y - repere.min.y) / repere.height(),
                );
            }
        }
    }
    for i in 0..haut.len().min(bas.len()).saturating_sub(1) {
        let a = (i * 2) as u32;
        maillage.add_triangle(a, a + 1, a + 2);
        maillage.add_triangle(a + 1, a + 2, a + 3);
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
        enfonces: Enfonces,
        souris: crate::touches::Souris,
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
        // L'opacite est rangee a part de la couleur : une couleur absente suit
        // l'edition, et on veut pouvoir la rendre translucide sans avoir a la
        // choisir.
        let opaque = |couleur: Color32, opacite: f32| -> Color32 {
            let a = (opacite.clamp(0.0, 1.0) * 255.0).round() as u8;
            Color32::from_rgba_unmultiplied(couleur.r(), couleur.g(), couleur.b(), a)
        };
        let corps_col = opaque(
            rvb(habits.reglages.corps_couleur, corps_col),
            habillage.corps_opacite,
        );
        let motif_col = opaque(
            rvb(habits.reglages.motif_couleur, motif_col),
            habillage.motif_opacite,
        );
        // Tous les traits de la coque en decoulent : contour de l'oeuf, ombres
        // des reliefs, cerclage des boutons, stries de la molette.
        let ombre_col = opaque(
            rvb(habits.reglages.bordure_couleur, ombre_col),
            habillage.bordure_opacite,
        );
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
        // Le rectangle de l'antenne ne sert plus qu'a placer la molette et a
        // savoir si le pointeur la survole. Le bloc qui l'habillait doublait la
        // roue dessinee par dessus, sans rien apporter, et son animation
        // debordait sur une piece qui ne bouge pas.

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

        // Relief du plastique : un degrade du haut vers le bas et un reflet en
        // haut a gauche. La coque etait un aplat parfaitement plat, ce qui se
        // voit d'autant plus que la vraie est bombee. Le degrade est un
        // maillage en eventail depuis le centre, chaque sommet prenant sa
        // teinte de sa hauteur ; egui ne remplit un polygone que d'une seule
        // couleur, un maillage est le seul moyen d'en avoir deux.
        // Un relief negatif retourne l'eclairage : la lumiere vient d'en bas et
        // l'ombre se pose en haut. C'est le meme reglage, du signe pres.
        let relief = habillage.relief_coque.clamp(-1.0, 1.0);
        if relief != 0.0 {
            let vigueur = relief.abs();
            let par_en_bas = relief < 0.0;
            let mut maillage = egui::Mesh::default();
            let teinte_de = |y: f32| -> Color32 {
                let brut = ((y - coque.min.y) / hauteur).clamp(0.0, 1.0);
                let t = if par_en_bas { 1.0 - brut } else { brut };
                if t < 0.5 {
                    // Du cote de la lumiere : forte au bord, nulle au milieu.
                    let force = (1.0 - t * 2.0).powf(1.6);
                    Color32::from_rgba_unmultiplied(255, 255, 255, (74.0 * force * vigueur) as u8)
                } else {
                    // Du cote de l'ombre, qui s'installe en s'eloignant.
                    let force = ((t - 0.5) * 2.0).powf(1.4);
                    Color32::from_rgba_unmultiplied(0, 0, 0, (58.0 * force * vigueur) as u8)
                }
            };
            maillage.colored_vertex(centre, teinte_de(centre.y));
            for point in &contour {
                maillage.colored_vertex(*point, teinte_de(point.y));
            }
            let n = contour.len() as u32;
            for i in 0..n {
                maillage.add_triangle(0, i + 1, if i + 2 > n { 1 } else { i + 2 });
            }
            ui.painter().add(Shape::mesh(maillage));

            // Le reflet, une tache claire en haut a gauche comme sur un objet
            // brillant. Trois cercles concentriques de plus en plus pales, ce
            // qui suffit a donner un bord doux.
            let cote_reflet = if par_en_bas { 1.0 } else { -1.0 };
            let foyer = pos2(
                centre.x + largeur * 0.22 * cote_reflet,
                centre.y + hauteur * 0.28 * cote_reflet,
            );
            for (facteur, alpha) in [(0.30f32, 22.0f32), (0.20, 26.0), (0.11, 30.0)] {
                ui.painter().circle_filled(
                    foyer,
                    largeur * facteur,
                    Color32::from_rgba_unmultiplied(255, 255, 255, (alpha * vigueur) as u8),
                );
            }
        }

        // Le fond de coque, pose sur le corps. Tout ce qui suit passe devant.
        if let Some(texture) = habits.coque {
            ui.painter()
                .add(peindre_texture(&contour, repere, texture.id()));
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
        // Creux des dents. Il sert des ici : la calotte ne descend pas jusqu'a
        // la droite de la fente mais s'arrete au dessus, et c'est la dentelure
        // qui finit le travail. Sinon la calotte couvrirait les dents qui
        // pointent vers le haut, et le papier pose dessous, celui de la fenetre
        // par exemple, ne s'y verrait pas.
        let creux = hauteur * 0.022;
        // La calotte descend d'un cheveu sous le haut du ruban qui la suit.
        // Bord a bord, les deux adoucissements d'egui ne se completaient pas et
        // laissaient un lisere clair tout du long. Le recouvrement vaut un
        // dixieme du creux : assez pour fermer la couture, trop peu pour
        // combler les dents qui pointent vers le haut.
        let cos_haut = (COS_FENTE + creux * 0.90 / (hauteur * 0.5)).min(0.999);
        let angle_haut = cos_haut.acos();
        let pas_calotte = 48;
        let mut calotte = Vec::with_capacity(pas_calotte + 1);
        for i in 0..=pas_calotte {
            let t = -angle_haut + 2.0 * angle_haut * i as f32 / pas_calotte as f32;
            let resserrement = 1.0 - 0.16 * t.cos();
            calotte.push(pos2(
                centre.x + largeur * 0.5 * t.sin() * resserrement,
                centre.y - hauteur * 0.5 * t.cos(),
            ));
        }
        // L'ecran occupe le haut de la coque, les commandes le bas.
        let marge = largeur * 0.17;
        let base = (largeur - 2.0 * marge).min(hauteur * 0.38).max(64.0);
        let cote = (base * habillage.ecran_taille.clamp(0.3, 2.0)).min(largeur * 0.98);
        let milieu = centre.y - hauteur * 0.04;
        let ecran_rect = Rect::from_center_size(
            pos2(centre.x, milieu + hauteur * habillage.ecran_dy.clamp(-0.4, 0.4)),
            vec2(cote, cote),
        );
        // La fenetre transparente ne suit pas la dalle : elle a sa taille et sa
        // position propres, sans quoi agrandir l'ecran agrandirait son cadre.
        let cote_fenetre = (base * habillage.fenetre_taille.clamp(0.3, 2.2)).min(largeur * 1.1);
        let fenetre_rect = Rect::from_center_size(
            pos2(centre.x, milieu + hauteur * habillage.fenetre_dy.clamp(-0.4, 0.4)),
            vec2(cote_fenetre, cote_fenetre),
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
        let cadre = fenetre_rect.expand(cote_fenetre * 0.30);
        // Un masque importe donne lui meme la forme : le papier est alors pose
        // sur tout le repere carre, celui dans lequel il a ete cuit, et c'est
        // le masque qui decide de ce qui se voit.
        let morceaux = if habits.masque_impose && habits.papier.is_some() {
            vec![vec![
                repere.left_top(),
                repere.right_top(),
                repere.right_bottom(),
                repere.left_bottom(),
            ]]
        } else {
            silhouette_fenetre(cadre)
        };
        let angle_calque = habillage.fenetre_rotation.to_radians();
        let pivot = cadre.center();
        // Ombre portee du calque, en couches concentriques comme celle de la
        // dalle. Elle est posee sous tous les morceaux, sinon un morceau
        // couvrirait l'ombre du suivant.
        let ombre_calque = habillage.ombre_fenetre.clamp(-0.25, 0.25);
        if ombre_calque != 0.0 {
            let sens = ombre_calque.signum();
            let etendue = cote_fenetre * ombre_calque.abs();
            const COUCHES: usize = 5;
            for i in (1..=COUCHES).rev() {
                let t = i as f32 / COUCHES as f32;
                let alpha = (30.0 * (1.0 - t) + 8.0) as u8;
                let grossi = 1.0 + 0.06 * t;
                for morceau in &morceaux {
                    let etale: Vec<Pos2> = morceau
                        .iter()
                        .map(|p| {
                            let g = pos2(
                                pivot.x + (p.x - pivot.x) * grossi,
                                pivot.y + (p.y - pivot.y) * grossi + etendue * 0.35 * sens,
                            );
                            tourner(g, pivot, angle_calque)
                        })
                        .collect();
                    if etale.len() >= 3 {
                        ui.painter().add(Shape::convex_polygon(
                            etale,
                            Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
                            Stroke::NONE,
                        ));
                    }
                }
            }
        }
        for morceau in morceaux {
            let morceau = if habillage.fenetre_deborde {
                morceau
            } else {
                rogner_sur(&morceau, &contour)
            };
            if morceau.len() < 3 {
                continue;
            }
            match habits.papier {
                Some(texture) => {
                    ui.painter().add(peindre_texture_tournee(
                        &morceau,
                        repere,
                        texture.id(),
                        angle_calque,
                        pivot,
                    ));
                }
                None => {
                    let tourne: Vec<Pos2> =
                        morceau.iter().map(|p| tourner(*p, pivot, angle_calque)).collect();
                    ui.painter().add(Shape::convex_polygon(tourne, motif_col, Stroke::NONE));
                }
            }
        }

        // Ombre portee de la dalle. Quelques couches de plus en plus larges et
        // de plus en plus pales : egui ne sait pas flouter, mais l'empilement
        // donne un bord degrade convaincant.
        // Une ombre negative se pose au dessus au lieu de dessous : la lumiere
        // vient alors d'en bas, comme pour le relief.
        let porte_ombre = habillage.ombre_ecran.clamp(-0.25, 0.25);
        if porte_ombre != 0.0 {
            let sens = porte_ombre.signum();
            let etendue = cote * porte_ombre.abs();
            const COUCHES: usize = 6;
            for i in (1..=COUCHES).rev() {
                let t = i as f32 / COUCHES as f32;
                let marge = etendue * t;
                let alpha = (34.0 * (1.0 - t) + 10.0) as u8;
                ui.painter().rect_filled(
                    ecran_rect.expand(marge).translate(vec2(0.0, etendue * 0.35 * sens)),
                    cote * 0.03 + marge,
                    Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
                );
            }
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
        let (gauche, droit, milieu) = if sur_ecran.is_pointer_button_down_on() {
            ui.input(|i| {
                (
                    i.pointer.primary_down(),
                    i.pointer.secondary_down(),
                    i.pointer.middle_down(),
                )
            })
        } else {
            (false, false, false)
        };
        for (bouton, clique, tenu) in [
            (souris.primaire, sur_ecran.clicked(), gauche),
            (souris.secondaire, sur_ecran.secondary_clicked(), droit),
            (souris.milieu, sur_ecran.middle_clicked(), milieu),
        ] {
            let cible = match bouton {
                crate::touches::Bouton::Aucun => continue,
                crate::touches::Bouton::A => &mut commandes.bouton_a,
                crate::touches::Bouton::B => &mut commandes.bouton_b,
                crate::touches::Bouton::C => &mut commandes.bouton_c,
                crate::touches::Bouton::Molette => &mut commandes.molette,
            };
            cible.clique |= clique;
            cible.maintenu |= tenu;
        }

        let _ = display;

        // La calotte passe en dernier, au dessus de la fenetre et de l'ecran :
        // sur la vraie console c'est une demi coque qui recouvre l'avant, et
        // une fenetre agrandie doit passer dessous et non par dessus.
        {
            let couleur = habillage
                .chapeau_couleur
                .map(|[r, v, b]| Color32::from_rgb(r, v, b))
                .unwrap_or(calotte_col);
            ui.painter().add(Shape::convex_polygon(calotte.clone(), couleur, Stroke::NONE));
            // Incluse dans le fond de coque, elle en porte la suite ; sinon
            // elle prend son propre papier s'il y en a un.
            let texture = if habillage.inclut_le_chapeau {
                habits.coque.or(habits.chapeau)
            } else {
                habits.chapeau
            };
            if let Some(texture) = texture {
                ui.painter()
                    .add(peindre_texture(&calotte, repere, texture.id()));
            }
        }

        // La fente elle meme : la coquille cassee, une ligne brisee entre la
        // calotte et le corps.
        //
        // Elle etait faite de triangles poses cote a cote sur la droite de la
        // fente, un par dent. Cela laissait un lisere clair tout du long : egui
        // adoucit le bord de chaque forme, et deux adoucissements qui se
        // touchent ne se completent pas. Le papier s'arretait au meme endroit,
        // chaque triangle portant le sien, si bien que les dents restaient
        // vides et que la droite se voyait au lieu de la dentelure.
        //
        // On peint donc d'un seul tenant, en un ruban tendu entre le bas de la
        // calotte et la ligne brisee. Un maillage n'a pas de bord adouci, et le
        // papier le traverse sans jointure puisqu'il n'y en a plus.
        //
        // Le dessous de la ligne n'est pas peint du tout. Le corps, son fond et
        // le papier de la fenetre sont deja poses a ce moment la : les laisser
        // paraitre est ce qui remplit les dents qui mordent vers le haut. Les
        // couvrir d'un aplat de la couleur du corps, comme on le faisait,
        // rendait ces dents unies quel que soit le papier choisi.
        let etendue = largeur * 0.5 * angle_fente.sin() * (1.0 - 0.16 * COS_FENTE);
        let dents = 11;
        let pas = etendue * 2.0 / dents as f32;
        let texture_calotte = if habillage.inclut_le_chapeau {
            habits.coque.or(habits.chapeau)
        } else {
            habits.chapeau
        };

        // La ligne brisee. Elle touche la droite de la fente a chaque bord de
        // dent, et s'en ecarte de `creux` au milieu, vers le bas une fois sur
        // deux. C'est la silhouette d'avant, au trait pres.
        let mut dentelure = Vec::with_capacity(dents * 2 + 1);
        for i in 0..dents {
            let x0 = centre.x - etendue + pas * i as f32;
            let vers_le_bas = i % 2 == 0;
            dentelure.push(pos2(x0, ligne_fente));
            dentelure.push(pos2(
                x0 + pas * 0.5,
                if vers_le_bas { ligne_fente + creux } else { ligne_fente - creux },
            ));
        }
        dentelure.push(pos2(centre.x + etendue, ligne_fente));

        // Le haut de la bande suit le flanc de l'oeuf : au dessus de la fente
        // la coque se resserre, et une droite depasserait de la silhouette.
        //
        // Les bornes se calculent, donc on ne les donne pas a `clamp` : il
        // affirme `min <= max` et tombe si l'une des deux est NaN. Le cas
        // arrive pour de bon, le temps d'une image, quand la fenetre change de
        // taille au changement de mode : la hauteur disponible vaut alors zero,
        // `demi_largeur` divise zero par zero et rend NaN. `max` puis `min`
        // rendent la valeur d'origine dans ce cas, sans rien affirmer.
        let bord_haut = demi_largeur(ligne_fente - creux);
        let haut: Vec<Pos2> = dentelure
            .iter()
            .map(|p| {
                pos2(
                    p.x.max(centre.x - bord_haut).min(centre.x + bord_haut),
                    ligne_fente - creux,
                )
            })
            .collect();
        let couleur_calotte = opaque(
            habillage
                .chapeau_couleur
                .map(|[r, v, b]| Color32::from_rgb(r, v, b))
                .unwrap_or(calotte_col),
            habillage.chapeau_opacite,
        );
        ui.painter().add(ruban(&haut, &dentelure, couleur_calotte, None));
        if let Some(texture) = texture_calotte {
            ui.painter()
                .add(ruban(&haut, &dentelure, Color32::WHITE, Some((texture.id(), repere))));
        }

        // Le mot imprime, pose apres la calotte et sa dentelure. Il etait
        // dessine avant elles et passait donc dessous des qu'on le remontait :
        // sur la console il est serigraphie sur le plastique, rien ne le
        // recouvre. Sans couleur choisie il prend celle d'accent de la coque.
        if habillage.titre_visible && !habillage.titre.trim().is_empty() {
            let couleur = opaque(
                habillage
                    .titre_couleur
                    .map(|[r, v, b]| Color32::from_rgb(r, v, b))
                    .unwrap_or(accent_col),
                habillage.titre_opacite,
            );
            let taille = (cote * 0.10 * habillage.titre_taille.clamp(0.3, 3.0))
                .clamp(5.0, 40.0);
            ui.painter().text(
                pos2(
                    centre.x,
                    cadre.min.y + taille * 1.05
                        + hauteur * habillage.titre_dy.clamp(-0.6, 0.6),
                ),
                egui::Align2::CENTER_CENTER,
                habillage.titre.trim(),
                egui::FontId::proportional(taille),
                couleur,
            );
        }

        // Trois boutons alignes sous l'ecran, comme sur la console.
        // Le rayon suit la largeur, sans plafond. Un plafond en pixels faisait
        // que les boutons paraissaient plus petits sur une grande coque que sur
        // une petite : la disposition changeait d'un mode a l'autre, et ce qu'on
        // reglait en personnalisation ne ressemblait pas a ce qu'on voyait en
        // jouant. Seul un plancher reste, pour les toutes petites fenetres.
        let rayon = (largeur * 0.062).max(10.0) * habillage.boutons_taille.clamp(0.3, 2.5);
        let ligne = (cadre.max.y + (coque.max.y - cadre.max.y) * 0.42)
            .min(coque.max.y - rayon - hauteur * 0.06)
            + hauteur * habillage.boutons_dy.clamp(-0.4, 0.4);
        let ecart = largeur * 0.27 * habillage.boutons_ecart.clamp(0.2, 2.5);

        let teinte_bouton = opaque(
            rvb(habits.reglages.bouton_couleur, bouton_col),
            habillage.bouton_opacite,
        );
        // `enfonce` vient de l'etat reel de la broche : le bouton s'anime que
        // l'appui vienne du clavier, du navigateur ou d'un clic sur l'ecran, et
        // pas seulement d'un pointeur pose sur la pastille.
        let bouton = |ui: &mut Ui, centre: Pos2, etiquette: &str, enfonce: bool| -> EtatBouton {
            let zone = Rect::from_center_size(centre, vec2(rayon * 2.0, rayon * 2.0));
            let reponse = ui.allocate_rect(zone, Sense::click_and_drag());
            // `is_pointer_button_down_on` reste vrai tant que le pointeur est
            // enfonce sur le bouton : c'est lui qui porte l'appui long.
            // L'etat rendu ne vient que du pointeur. Y verser la broche
            // rebouclerait : le dessin la tiendrait basse, l'appelant la
            // relirait, et le bouton resterait enfonce pour toujours.
            let etat = EtatBouton {
                maintenu: reponse.is_pointer_button_down_on(),
                clique: reponse.clicked(),
            };
            // Le relief, lui, suit la broche : peu importe d'ou vient l'appui.
            let vu_enfonce = etat.maintenu || enfonce;
            let peintre = ui.painter();
            let decalage = if vu_enfonce { 1.5 } else { 0.0 };
            // Creux dans la coque, puis la pastille. Sur la console les trois
            // boutons tranchent sur le corps, ils ne sont pas de sa couleur.
            peintre.circle_filled(pos2(centre.x, centre.y + 2.0), rayon * 1.12, ombre_col);
            peintre.circle_filled(
                pos2(centre.x, centre.y + decalage),
                rayon,
                if vu_enfonce { teinte_bouton.gamma_multiply(0.75) } else { teinte_bouton },
            );
            // Reflet en haut a gauche, ce qui donne le relief du plastique.
            if !vu_enfonce {
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
            let mut e = bouton(ui, pos2(centre.x - ecart, ligne), "A", enfonces.a);
            e.maintenu |= commandes.bouton_a.maintenu;
            e.clique |= commandes.bouton_a.clique;
            e
        };
        commandes.bouton_b = {
            let mut e = bouton(ui, pos2(centre.x, ligne + rayon * 0.5), "B", enfonces.b);
            e.maintenu |= commandes.bouton_b.maintenu;
            e.clique |= commandes.bouton_b.clique;
            e
        };
        commandes.bouton_c = {
            // Le clic droit sur l'ecran vaut C : sans cette fusion, le bouton
            // dessine ecrasait ce que l'ecran avait deja signale.
            let mut e = bouton(ui, pos2(centre.x + ecart, ligne), "C", enfonces.c);
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

        // Une roue vue de cote, dans son logement. Le logement garde la
        // couleur des traits pour que la roue s'en detache ; la roue porte des
        // stries qui defilent, resserrees en haut et en bas comme le sont les
        // cannelures d'un cylindre qu'on regarde de profil. Les deux fleches
        // d'avant ne disaient rien du sens.
        let peintre = ui.painter();
        let teinte = opaque(
            rvb(habits.reglages.molette_couleur, accent_col),
            habillage.molette_opacite,
        );

        // Logement.
        peintre.rect_filled(molette, molette.width() * 0.36, ombre_col);

        // Enfoncee, la roue s'enfonce dans son logement et s'assombrit, comme
        // les trois boutons. Sans cela rien ne distinguait un appui de molette
        // d'un simple survol.
        let enfoncee = commandes.molette.maintenu || enfonces.molette;
        let creux = if enfoncee { 1.5 } else { 0.0 };
        let teinte = if enfoncee { teinte.gamma_multiply(0.75) } else { teinte };

        // Roue.
        let roue = molette
            .shrink2(vec2(molette.width() * 0.16, molette.height() * 0.07))
            .translate(vec2(0.0, creux));
        let arrondi = roue.width() * 0.42;
        peintre.rect_filled(roue, arrondi, teinte);

        // Stries. L'angle balaie la demi rotation visible, et la position
        // verticale suit son cosinus : elles se resserrent aux extremites.
        const STRIES: usize = 9;
        let milieu = roue.center().y;
        let demi = roue.height() * 0.5;
        let phase = (angle_molette * 0.012).rem_euclid(1.0);
        let x0 = roue.min.x + roue.width() * 0.20;
        let x1 = roue.max.x - roue.width() * 0.20;
        for i in 0..STRIES {
            let t = (i as f32 + phase) / STRIES as f32;
            let angle = t * std::f32::consts::PI;
            let y = milieu - demi * angle.cos() * 0.92;
            // Au centre la strie est large et franche, au bord elle s'efface.
            let vu = angle.sin();
            peintre.line_segment(
                [pos2(x0, y), pos2(x1, y)],
                Stroke::new(
                    (roue.width() * 0.11 * vu).max(0.7),
                    ombre_col.gamma_multiply(0.25 + 0.55 * vu),
                ),
            );
        }

        // Un creux en haut et en bas, la ou la roue rentre dans son logement.
        for (haut, alpha) in [(true, 70u8), (false, 55)] {
            let bande = if haut {
                Rect::from_min_max(
                    roue.min,
                    pos2(roue.max.x, roue.min.y + roue.height() * 0.16),
                )
            } else {
                Rect::from_min_max(
                    pos2(roue.min.x, roue.max.y - roue.height() * 0.16),
                    roue.max,
                )
            };
            peintre.rect_filled(
                bande,
                arrondi * 0.5,
                Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
            );
        }
        peintre.rect_stroke(roue, arrondi, Stroke::new(1.5, ombre_col));

        commandes
    }
}
