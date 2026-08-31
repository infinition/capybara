//! Correspondance entre les touches du clavier et les commandes de la console.
//!
//! Chaque commande accepte plusieurs touches : le boitier n'a que trois boutons
//! et une molette, mais on veut pouvoir les atteindre aussi bien avec les
//! lettres qu'avec les fleches. Plusieurs touches tenues ensemble donnent les
//! combinaisons de la console, A plus C pour la remise a zero par exemple :
//! chaque broche est evaluee separement, rien ne s'exclut.
//!
//! Les touches sont rangees par leur nom et non par leur valeur : le nom
//! traverse une mise a jour de la bibliotheque graphique, une valeur non.

use egui::Key;

/// Les six commandes qu'on peut remapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commande {
    BoutonA,
    BoutonB,
    BoutonC,
    Molette,
    TournerDroite,
    TournerGauche,
}

impl Commande {
    pub const TOUTES: [Commande; 6] = [
        Commande::BoutonA,
        Commande::BoutonB,
        Commande::BoutonC,
        Commande::Molette,
        Commande::TournerDroite,
        Commande::TournerGauche,
    ];

    /// Libelle affiche, en francais puis en anglais.
    pub fn libelle(self) -> (&'static str, &'static str) {
        match self {
            Commande::BoutonA => ("Bouton A", "Button A"),
            Commande::BoutonB => ("Bouton B", "Button B"),
            Commande::BoutonC => ("Bouton C", "Button C"),
            Commande::Molette => ("Clic molette", "Dial click"),
            Commande::TournerDroite => ("Molette vers la droite", "Dial right"),
            Commande::TournerGauche => ("Molette vers la gauche", "Dial left"),
        }
    }
}

/// Touches par defaut, celles qui valaient avant que la table soit reglable.
fn defaut(commande: Commande) -> &'static [&'static str] {
    match commande {
        Commande::BoutonA => &["A", "Q", "ArrowLeft"],
        Commande::BoutonB => &["B", "Space", "Num2"],
        Commande::BoutonC => &["C", "D", "ArrowRight"],
        Commande::Molette => &["Enter", "S", "Num0"],
        Commande::TournerDroite => &["ArrowUp"],
        Commande::TournerGauche => &["ArrowDown"],
    }
}

/// La table complete, telle qu'elle est enregistree.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Touches {
    pub bouton_a: Vec<String>,
    pub bouton_b: Vec<String>,
    pub bouton_c: Vec<String>,
    pub molette: Vec<String>,
    pub tourner_droite: Vec<String>,
    pub tourner_gauche: Vec<String>,
}

impl Default for Touches {
    fn default() -> Self {
        let liste = |c| defaut(c).iter().map(|s| s.to_string()).collect();
        Self {
            bouton_a: liste(Commande::BoutonA),
            bouton_b: liste(Commande::BoutonB),
            bouton_c: liste(Commande::BoutonC),
            molette: liste(Commande::Molette),
            tourner_droite: liste(Commande::TournerDroite),
            tourner_gauche: liste(Commande::TournerGauche),
        }
    }
}

impl Touches {
    fn champ(&self, commande: Commande) -> &Vec<String> {
        match commande {
            Commande::BoutonA => &self.bouton_a,
            Commande::BoutonB => &self.bouton_b,
            Commande::BoutonC => &self.bouton_c,
            Commande::Molette => &self.molette,
            Commande::TournerDroite => &self.tourner_droite,
            Commande::TournerGauche => &self.tourner_gauche,
        }
    }

    fn champ_mut(&mut self, commande: Commande) -> &mut Vec<String> {
        match commande {
            Commande::BoutonA => &mut self.bouton_a,
            Commande::BoutonB => &mut self.bouton_b,
            Commande::BoutonC => &mut self.bouton_c,
            Commande::Molette => &mut self.molette,
            Commande::TournerDroite => &mut self.tourner_droite,
            Commande::TournerGauche => &mut self.tourner_gauche,
        }
    }

    /// Noms des touches d'une commande, tels qu'ils s'affichent.
    pub fn noms(&self, commande: Commande) -> &[String] {
        self.champ(commande)
    }

