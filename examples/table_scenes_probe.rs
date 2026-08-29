//! Extrait la table des scenes d'une edition, sans rien executer.
//!
//! Usage : cargo run --release --example table_scenes_probe --
//!             <dump.bin> <cle hex> [motif]
//!
//! Les numeros de scene ont ete releves sur l'image Earth, et Jade Forest a sa
//! zone de travail decalee : les reprendre d'une edition a l'autre mene a
//! conclure sur un numero qui ne veut plus rien dire. Cette sonde va donc les
//! chercher dans l'image qu'on lui donne.
//!
//! La methode ne suppose rien de la disposition du descripteur. Elle part des
//! chaines `PSID_`, qui sont en clair dans l'image, cherche les mots de trente
//! deux bits qui pointent dessus, et retient le pas qui revient le plus souvent
//! entre deux pointeurs voisins : c'est la taille du descripteur. Le champ qui
//! porte le numero se deduit ensuite en cherchant, dans le descripteur, le mot
//! dont la valeur suit le rang de l'entree.
//!
//! `XIP_BASE=0x...` change la base de la fenetre, comme pour `dis_probe`.
//!
//! Un motif en troisieme argument ne garde que les scenes dont le nom le
//! contient, sans egard a la casse.

use std::collections::{BTreeMap, HashMap};

use capybara::emulator::Machine;

/// Une entree de la table, telle qu'on la lit.
struct Scene {
    numero: u32,
    nom: String,
    descripteur: u32,
    mots: Vec<u32>,
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let motif = a.next().map(|v| v.to_lowercase());

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();

