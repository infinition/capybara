//! Sonde de son : attend que le firmware joue une note, puis releve ce qu'il
//! ecrit sur ses peripheriques.
//!
//! Usage : cargo run --release --example son_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [secondes apres]
//!
//! Le moteur audio du firmware ne tourne que pendant qu'un son joue : hors de
//! ces instants, rien ne sort. On rejoue donc des appuis jusqu'a ce que
//! `jouer_son`, en `0x1001FCB4`, soit atteint, on vide la trace, et on regarde
//! ce que la console touche a partir de la. Le buzzer se signale ainsi tout
//! seul, sans avoir a deviner sa page.

use tamagotchi_paradise_rs::emulator::etat::Instantane;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

const SECONDE: f64 = 96_000_000.0;
/// Entree de `jouer_son(identifiant, volume)`.
const JOUER_SON: u32 = 0x1001_FCB4;
/// Drapeau pose tant qu'un son est en cours.
const SON_EN_COURS: u32 = 0x1801_4284;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let apres: f64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(0.5);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));

    // Appuis en boucle : le son n'arrive que sur des evenements de jeu, il faut
    // donc vraiment jouer pour en declencher un.
    let touches = [
        Machine::BOUTON_A,
        Machine::BOUTON_B,
        Machine::BOUTON_MOLETTE,
        Machine::BOUTON_C,
    ];
    let periode = (2.5 * SECONDE) as u64;
    let duree = (0.3 * SECONDE) as u64;

    let budget = (600.0 * SECONDE) as u64;
    let mut pas = 0u64;
    let mut tenue: Option<(u64, u32)> = None;
    let mut trouve = None;

    while pas < budget {
        if pas % periode == 0 {
            let broche = touches[(pas / periode) as usize % touches.len()];
            m.appuyer(broche);
            tenue = Some((pas + duree, broche));
        }
        if let Some((fin, broche)) = tenue {
            if pas >= fin {
                m.relacher(broche);
                tenue = None;
            }
        }
        if m.cpu.regs.pc == JOUER_SON {
            trouve = Some((pas, m.cpu.regs.get_reg(0), m.cpu.regs.get_reg(1)));
            break;
        }
        if !matches!(m.step(), StepResult::Ok(_)) {
            println!("arret a PC={:#010x}", m.cpu.regs.pc);
            break;
        }
        pas += 1;
    }

    let Some((quand, identifiant, volume)) = trouve else {
        println!("== aucun son joue en {:.0} secondes de temps console", pas as f64 / SECONDE);
        return;
    };
    println!(
        "== son {} joue a {:.1} s, volume {}",
        identifiant,
        quand as f64 / SECONDE,
        volume
    );

    // A partir d'ici, tout ce que touche la console appartient au son.
    m.bus.mmio_trace.clear();
    m.bus.mmio_trace.enabled = true;
    // Tout ce que le module audio dit au materiel, quelle que soit la page.
    // MODULE_AUDIO permet d'elargir a un autre intervalle a l'essai.
    m.bus.mmio_trace.log_pc = Some(
        std::env::var("MODULE_AUDIO")
            .ok()
            .and_then(|v| {
                let (a, b) = v.split_once('-')?;
                Some((
                    u32::from_str_radix(a.trim().trim_start_matches("0x"), 16).ok()?,
                    u32::from_str_radix(b.trim().trim_start_matches("0x"), 16).ok()?,
                ))
            })
            .unwrap_or((0x1001_F000, 0x1008_0000)),
    );
    // Releve pendant que le son joue, et non apres : le tableau de voix garde
    // ses valeurs au silence, seul le drapeau dit ce qui sonne vraiment.
    println!("\n== voix pendant le son");
    for &instant in &[1u64, 5, 15, 40, 80, 160, 300] {
        let cible = quand + (instant as f64 * 0.001 * SECONDE) as u64;
        while pas < cible && matches!(m.step(), StepResult::Ok(_)) {
            pas += 1;
        }
        println!(
            "  a {:>4} ms : drapeau {}  voix {:?}",
            instant,
            m.bus.sram.read_u8((SON_EN_COURS - 0x1800_0000) as usize),
            m.voix_audio()
        );
    }

    let sortie = m.periph.port1.entrees;
    let mut bascules_port1 = 0u64;
    let mut moteur_actif = 0u64;

    let fin = pas + (apres * SECONDE) as u64;
    let mut precedent = m.periph.port1.entrees;
    while pas < fin {
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
        if m.periph.port1.entrees != precedent {
            bascules_port1 += 1;
            precedent = m.periph.port1.entrees;
        }
        if m.bus.sram.read_u8((SON_EN_COURS - 0x1800_0000) as usize) != 0 {
            moteur_actif += 1;
        }
    }
    println!(
        "== {:.2} s de plus, moteur audio actif sur {:.1} % des pas",
        apres,
        moteur_actif as f64 * 100.0 / (fin - quand).max(1) as f64
    );
    println!("== bascules du port 1 : {} (etat de depart {:#010x})", bascules_port1, sortie);

    // Le tableau garde ses valeurs une fois le son fini : ce releve montre
    // justement qu'il ne suffit pas, et pourquoi le drapeau decide seul.
    println!("\n== structure de voix apres le son, valeurs residuelles");
    for &instant in &[0u64, 5, 15, 40, 80, 160] {
        let cible = quand + (instant as f64 * 0.001 * SECONDE) as u64;
        while pas < cible && matches!(m.step(), StepResult::Ok(_)) {
            pas += 1;
        }
        // Huit voix de 0x34 octets, a partir de 0x1801C820 : l'allocation en
        // 0x10022BE2 indexe par le type de son.
        let o = (0x1801_C820u32 - 0x1800_0000) as usize;
        let mut lignes = Vec::new();
        for voix in 0..8usize {
            let d = &m.bus.sram.data[o + voix * 0x34..o + (voix + 1) * 0x34];
            if d.iter().any(|&b| b != 0) {
                lignes.push(format!(
                    "voix {} : {}",
                    voix,
                    d.iter().take(24).map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ")
                ));
            }
        }
        if lignes.is_empty() {
            println!("  a {:>4} ms : aucune voix active", instant);
        } else {
            for l in lignes {
                println!("  a {:>4} ms : {}", instant, l);
            }
        }
        println!("            voix rendues a l'interface : {:?}", m.voix_audio());
        let e = (0x1800_ECD0u32 - 0x1800_0000) as usize;
        println!(
            "            melodie {} periode {} index {}",
            u32::from_le_bytes([
                m.bus.sram.data[e],
                m.bus.sram.data[e + 1],
                m.bus.sram.data[e + 2],
                m.bus.sram.data[e + 3]
            ]),
            u16::from_le_bytes([m.bus.sram.data[e + 10], m.bus.sram.data[e + 11]]),
            u16::from_le_bytes([m.bus.sram.data[e + 4], m.bus.sram.data[e + 5]])
        );
    }

    println!("\n== acces materiels venus du module audio");
    let mut vus = std::collections::BTreeMap::<(u32, u32), u64>::new();
    for e in &m.bus.mmio_trace.log {
        *vus.entry((e.pc, e.addr)).or_default() += 1;
    }
    if vus.is_empty() {
        println!("  aucun : le moteur audio ne parle a aucun peripherique");
    }
    for ((pc, adr), n) in vus.iter().take(30) {
        println!("  PC {:#010x}  ->  {:#010x}   {} fois", pc, adr, n);
    }

    println!("\n== registres sans modele, pendant le son");
    for (adr, nom, s) in m.bus.mmio_trace.hottest(25) {
        println!(
            "  {:#010x} {:<10} lectures {:>8}  ecritures {:>7}  derniere {:#010x}  premier PC {:#010x}",
            adr, nom, s.reads, s.writes, s.last_write, s.first_pc
        );
    }
    println!("\n== tous les registres peripheriques, pendant le son");
    for (adr, nom, s) in m.bus.mmio_trace.hottest_all(25) {
        println!(
            "  {:#010x} {:<10} lectures {:>8}  ecritures {:>7}  derniere {:#010x}  premier PC {:#010x}",
            adr, nom, s.reads, s.writes, s.last_write, s.first_pc
        );
    }
}
