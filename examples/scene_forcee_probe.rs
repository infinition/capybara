//! Force une scene par la machine a etats, et releve ce qu'elle reveille.
//!
//! Usage : cargo run --release --example scene_forcee_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> <scene> [secondes]
//!
//! Le numero de scene est le rang dans la table, celui que `table_scenes_probe`
//! rend. Attention : les numeros consignes avant le 29 aout 2026 venaient du
//! champ `+0x10` du descripteur et valent un de trop.
//!
//! Poser le numero voulu en `0x18001BF6` ne declenche rien, c'est mesure. La
//! machine a scenes marche autrement. Elle garde son etat dans les trois bits
//! bas de `0x18001BFA` :
//!
//! ```text
//!   0  entree : lit la scene en 0x18001BF4, cherche son descripteur, appelle
//!      le premier gestionnaire, puis passe a 1
//!   1  marche : appelle le gestionnaire de boucle a chaque tour
//!   2  sortie et veille
//! ```
//!
//! Ecrire la scene voulue en `0x18001BF4` et remettre ces trois bits a zero
//! revient donc a lui demander d'entrer dans cette scene au prochain tour. La
//! scene quittee n'est pas defaite proprement : ce qu'elle avait pris sur le
//! tas y reste. C'est acceptable pour une sonde, pas pour le jeu.
//!
//! `SORTIE=chemin.ppm` rend l'ecran atteint. `SORTIE_ETAT=chemin.tamastate`
//! garde l'etat, pour repartir de la sans refaire le trajet.

use std::collections::BTreeSet;
use std::io::Write;

use capybara::emulator::etat::Instantane;
use capybara::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;
const SCENE: u32 = 0x1800_1BF4;
const TRANSITION: u32 = 0x1800_1BF6;
const PRECEDENTE: u32 = 0x1800_1BF8;
const ETAT_MACHINE: u32 = 0x1800_1BFA;

