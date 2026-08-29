//! Sonde d'instantane : repart d'un etat sauvegarde par l'interface et rend
//! compte de ce que le firmware fait ensuite.
//!
//! Usage : cargo run --release --example etat_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [pas]
//!
//! C'est le moyen le plus court d'examiner un blocage signale par un joueur :
//! il n'y a pas a refaire toute la mise en route du jeu.

use capybara::emulator::etat::Instantane;
use capybara::emulator::{Machine, StepResult};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let budget: u64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(200_000_000);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    let etat = Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat");
    m.restaurer(&etat);

    let lire_h = |m: &Machine, a: u32| -> u32 {
        let o = (a - 0x1800_0000) as usize;
        m.bus.sram.read_u8(o) as u32 | ((m.bus.sram.read_u8(o + 1) as u32) << 8)
    };

    println!("== etat restaure");
    println!("  pas executes  {}", m.cpu.cycles);
    println!("  PC            {:#010x}", m.cpu.regs.pc);
    println!("  trames ecran  {}", m.periph.display.trames);
    println!(
        "  etat du jeu   courant {}   transition demandee {}",
        lire_h(&m, 0x1800_1BF4),
        lire_h(&m, 0x1800_1BF6)
    );

    // Ce que le firmware parcourt ensuite, pour trouver la boucle ou il tourne.
    // ENTREES="pas:broche:duree,..." rejoue des appuis, en pas comptes depuis
    // la restauration. La broche porte l'identifiant du firmware.
    let mut appuis: Vec<(u64, u32, u64)> = std::env::var("ENTREES")
        .ok()
        .map(|v| {
            v.split(',')
                .filter_map(|e| {
                    let c: Vec<&str> = e.split(':').collect();
                    if c.len() != 3 {
                        return None;
                    }
                    Some((
                        c[0].parse().ok()?,
                        u32::from_str_radix(c[1].trim_start_matches("0x"), 16).ok()?,
                        c[2].parse().ok()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    appuis.sort_by_key(|a| a.0);
    let mut relachements: Vec<(u64, u32)> = Vec::new();
    // Etats du jeu traverses, dans l'ordre : c'est ce qui dit si l'on avance.
    let mut parcours: Vec<u32> = vec![lire_h(&m, 0x1800_1BF4)];

    let mut broches: std::collections::BTreeMap<u32, u64> = std::collections::BTreeMap::new();
    let mut hist: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    let trames_depart = m.periph.display.trames;
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
        if pas % 100_000 == 0 {
            let courant = lire_h(&m, 0x1800_1BF4);
            if parcours.last() != Some(&courant) {
                parcours.push(courant);
            }
        }
        // Quelles broches le firmware lit vraiment, et combien de fois. C'est
        // la seule facon de savoir si un bouton est seulement scrute.
        if m.cpu.regs.pc == 0x0000_2714 {
            *broches.entry(m.cpu.regs.get_reg(0)).or_default() += 1u64;
        }
        *hist.entry(m.cpu.regs.pc).or_default() += 1;
        if !matches!(m.step(), StepResult::Ok(_)) {
            println!("  arret a {:#010x}", m.cpu.regs.pc);
            break;
        }
        pas += 1;
    }
    println!("\n== etats du jeu traverses : {:?}", parcours);

    println!("\n== apres {} pas de plus", pas);
    println!(
        "  trames ecran  {} (+{})",
        m.periph.display.trames,
        m.periph.display.trames - trames_depart
    );
    println!(
        "  etat du jeu   courant {}   transition demandee {}",
        lire_h(&m, 0x1800_1BF4),
        lire_h(&m, 0x1800_1BF6)
    );
    println!("  PC            {:#010x}", m.cpu.regs.pc);

    println!("\n== broches lues par le firmware");
    if broches.is_empty() {
        println!("  aucune");
    }
    for (broche, nombre) in &broches {
        println!("  {:#04x}  {:>10} lectures", broche, nombre);
    }

    let mut chaud: Vec<_> = hist.into_iter().collect();
    chaud.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    chaud.truncate(12);
    println!("\n== adresses les plus executees");
    for (adresse, nombre) in chaud {
        println!("  {:#010x}  {:>10} fois", adresse, nombre);
    }

    let couleurs: std::collections::HashSet<u16> = m.periph.display.vram.iter().copied().collect();
    println!("\n== ecran : {} couleurs distinctes", couleurs.len());

    // ECRAN=chemin.ppm rend l'image, pour voir ou en est le jeu.
    if let Ok(chemin) = std::env::var("ECRAN") {
        use std::io::Write;
        let d = &m.periph.display;
        let mut octets = Vec::with_capacity(d.vram.len() * 3);
        for &px in &d.vram {
            let r = ((px >> 11) & 0x1F) as u8;
            let v = ((px >> 5) & 0x3F) as u8;
            let b = (px & 0x1F) as u8;
            octets.push((r << 3) | (r >> 2));
            octets.push((v << 2) | (v >> 4));
            octets.push((b << 3) | (b >> 2));
        }
        if let Ok(mut f) = std::fs::File::create(&chemin) {
            let _ = write!(f, "P6\n{} {}\n255\n", d.width, d.height);
            let _ = f.write_all(&octets);
            println!("  image ecrite dans {}", chemin);
        }
    }

    let fin: String = m.console.chars().rev().take(400).collect::<Vec<_>>().into_iter().rev().collect();
    println!("\n== console du firmware (fin)\n{}", fin.trim_end());
}