    /// Touches d'une commande. Les noms inconnus sont ignores plutot que de
    /// faire echouer toute la table : un fichier ecrit par une autre version
    /// reste utilisable.
    pub fn cles(&self, commande: Commande) -> Vec<Key> {
        self.champ(commande)
            .iter()
            .filter_map(|nom| Key::from_name(nom))
            .collect()
    }

    /// Ajoute une touche a une commande, si elle n'y est pas deja.
    ///
    /// Elle est d'abord retiree de toutes les autres : une meme touche ne peut
    /// pas commander deux choses, sinon un appui en declencherait deux.
    pub fn ajouter(&mut self, commande: Commande, touche: Key) {
        let nom = touche.name().to_string();
        for autre in Commande::TOUTES {
            self.champ_mut(autre).retain(|n| n != &nom);
        }
        self.champ_mut(commande).push(nom);
    }

    /// Retire une touche d'une commande.
    pub fn retirer(&mut self, commande: Commande, nom: &str) {
        self.champ_mut(commande).retain(|n| n != nom);
    }

    /// Remet une commande a ses touches d'origine.
    pub fn reinitialiser(&mut self, commande: Commande) {
        let valeurs: Vec<String> = defaut(commande).iter().map(|s| s.to_string()).collect();
        for nom in &valeurs {
            for autre in Commande::TOUTES {
                if autre != commande {
                    self.champ_mut(autre).retain(|n| n != nom);
                }
            }
        }
        *self.champ_mut(commande) = valeurs;
    }
}

/// Commande de la console qu'un bouton de souris peut declencher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Bouton {
    Aucun,
    A,
    B,
    C,
    Molette,
}

impl Bouton {
    pub const TOUS: [Bouton; 5] = [Bouton::Aucun, Bouton::A, Bouton::B, Bouton::C, Bouton::Molette];

    pub fn libelle(self) -> (&'static str, &'static str) {
        match self {
            Bouton::Aucun => ("rien", "none"),
            Bouton::A => ("A", "A"),
            Bouton::B => ("B", "B"),
            Bouton::C => ("C", "C"),
            Bouton::Molette => ("Molette", "Dial"),
        }
    }
}

/// Ce que font les trois boutons de la souris quand on clique sur l'ecran.
///
/// Viser les petits boutons dessines n'est pas toujours commode : cliquer
/// n'importe ou sur l'ecran declenche une commande, et chacun choisit
/// laquelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Souris {
    pub primaire: Bouton,
    pub secondaire: Bouton,
    pub milieu: Bouton,
}

impl Default for Souris {
    fn default() -> Self {
        Self {
            primaire: Bouton::A,
            secondaire: Bouton::C,
            milieu: Bouton::Molette,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_touches_par_defaut_se_relisent() {
        let t = Touches::default();
        assert!(t.cles(Commande::BoutonA).contains(&Key::A));
        assert!(t.cles(Commande::BoutonB).contains(&Key::Space));
        assert!(t.cles(Commande::TournerDroite).contains(&Key::ArrowUp));
    }

    #[test]
    fn une_touche_ne_sert_qu_a_une_commande() {
        let mut t = Touches::default();
        t.ajouter(Commande::BoutonC, Key::A);
        assert!(!t.cles(Commande::BoutonA).contains(&Key::A));
        assert!(t.cles(Commande::BoutonC).contains(&Key::A));
    }

    #[test]
    fn un_nom_inconnu_est_ignore_sans_tout_perdre() {
        let mut t = Touches::default();
        t.bouton_a.push("TouchePasDeCeMonde".to_string());
        assert!(t.cles(Commande::BoutonA).contains(&Key::A));
    }

    #[test]
    fn la_remise_a_zero_rend_les_touches_d_origine() {
        let mut t = Touches::default();
        t.ajouter(Commande::BoutonC, Key::A);
        t.reinitialiser(Commande::BoutonA);
        assert!(t.cles(Commande::BoutonA).contains(&Key::A));
        assert!(!t.cles(Commande::BoutonC).contains(&Key::A));
    }
}
