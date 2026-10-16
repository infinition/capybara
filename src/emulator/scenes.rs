//! La table des scenes du firmware, telle qu'elle se lit dans l'image.
//!
//! Le firmware garde ses ecrans dans un tableau de descripteurs de vingt huit
//! octets, portant quatre gestionnaires, un pointeur de nom, un drapeau et le
//! numero de scene — celui qu'on lit en `0x18001BF4`.
//!
//! Ce numero est ecrit dans le descripteur et on le lit la. Le deduire du rang
//! dans le tableau paraissait equivalent, les deux coincidant sur l'edition
//! eau, mais le premier descripteur y porte un pointeur de nom invalide : la
//! recherche, qui part des noms, commencait donc au deuxieme et rendait toute
//! la table decalee d'une unite. La scene 29 s'affichait `HOME_SPACE` alors que
//! le firmware l'appelle `HOME`, et ainsi de suite jusqu'au bout.
//!
//! On ne code rien en dur parce que la disposition change d'une edition a
//! l'autre : Jade Forest compte des ecrans de plus que Earth, et tout ce qui
//! suit l'insertion se decale.
//!
//! La recherche ne suppose donc rien. Elle part des chaines `PSID_`, en clair
//! dans l'image, releve les mots de trente deux bits qui pointent dessus,
//! retient le pas dominant entre deux pointeurs voisins, puis cherche dans le
//! descripteur le champ qui progresse de un a chaque rang : c'est le numero.

/// Taille d'un descripteur, retenue comme pas dominant entre deux pointeurs.
const PAS_MAX: usize = 256;
/// Au dela, une chaine n'est pas un nom de scene.
const NOM_MAX: usize = 64;

pub struct TableScenes {
    /// Adresse du tableau vue par le firmware.
    pub base: u32,
    /// Les noms, indexes par numero de scene. Les trous — un descripteur dont
    /// le pointeur de nom ne mene nulle part — restent vides.
    pub noms: Vec<String>,
}

