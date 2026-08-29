//! Sonde de scene : repart d'un instantane, rejoue des appuis en temps console,
//! et rend a la fois l'ecran et les variables d'etat du jeu.
//!
//! Usage : cargo run --release --example scene_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [sortie.ppm] [secondes]
//!
//! `ENTREES` prend ici des secondes, pas des pas : `0.5:9:0.2` appuie sur A a
//! une demi-seconde pendant deux dixiemes. Raisonner en pas obligeait a
//! recalculer a chaque changement de budget.

use std::io::Write;

use capybara::emulator::etat::Instantane;
use capybara::emulator::{Machine, StepResult};

/// Cycles du coeur pour une seconde de temps console, le SysTick etant arme a
/// 95999 pour une milliseconde.
const SECONDE: f64 = 96_000_000.0;

/// Variables d'etat du jeu retrouvees par comparaison de deux instantanes.
mod jeu {
    /// Scene courante, sur deux octets. La precedente suit en 0x18001BF8.
    pub const SCENE: u32 = 0x1800_1BF4;
    /// Drapeaux de la boucle principale. Le bit 4 marque la mise en veille.
    pub const DRAPEAUX: u32 = 0x1800_1BFB;
    /// Compteur d'inactivite, sur deux octets, remis a zero par tout appui.
    pub const INACTIVITE: u32 = 0x1800_1BFE;
    /// Calendrier du jeu : annee sur deux octets, puis mois, jour, heure,
    /// minute, seconde.
    pub const HORLOGE: u32 = 0x1800_1BA4;
    /// Deuxieme exemplaire du calendrier, celui de la structure de sauvegarde.
    pub const HORLOGE_SAUVEE: u32 = 0x1800_0BB8;
    /// Masque des boutons tenus, tel que le pilote d'entrees le publie.
    pub const BOUTONS: u32 = 0x1800_1C1C;
}

