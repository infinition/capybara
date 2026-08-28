//! Sonde de source de temps : depuis un instantane, releve tout ce que le
//! firmware interroge sans modele derriere, avec le PC responsable.
//!
//! Usage : cargo run --release --example temps_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [secondes]
//!
//! Le calendrier du jeu ne bouge pas. Soit il est compte en logiciel a partir
//! d'un tic qu'on ne produit pas, soit il est lu sur un peripherique qu'on ne
//! modelise pas. Cette sonde tranche : elle nomme les registres scrutes et le
//! code qui les scrute.

use tamagotchi_paradise_rs::emulator::etat::Instantane;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

/// Cycles du coeur pour une seconde de temps console, le SysTick etant arme a
/// 95999 pour une milliseconde.
const SECONDE: u64 = 96_000_000;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let secondes: f64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(3.0);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));

    // La trace heritee de l'instantane melerait le demarrage a ce qu'on mesure.
    m.bus.mmio_trace.clear();
    m.bus.mmio_trace.enabled = true;
    // MMIO_PAGE=0x... journalise dans l'ordre les acces a une page, avec le PC
    // appelant : les compteurs seuls ne disent pas la sequence.
    m.bus.mmio_trace.log_page = std::env::var("MMIO_PAGE")
        .ok()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());

    // ENTREES="seconde:broche:duree,..." en temps console, comme scene_probe.
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
                        (quand * SECONDE as f64) as u64,
                        u32::from_str_radix(c[1].trim_start_matches("0x"), 16).ok()?,
                        (duree * SECONDE as f64) as u64,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    appuis.sort_by_key(|a| a.0);
    let mut relachements: Vec<(u64, u32)> = Vec::new();

    println!("== SysTick au depart");
    println!("  CSR {:#010x}  RVR {:#010x}  CVR {:#010x}", m.cpu.nvic.syst_csr, m.cpu.nvic.syst_rvr, m.cpu.nvic.syst_cvr);
    println!(
        "  compte {}   interruption {}",
        if m.cpu.nvic.syst_csr & 1 != 0 { "oui" } else { "non" },
        if m.cpu.nvic.syst_csr & 2 != 0 { "oui" } else { "non" }
    );
    print!("  interruptions autorisees :");
    for i in 0..240u32 {
        if m.cpu.nvic.iser[(i / 32) as usize] & (1 << (i % 32)) != 0 {
            print!(" {}", i);
        }
    }
    println!("\n");

    // Compter les entrees en exception par l'adresse du gestionnaire : le coeur
    // ne tient pas l'IPSR, seul le passage en mode Handler est observable.
    let mut entrees = std::collections::BTreeMap::<u32, u64>::new();
    let mut en_handler = matches!(m.cpu.regs.mode, tamagotchi_paradise_rs::emulator::Mode::Handler);
    // Histogramme des adresses executees : une boucle morte s'y voit tout de
    // suite, la ou les compteurs de peripheriques ne disent rien.
    let mut chaud = std::collections::BTreeMap::<u32, u64>::new();

    let budget = (secondes * SECONDE as f64) as u64;
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
        *chaud.entry(m.cpu.regs.pc).or_default() += 1;
        // La console de debug du firmware nomme ses assertions : la capturer
        // evite de deviner pourquoi il s'arrete.
        if m.cpu.regs.pc == Machine::SORTIE_CONSOLE {
            let c = (m.cpu.regs.get_reg(0) & 0xFF) as u8;
            if c == 10 || (0x20..0x7F).contains(&c) {
                m.console.push(c as char);
            }
        }
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        let maintenant =
            matches!(m.cpu.regs.mode, tamagotchi_paradise_rs::emulator::Mode::Handler);
        if maintenant && !en_handler {
            *entrees.entry(m.cpu.regs.pc).or_default() += 1;
        }
        en_handler = maintenant;
        pas += 1;
    }
    let duree = pas as f64 / SECONDE as f64;
    println!("== {} pas, soit {:.2} secondes de temps console\n", pas, duree);

    println!("== entrees en exception, par adresse de gestionnaire");
    if entrees.is_empty() {
        println!("  aucune");
    }
    let mut liste: Vec<_> = entrees.iter().collect();
    liste.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (adr, n) in liste {
        println!("  {:#010x} {:>8} fois, {:.1} par seconde", adr, n, *n as f64 / duree);
    }
    println!();

    if !m.console.is_empty() {
        println!("== console de debug du firmware
{}
", m.console);
    }

    println!("== adresses les plus executees");
    let mut liste_chaude: Vec<_> = chaud.iter().collect();
    liste_chaude.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (adr, n) in liste_chaude.iter().take(15) {
        println!("  {:#010x} {:>10} fois, {:.1} %", adr, n, **n as f64 * 100.0 / pas as f64);
    }
    println!("  ({} adresses distinctes)\n", chaud.len());

    println!("== registres sans modele, les plus sollicites");
    for (adr, nom, s) in m.bus.mmio_trace.hottest(40) {
        println!(
            "  {:#010x} {:<10} lectures {:>9}  ecritures {:>7}  derniere {:#010x}  premier PC {:#010x}",
            adr, nom, s.reads, s.writes, s.last_write, s.first_pc
        );
    }

    println!("\n== tous les registres peripheriques, les plus sollicites");
    for (adr, nom, s) in m.bus.mmio_trace.hottest_all(40) {
        println!(
            "  {:#010x} {:<10} lectures {:>9}  ecritures {:>7}  derniere {:#010x}  premier PC {:#010x}",
            adr, nom, s.reads, s.writes, s.last_write, s.first_pc
        );
    }

    if m.bus.mmio_trace.log_page.is_some() {
        println!("\n== acces a la page observee, dans l'ordre");
        for e in m.bus.mmio_trace.log.iter().take(60) {
            println!(
                "  PC {:#010x}  {} {:#010x} = {:#010x}",
                e.pc,
                if e.is_write { "ecrit" } else { "lit  " },
                e.addr,
                e.value
            );
        }
        println!("  ({} acces au total)", m.bus.mmio_trace.log.len());
    }

    println!("\n== adresses hors carte memoire");
    for (adr, s) in m.bus.mmio_trace.off_map.iter().take(20) {
        println!(
            "  {:#010x} lectures {:>9}  ecritures {:>7}  premier PC {:#010x}",
            adr, s.reads, s.writes, s.first_pc
        );
    }
}