/// L'instruction qui relit ce que le gestionnaire de boucle a rendu. Une valeur
/// non nulle veut dire « j'ai fini, sors moi de la ». Le mot est en `sp + 20`.
const TEST_SORTIE: u32 = 0x0000_97F2;
/// Deplacement du mot rendu dans le cadre de la machine a scenes.
const RENDU: u32 = 20;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let cible: u16 = a.next().and_then(|v| v.parse().ok()).expect("numero de scene");
    let secondes: f64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(8.0);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));
    m.bus.mmio_trace.enabled = true;

    // RESET=1 rallume la console sur la flash de l'instantane, donc avec sa
    // sauvegarde, au lieu de reprendre l'execution ou elle en etait. C'est le
    // seul moyen d'atteindre une scene lourde : depuis une scene de jeu, qui
    // occupe trente et un des trente deux kilo-octets du tas, l'entree dans une
    // autre scene ne trouve pas de place et saute a l'assertion.
    if std::env::var("RESET").is_ok() {
        m.reset();
        m.is_running = true;
        m.console.clear();
        println!("== console rallumee sur la flash de l'instantane");
    }

    // ATTENTE=secondes laisse la mise en route se derouler avant de forcer.
    let attente: f64 = std::env::var("ATTENTE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    avancer(&mut m, attente);
    let avant = pages(&m);
    println!(
        "== depart : scene {}, precedente {}, etat {}, {} pages",
        lire16(&m, SCENE),
        lire16(&m, PRECEDENTE),
        lire8(&m, ETAT_MACHINE) & 7,
        avant.len()
    );

    // BRUTAL=1 ecrit la scene et remet l'etat a l'entree. La machine obeit,
    // mais la scene quittee n'a rien rendu et l'allocateur saute au halt. Garde
    // pour memoire, et pour eprouver a nouveau si la sortie propre casse.
    let brutal = std::env::var("BRUTAL").is_ok();
    ecrire16(&mut m, TRANSITION, cible);
    if brutal {
        let quittee = lire16(&m, SCENE);
        ecrire16(&mut m, PRECEDENTE, quittee);
        ecrire16(&mut m, SCENE, cible);
        ecrire16(&mut m, TRANSITION, 0xFFFF);
        let drapeaux = lire8(&m, ETAT_MACHINE) & !7;
        ecrire8(&mut m, ETAT_MACHINE, drapeaux);
        println!("== scene {} posee de force, etat remis a l'entree\n", cible);
    } else {
        println!("== scene {} demandee, sortie de la scene courante forcee\n", cible);
    }

    // ARRET=0x... fige la trace a la premiere arrivee sur une adresse. Par
    // defaut c'est l'entree du halt fatal de Jade Forest.
    let arret = std::env::var("ARRET")
        .ok()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .or(Some(0x1005_E904));
    let mut suivi = Suivi::new(arret);
    suivi.forcer_sortie = !brutal;
    let mut atteinte = false;
    for etape in 1..=(secondes.ceil() as u32) {
        avancer_en_comptant(&mut m, 1.0, &mut suivi);
        let scene = lire16(&m, SCENE);
        let apres = pages(&m);
        let neuves: Vec<u32> = apres.difference(&avant).copied().collect();
        println!(
            "  a {} s : scene {}, etat {}, {} pages, {} nouvelles",
            etape,
            scene,
            lire8(&m, ETAT_MACHINE) & 7,
            apres.len(),
            neuves.len()
        );
        for page in &neuves {
            println!(
                "      {:#010x}  {}",
                page,
                capybara::emulator::mmu::periph::name_of(*page)
            );
        }
        if scene == cible && !atteinte {
            println!("      (la scene tient)");
            atteinte = true;
        }
    }

    if !atteinte {
        println!("\n== la scene n'a pas tenu : le firmware est reparti ailleurs");
    }

    if let Some((trace, lr)) = &suivi.capture {
        println!("\n== chemin jusqu'a {:#010x}, LR {:#010x}", arret.unwrap_or(0), lr);
        // Les repetitions n'apprennent rien : on ne garde que les sauts.
        let mut precedent = 0u32;
        for pc in trace {
            if pc.abs_diff(precedent) > 8 {
                println!("  {:#010x}", pc);
            }
            precedent = *pc;
        }
    } else if arret.is_some() {
        println!("\n== {:#010x} jamais atteinte", arret.unwrap_or(0));
    }

    println!("\n== adresses les plus executees");
    let total: u64 = suivi.compte.values().sum();
    let mut hist: Vec<_> = suivi.compte.into_iter().collect();
    hist.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (adresse, n) in hist.iter().take(12) {
        println!("  {:#010x}  {:>6.2} %", adresse, *n as f64 * 100.0 / total as f64);
    }

    println!("\n== registres les plus touches, hors des pages connues avant");
    let mut v: Vec<_> = m
        .bus
        .mmio_trace
        .all
        .iter()
        .filter(|(a, _)| !avant.contains(&(**a & !0xFFF)))
        .map(|(a, s)| (*a, *s))
        .collect();
    v.sort_by_key(|(_, s)| std::cmp::Reverse(s.reads + s.writes));
    for (adresse, s) in v.iter().take(24) {
        println!(
            "  {:#010x}  {:<10} lectures {:>8}  ecritures {:>8}  derniere {:#010x}  premier PC {:#010x}",
            adresse,
            capybara::emulator::mmu::periph::name_of(adresse & !0xFFF),
            s.reads,
            s.writes,
            s.last_write,
            s.first_pc
        );
    }

    if let Ok(chemin) = std::env::var("SORTIE_ETAT") {
        match m.instantane().ecrire(std::path::Path::new(&chemin)) {
            Ok(()) => println!("\n== etat ecrit dans {}", chemin),
            Err(e) => println!("\n== etat non ecrit : {}", e),
        }
    }
    if let Ok(chemin) = std::env::var("SORTIE") {
        ecrire_ppm(&m, &chemin);
    }
}

fn avancer(m: &mut Machine, secondes: f64) {
    let fin = m.cpu.cycles + (secondes * SECONDE) as u64;
    while m.cpu.cycles < fin {
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
    }
}

/// Ou le firmware passe son temps, et comment il y est arrive. Une scene qui ne
/// demarre pas et une boucle morte se ressemblent de l'exterieur : seul
/// l'histogramme les separe, et seule la trace dit pourquoi.
struct Suivi {
    /// Combien de fois chaque adresse a ete relevee par sondage.
    compte: std::collections::HashMap<u32, u64>,
    /// Adresse dont on veut le contexte d'arrivee, la premiere fois.
    arret: Option<u32>,
    /// Les dernieres adresses executees, gardees en continu.
    trace: std::collections::VecDeque<u32>,
    /// La trace figee a l'instant de l'arrivee, et le LR qui allait avec.
    capture: Option<(Vec<u32>, u32)>,
    /// Reste a poser le rendu de sortie sur le prochain passage du test.
    forcer_sortie: bool,
    /// Le pas ou la sortie a ete posee.
    sortie_posee: Option<u64>,
}

