//! La table des scenes du firmware, telle qu'elle se lit dans l'image.
//!
//! Le firmware garde ses cent vingt neuf ecrans dans un tableau de descripteurs
//! de vingt huit octets, portant quatre gestionnaires, un pointeur de nom et un
//! compteur. Le numero de scene, celui qu'on lit en `0x18001BF4`, est le rang
//! dans ce tableau. Le champ `+0x10` vaut le rang plus un et n'est pas le
//! numero : les confondre decale toute la table d'une unite.
//!
//! On ne code pas les numeros en dur parce qu'ils changent d'une edition a
//! l'autre : Jade Forest compte trois ecrans de plus que Earth, et tout ce qui
//! suit l'insertion se decale.
//!
//! La recherche ne suppose rien de la disposition. Elle part des chaines
//! `PSID_`, en clair dans l'image, releve les mots de trente deux bits qui
//! pointent dessus, et retient le pas dominant entre deux pointeurs voisins.

/// Taille d'un descripteur, retenue comme pas dominant entre deux pointeurs.
const PAS_MAX: usize = 256;
/// Au dela, une chaine n'est pas un nom de scene.
const NOM_MAX: usize = 64;

pub struct TableScenes {
    /// Adresse du tableau vue par le firmware.
    pub base: u32,
    /// Les noms, indexes par numero de scene.
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
        let base_off = suite[0].0 - champ_nom;

        let noms: Vec<String> = (0..suite.len())
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