impl TableScenes {
    /// Cherche la table dans l'image. `xip_base` est la base de la fenetre XIP
    /// telle que le firmware l'a programmee : sans elle les pointeurs ne se
    /// ramenent pas a des offsets et rien ne correspond.
    pub fn reperer(flash: &[u8], xip_base: u32) -> Option<Self> {
        let decalage = (xip_base & 0x00FF_FFFF) as usize;
        let adresse =
            |off: usize| -> Option<u32> { off.checked_sub(decalage).map(|d| 0x1000_0000 + d as u32) };
        let mot = |o: usize| -> u32 {
            if o + 4 <= flash.len() {
                u32::from_le_bytes([flash[o], flash[o + 1], flash[o + 2], flash[o + 3]])
            } else {
                0
            }
        };

        // Les chaines, et l'adresse a laquelle le firmware les voit.
        let mut noms: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        let motif = b"PSID_";
        let mut i = 0usize;
        while i + motif.len() < flash.len() {
            if &flash[i..i + motif.len()] == motif {
                let mut fin = i;
                while fin < flash.len() && flash[fin] != 0 && fin - i < NOM_MAX {
                    fin += 1;
                }
                if fin < flash.len() && flash[fin] == 0 {
                    if let Ok(nom) = std::str::from_utf8(&flash[i..fin]) {
                        if nom.chars().all(|c| c.is_ascii_graphic()) {
                            if let Some(a) = adresse(i) {
                                noms.insert(a, nom.to_string());
                            }
                        }
                    }
                }
                i = fin.max(i + 1);
            } else {
                i += 1;
            }
        }
        if noms.len() < 16 {
            return None;
        }

        // Les mots qui pointent dessus.
        let mut pointeurs: Vec<(usize, u32)> = Vec::new();
        let mut off = 0usize;
        while off + 4 <= flash.len() {
            let m = mot(off);
            if noms.contains_key(&m) {
                pointeurs.push((off, m));
            }
            off += 4;
        }

        // Le pas dominant entre deux pointeurs voisins donne la taille du
        // descripteur.
        let mut pas: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        for f in pointeurs.windows(2) {
            let d = f[1].0 - f[0].0;
            if d > 0 && d <= PAS_MAX {
                *pas.entry(d).or_insert(0) += 1;
            }
        }
        let taille = *pas.iter().max_by_key(|(_, n)| **n)?.0;

        // La plus longue suite a ce pas est la table.
        let (mut debut, mut fin) = (0usize, 0usize);
        let (mut d, mut f) = (0usize, 0usize);
        while f < pointeurs.len() {
            if f + 1 < pointeurs.len() && pointeurs[f + 1].0 - pointeurs[f].0 == taille {
                f += 1;
            } else {
                if f - d > fin - debut {
                    debut = d;
                    fin = f;
                }
                f += 1;
                d = f;
            }
        }
        let suite = &pointeurs[debut..=fin];
        if suite.len() < 16 {
            return None;
        }
        let champ_nom = suite[0].0 % taille;
        let mut base_off = suite[0].0 - champ_nom;

        // Le champ qui porte le numero de scene. Sans lui il faudrait prendre le
        // rang dans la suite trouvee, ce qui revient au meme tant que la suite
        // commence au premier descripteur — mais sur l'edition eau le
        // descripteur zero porte un pointeur de nom invalide, la recherche
        // demarre donc au suivant, et toute la table se decale d'une unite.
        //
        // Le critere est celui de `table_scenes_probe`, pour que la sonde et
        // l'interface ne puissent pas dire deux choses differentes : le champ
        // qui tient sur seize bits et qui croit le plus souvent d'une entree a
        // la suivante. On ne demande pas une suite parfaitement croissante,
        // parce que le tableau se termine par un descripteur sentinelle dont le
        // numero ne suit pas, et parce qu'une edition peut avoir insere un ecran
        // et rompu l'ordre.
        let mut champ_numero = None;
        let mut meilleur = 0usize;
        for candidat in (0..taille).step_by(4) {
            if candidat == champ_nom {
                continue;
            }
            let hors_bornes = (0..suite.len())
                .any(|rang| mot(base_off + rang * taille + candidat) >= 0x1_0000);
            if hors_bornes {
                continue;
            }
            let montees = (1..suite.len())
                .filter(|&rang| {
                    mot(base_off + (rang - 1) * taille + candidat)
                        < mot(base_off + rang * taille + candidat)
                })
                .count();
            if montees > meilleur {
                meilleur = montees;
                champ_numero = Some(candidat);
            }
        }

        // Le tableau commence au descripteur numero zero, qui peut preceder le
        // premier nom trouve. On recule d'autant de descripteurs que vaut le
        // numero du premier, quand la place existe et que ce recul tombe encore
        // sur des descripteurs plausibles — sans quoi une edition ou le champ
        // aurait ete mal identifie se verrait decalee dans l'autre sens.
        let mut decale = 0usize;
        if let Some(c) = champ_numero {
            let premier = mot(base_off + c) as usize;
            let recul = premier.saturating_mul(taille);
            let plausible = premier > 0
                && premier < suite.len()
                && base_off >= recul
                && (1..=premier).all(|k| {
                    let d = base_off - k * taille;
                    mot(d + c) as usize == premier - k
                });
            if plausible {
                base_off -= recul;
                decale = premier;
            }
        }

        let total = suite.len() + decale;
        let noms: Vec<String> = (0..total)
            .map(|rang| {
                let p = mot(base_off + rang * taille + champ_nom);
                noms.get(&p).cloned().unwrap_or_default()
            })
            .collect();
        Some(Self { base: adresse(base_off)?, noms })
    }

    /// Le nom d'une scene, sans le prefixe `PSID_` qui n'apprend rien.
    pub fn nom(&self, rang: u16) -> Option<&str> {
        self.noms
            .get(rang as usize)
            .map(|n| n.strip_prefix("PSID_").unwrap_or(n))
            .filter(|n| !n.is_empty())
    }
}