/// Les six champs sont des demi-mots, pas des octets : annee, mois, jour,
/// heure, minute, seconde.
fn horloge(m: &Machine, base: u32) -> String {
    let o = (base - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    let champ = |i: usize| {
        let p = o + i * 2;
        d.get(p).copied().unwrap_or(0) as u32 | ((d.get(p + 1).copied().unwrap_or(0) as u32) << 8)
    };
    format!(
        "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
        champ(0),
        champ(1),
        champ(2),
        champ(3),
        champ(4),
        champ(5)
    )
}

fn demi(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn octet(m: &Machine, adr: u32) -> u8 {
    m.bus.sram.data.get((adr - 0x1800_0000) as usize).copied().unwrap_or(0)
}

fn etat_lisible(m: &Machine, quand: &str) {
    println!("== etat {}", quand);
    println!("  scene         {}", demi(m, jeu::SCENE));
    println!(
        "  drapeaux      {:#04x}   veille {}",
        octet(m, jeu::DRAPEAUX),
        if octet(m, jeu::DRAPEAUX) & 0x10 != 0 { "oui" } else { "non" }
    );
    println!("  inactivite    {}", demi(m, jeu::INACTIVITE));
    println!("  boutons       {:#04x}", octet(m, jeu::BOUTONS));
    println!("  horloge       {}", horloge(m, jeu::HORLOGE));
    println!("  horloge sauvee {}", horloge(m, jeu::HORLOGE_SAUVEE));
    println!("  compteur RTC  {} s", m.periph.snsys.secondes);
    for base in [jeu::HORLOGE, jeu::HORLOGE_SAUVEE, 0x1800_1BF0] {
        println!("  {:#010x}  {}", base, hexa(m, base, 24));
    }
}

/// Vidange lisible, pour ne pas avoir a deviner la disposition d'une structure.
fn hexa(m: &Machine, base: u32, n: usize) -> String {
    let o = (base - 0x1800_0000) as usize;
    m.bus.sram.data[o..(o + n).min(m.bus.sram.data.len())]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let sortie = a.next().unwrap_or_else(|| "ecran.ppm".into());
    let secondes: f64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(3.0);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));

    // RESET=1 rallume la console avec la flash de l'instantane, donc avec sa
    // sauvegarde. C'est le seul moyen de revoir l'entree en scene de jeu sans
    // rejouer toute la mise en route a la main.
    if std::env::var("RESET").is_ok() {
        m.reset();
        m.is_running = true;
        m.console.clear();
        println!("== console rallumee sur la flash de l'instantane");
    }

    // L'instantane rapporte son propre tic systeme ; SONIX_TIC doit primer,
    // sinon on ne peut pas eprouver une source d'interruption sur un etat deja
    // pris.
    m.periph.tic = capybara::emulator::peripherals::TicSysteme::default();

    // ENTREES="seconde:broche:duree,..." en temps console.
    let mut appuis: Vec<(u64, u32, u64)> = std::env::var("ENTREES")
        .ok()
        .map(|v| {
            v.split(',')
                .filter_map(|e| {
                    let c: Vec<&str> = e.split(':').collect();
                    if c.len() != 3 {
                        return None;
                    }
                    let quand: f64 = c[0].parse().ok()?;
                    let duree: f64 = c[2].parse().ok()?;
                    Some((
                        (quand * SECONDE) as u64,
                        u32::from_str_radix(c[1].trim_start_matches("0x"), 16).ok()?,
                        (duree * SECONDE) as u64,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    appuis.sort_by_key(|a| a.0);

    etat_lisible(&m, "au depart");

    // TRACE_PAS=N garde les N dernieres adresses executees. C'est la seule
    // lecture fiable du chemin qui mene a un arret, un desassemblage a froid se
    // decalant des qu'il traverse des donnees.
    let trace_len: usize = std::env::var("TRACE_PAS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut trace: std::collections::VecDeque<u32> = std::collections::VecDeque::new();

    let budget = (secondes * SECONDE) as u64;
    let mut relachements: Vec<(u64, u32)> = Vec::new();
    let mut pas = 0u64;
    while pas < budget {
        while appuis.first().is_some_and(|a| a.0 <= pas) {
            let (_, broche, duree) = appuis.remove(0);
            m.appuyer(broche);
            relachements.push((pas + duree, broche));
        }
        relachements.retain(|&(quand, broche)| {
            if quand <= pas {
                m.relacher(broche);
                false
            } else {
                true
            }
        });
        let avant = m.cpu.regs.pc;
        if trace_len > 0 {
            if trace.len() == trace_len {
                trace.pop_front();
            }
            trace.push_back(avant);
        }
        match m.step() {
            StepResult::Ok(_) => {}
            autre => {
                let raison = match autre {
                    StepResult::Undefined(op) => format!("instruction non decodee {:#06x}", op),
                    StepResult::Halt => "halt".to_string(),
                    StepResult::Breakpoint => "point d'arret".to_string(),
                    StepResult::Ok(_) => unreachable!(),
                };
                println!(
                    "arret a PC={:#010x} ({}), etape suivante {:#010x}",
                    avant, raison, m.cpu.regs.pc
                );
                println!("  registres : {}", (0..13)
                    .map(|i| format!("r{}={:#x}", i, m.cpu.regs.get_reg(i)))
                    .collect::<Vec<_>>()
                    .join(" "));
                println!("  SP={:#010x} LR={:#010x}", m.cpu.regs.get_sp(), m.cpu.regs.lr);
                if !trace.is_empty() {
                    println!("  derniers pas :");
                    for a in &trace {
                        println!("    {:#010x}", a);
                    }
                }
                break;
            }
        }
        pas += 1;
        // Un releve periodique dit si l'horloge suit le compteur ou decroche,
        // ce qu'un simple avant/apres ne distingue pas.
        if pas % (10.0 * SECONDE) as u64 == 0 {
            println!(
                "  a {:>5} s d'execution : compteur {:>5} s, horloge {}, scene {}, inactivite {}",
                (pas as f64 / SECONDE) as u64,
                m.periph.snsys.secondes,
                horloge(&m, jeu::HORLOGE),
                demi(&m, jeu::SCENE),
                demi(&m, jeu::INACTIVITE)
            );
        }
    }

    println!();
    etat_lisible(&m, &format!("apres {:.2} secondes", pas as f64 / SECONDE));

    // SORTIE_ETAT=chemin ecrit l'etat atteint. Rejouer vingt minutes de mise en
    // route a chaque essai coute trop cher : on la rejoue une fois, on garde le
    // resultat, et on repart de la.
    if let Ok(chemin) = std::env::var("SORTIE_ETAT") {
        match m.instantane().ecrire(std::path::Path::new(&chemin)) {
            Ok(()) => println!("== etat ecrit dans {}", chemin),
            Err(e) => println!("== etat non ecrit : {}", e),
        }
    }

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
    let mut f = std::fs::File::create(&sortie).expect("creation du fichier");
    write!(f, "P6\n{} {}\n255\n", largeur, largeur).unwrap();
    f.write_all(&donnees).unwrap();
    let distinctes: std::collections::HashSet<&[u8]> = donnees.chunks(3).collect();
    println!("\n== ecran ecrit dans {}, {} couleurs distinctes", sortie, distinctes.len());
}