impl Suivi {
    fn new(arret: Option<u32>) -> Self {
        Self {
            compte: std::collections::HashMap::new(),
            arret,
            trace: std::collections::VecDeque::new(),
            capture: None,
            forcer_sortie: false,
            sortie_posee: None,
        }
    }
}

fn avancer_en_comptant(m: &mut Machine, secondes: f64, s: &mut Suivi) {
    let fin = m.cpu.cycles + (secondes * SECONDE) as u64;
    let mut n = 0u64;
    while m.cpu.cycles < fin {
        let pc = m.cpu.regs.pc;
        // Un releve sur mille suffit a designer une boucle morte, et coute
        // assez peu pour ne pas fausser la mesure de duree.
        if n % 1000 == 0 {
            *s.compte.entry(pc).or_insert(0) += 1;
        }
        n += 1;
        // Le seul geste de la sonde : dire une fois au gestionnaire de scenes
        // que la scene courante a fini. Tout le reste, le demontage qui rend la
        // memoire et la bascule vers la scene demandee, est fait par le
        // firmware lui meme.
        if s.forcer_sortie && pc == TEST_SORTIE {
            let o = (m.cpu.regs.get_sp() + RENDU - 0x1800_0000) as usize;
            if o + 4 <= m.bus.sram.data.len() {
                m.bus.sram.data[o..o + 4].copy_from_slice(&1u32.to_le_bytes());
                s.forcer_sortie = false;
                s.sortie_posee = Some(m.cpu.cycles);
            }
        }
        if s.capture.is_none() {
            s.trace.push_back(pc);
            if s.trace.len() > 60 {
                s.trace.pop_front();
            }
            if s.arret == Some(pc) {
                s.capture = Some((s.trace.iter().copied().collect(), m.cpu.regs.lr));
            }
        }
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
    }
}

fn pages(m: &Machine) -> BTreeSet<u32> {
    m.bus.mmio_trace.all.keys().map(|a| a & !0xFFF).collect()
}

fn lire16(m: &Machine, adresse: u32) -> u16 {
    let o = (adresse - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    if o + 2 > d.len() {
        return 0;
    }
    u16::from_le_bytes([d[o], d[o + 1]])
}

fn lire8(m: &Machine, adresse: u32) -> u8 {
    let o = (adresse - 0x1800_0000) as usize;
    m.bus.sram.data.get(o).copied().unwrap_or(0)
}

fn ecrire16(m: &mut Machine, adresse: u32, valeur: u16) {
    let o = (adresse - 0x1800_0000) as usize;
    let d = &mut m.bus.sram.data;
    if o + 2 <= d.len() {
        d[o..o + 2].copy_from_slice(&valeur.to_le_bytes());
    }
}

fn ecrire8(m: &mut Machine, adresse: u32, valeur: u8) {
    let o = (adresse - 0x1800_0000) as usize;
    if let Some(c) = m.bus.sram.data.get_mut(o) {
        *c = valeur;
    }
}

fn ecrire_ppm(m: &Machine, sortie: &str) {
    let largeur = 128u32;
    let vram = &m.periph.display.vram;
    let unites = (largeur * largeur) as usize;
    let mut donnees = Vec::with_capacity(unites * 3);
    for i in 0..unites {
        let px = vram.get(i).copied().unwrap_or(0);
        let r = ((px >> 11) & 0x1F) as u8;
        let v = ((px >> 5) & 0x3F) as u8;
        let b = (px & 0x1F) as u8;
        donnees.push((r << 3) | (r >> 2));
        donnees.push((v << 2) | (v >> 4));
        donnees.push((b << 3) | (b >> 2));
    }
    let mut f = std::fs::File::create(sortie).expect("creation du fichier");
    write!(f, "P6\n{} {}\n255\n", largeur, largeur).unwrap();
    f.write_all(&donnees).unwrap();
    let distinctes: std::collections::HashSet<&[u8]> = donnees.chunks(3).collect();
    println!("== ecran ecrit dans {}, {} couleurs distinctes", sortie, distinctes.len());
}