    let base = std::env::var("XIP_BASE")
        .ok()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x6001_1000);
    let decalage = (base & 0x00FF_FFFF) as usize;
    let flash = &m.bus.flash.data;
    println!("== image de {} octets, fenetre XIP a {:#010x}", flash.len(), base);

    // L'adresse vue par le firmware pour un octet de l'image, et l'inverse.
    let adresse = |off: usize| -> Option<u32> {
        off.checked_sub(decalage).map(|d| 0x1000_0000u32.wrapping_add(d as u32))
    };

    // 1. Les chaines. Elles sont en clair et terminees par un zero.
    let mut noms: HashMap<u32, String> = HashMap::new();
    let motif_chaine = b"PSID_";
    let mut i = 0usize;
    while i + motif_chaine.len() < flash.len() {
        if &flash[i..i + motif_chaine.len()] == motif_chaine {
            let mut fin = i;
            while fin < flash.len() && flash[fin] != 0 && fin - i < 64 {
                fin += 1;
            }
            if fin < flash.len() && flash[fin] == 0 {
                if let Ok(nom) = std::str::from_utf8(&flash[i..fin]) {
                    if nom.chars().all(|c| c.is_ascii_graphic()) {
                        if let Some(adr) = adresse(i) {
                            noms.insert(adr, nom.to_string());
                        }
                    }
                }
            }
            i = fin.max(i + 1);
        } else {
            i += 1;
        }
    }
    println!("== {} chaines PSID_ trouvees", noms.len());
    if noms.is_empty() {
        println!("   rien a faire : verifier la cle et la base XIP");
        return;
    }

    // 2. Les mots qui pointent dessus. On note l'offset du pointeur.
    let mut pointeurs: Vec<(usize, u32)> = Vec::new();
    let mut off = 0usize;
    while off + 4 <= flash.len() {
        let mot = u32::from_le_bytes([flash[off], flash[off + 1], flash[off + 2], flash[off + 3]]);
        if noms.contains_key(&mot) {
            pointeurs.push((off, mot));
        }
        off += 4;
    }
    println!("== {} pointeurs vers ces chaines", pointeurs.len());

    // 3. Le pas qui revient le plus souvent entre deux pointeurs voisins.
    let mut pas: BTreeMap<usize, usize> = BTreeMap::new();
    for f in pointeurs.windows(2) {
        let d = f[1].0 - f[0].0;
        if d > 0 && d <= 256 {
            *pas.entry(d).or_insert(0) += 1;
        }
    }
    let Some((&taille, &combien)) = pas.iter().max_by_key(|(_, n)| **n) else {
        println!("   pas de pas dominant : la table n'est pas un tableau regulier");
        return;
    };
    println!("== descripteur de {} octets, {} fois de suite", taille, combien);

    // 4. La plus longue suite a ce pas : c'est la table.
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
    let champ_nom = suite[0].0 % taille;
    let mut base_table = suite[0].0 - champ_nom;

    // La suite commence au premier nom trouve, pas forcement a la premiere
    // entree : une entree dont le nom ne commence pas par PSID_ passerait
    // inapercue et decalerait tous les numeros d'un rang. On remonte donc tant
    // que le descripteur precedent porte des gestionnaires plausibles.
    let mot_a = |o: usize| -> u32 {
        if o + 4 <= flash.len() {
            u32::from_le_bytes([flash[o], flash[o + 1], flash[o + 2], flash[o + 3]])
        } else {
            0
        }
    };
    let plausible = |d: usize| -> bool {
        // Deux gestionnaires en fenetre XIP, et un pointeur de nom qui vise
        // l'image : c'est la signature d'un descripteur.
        let g1 = mot_a(d);
        let g2 = mot_a(d + 4);
        let n = mot_a(d + champ_nom);
        (0x1000_0000..0x1010_0000).contains(&g1)
            && (0x1000_0000..0x1010_0000).contains(&g2)
            && (0x1000_0000..0x1010_0000).contains(&n)
    };
    let mut remontees = 0usize;
    while base_table >= taille && plausible(base_table - taille) {
        base_table -= taille;
        remontees += 1;
    }
    if remontees > 0 {
        println!("== {} entrees sans nom PSID_ en tete de table", remontees);
    }
    println!(
        "== table a {:#010x}, {} entrees, pointeur de nom au champ +{:#04x}\n",
        adresse(base_table).unwrap_or(0),
        suite.len(),
        champ_nom
    );

    // 5. Le champ du numero : celui dont la valeur suit le rang de l'entree.
    let mots_par_entree = taille / 4;
    let lire = |o: usize| -> u32 {
        if o + 4 <= flash.len() {
            u32::from_le_bytes([flash[o], flash[o + 1], flash[o + 2], flash[o + 3]])
        } else {
            0
        }
    };
    // On ne demande pas une suite parfaitement croissante : une edition peut
    // avoir insere une scene et rompu l'ordre. On prend le champ qui tient sur
    // seize bits et qui croit le plus souvent d'une entree a la suivante.
    let mut champ_numero = None;
    let mut meilleur = 0usize;
    for c in 0..mots_par_entree {
        let o = c * 4;
        if o == champ_nom {
            continue;
        }
        if suite.iter().any(|(p, _)| lire(p - champ_nom + o) >= 0x1_0000) {
            continue;
        }
        let montees = suite
            .windows(2)
            .filter(|w| lire(w[0].0 - champ_nom + o) < lire(w[1].0 - champ_nom + o))
            .count();
        if montees > meilleur {
            meilleur = montees;
            champ_numero = Some(o);
        }
    }
    if let Some(o) = champ_numero {
        println!("   ce champ croit sur {} des {} paires voisines", meilleur, suite.len() - 1);
        let _ = o;
    }
    let Some(champ_numero) = champ_numero else {
        println!("   aucun champ ne se comporte comme un numero, voici les descripteurs bruts");
        for (p, adr) in suite.iter().take(8) {
            let d = p - champ_nom;
            let mots: Vec<String> =
                (0..mots_par_entree).map(|c| format!("{:#010x}", lire(d + c * 4))).collect();
            println!("  {}  {}", noms[adr], mots.join(" "));
        }
        return;
    };
    println!("== numero au champ +{:#04x}\n", champ_numero);

    // 6. La table, parcourue par rang et non par nom trouve. Le rang est ce que
    // le firmware ecrit dans `0x18001BF4` : c'est lui qui fait foi. Le champ
    // `+0x10` le suit, mais rien ne garantit qu'il ne saute jamais, et un
    // desaccord entre les deux est precisement ce qu'il faut voir.
    let entrees = remontees + suite.len();
    let scenes: Vec<Scene> = (0..entrees)
        .map(|rang| {
            let d = base_table + rang * taille;
            let p = lire(d + champ_nom);
            Scene {
                numero: rang as u32,
                nom: noms.get(&p).cloned().unwrap_or_else(|| format!("<{:#010x}>", p)),
                descripteur: adresse(d).unwrap_or(0),
                mots: (0..mots_par_entree).map(|c| lire(d + c * 4)).collect(),
            }
        })
        .collect();

    let desaccords: Vec<&Scene> =
        scenes.iter().filter(|s| s.mots[champ_numero / 4] != s.numero).collect();
    if desaccords.is_empty() {
        println!("== le champ +{:#04x} vaut le rang partout\n", champ_numero);
    } else {
        println!("== {} entrees ou le champ +{:#04x} differe du rang", desaccords.len(), champ_numero);
        for s in desaccords.iter().take(10) {
            println!("   rang {:>3}  champ {:>3}  {}", s.numero, s.mots[champ_numero / 4], s.nom);
        }
        println!();
    }

    let gardees: Vec<&Scene> = scenes
        .iter()
        .filter(|s| motif.as_ref().is_none_or(|f| s.nom.to_lowercase().contains(f)))
        .collect();

    println!("{:>5}  {:<32} {:<12} gestionnaires", "rang", "nom", "descripteur");
    for s in &gardees {
        let mains: Vec<String> = s
            .mots
            .iter()
            .enumerate()
            .filter(|(c, v)| c * 4 != champ_nom && c * 4 != champ_numero && **v >= 0x1000_0000)
            .map(|(_, v)| format!("{:#010x}", v))
            .collect();
        println!(
            "{:>5}  {:<32} {:#010x}  {}",
            s.numero,
            s.nom,
            s.descripteur,
            mains.join(" ")
        );
    }
    println!("\n== {} scenes affichees sur {}", gardees.len(), scenes.len());
}
