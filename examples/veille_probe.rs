//! Reconstruit l'etat de veille dont la console ne ressort plus, et regarde ce
//! que devient un appui.
//!
//! Usage : cargo run --release --example veille_probe --
//!             <dump.bin> <cle hex> <etat.tamastate> [images tenues]
//!
//! L'etat bloque se decrit entierement par deux choses, relevees dans
//! l'interface : le PC gare dans la boucle de veille en `0x23D0..0x2434`, et la
//! scene 110 SYSTEM_POWERDOWN encore inscrite en memoire vive. On les repose
//! tous les deux sur un instantane de jeu ordinaire, sans avoir a attendre les
//! minutes d'inactivite qu'il faudrait pour y arriver par le jeu.
//!
//! La sonde deroule ensuite des images comme le fait l'interface, tient un
//! bouton, et rend a chaque image le compteur de cycles, le PC, la scene et
//! l'etat du controleur d'alimentation. Un reset se lit au compteur qui
//! retombe a zero.

use capybara::emulator::etat::Instantane;
use capybara::emulator::Machine;

/// Boucle de veille profonde, celle que `Machine::VEILLE_PROFONDE` decrit.
const ENTREE_VEILLE: u32 = 0x0000_23D0;
const SCENE: u32 = 0x1800_1BF4;
const ETAT_MACHINE: u32 = 0x1800_1BFA;
/// Scene d'extinction, celle que l'interface affiche quand l'ecran est noir.
const POWERDOWN: u16 = 110;

fn lire16(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn ecrire16(m: &mut Machine, adr: u32, val: u16) {
    let o = (adr - 0x1800_0000) as usize;
    if o + 1 < m.bus.sram.data.len() {
        m.bus.sram.data[o] = (val & 0xFF) as u8;
        m.bus.sram.data[o + 1] = (val >> 8) as u8;
    }
}

fn etat(m: &Machine) -> String {
    format!(
        "cycles {:>12}  PC {:#010x}  scene {:>3}  veille {}  profond {}",
        m.cpu.cycles,
        m.cpu.regs.pc,
        lire16(m, SCENE),
        if m.en_veille_profonde() { "oui" } else { "non" },
        if m.periph.pmu.deep_sleep_active { "oui" } else { "non" },
    )
}

fn main() {
    let mut a = std::env::args().skip(1);
    let dump = a.next().expect("dump.bin");
    let cle = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat_path = a.next().expect("etat.tamastate");
    let tenues: usize = a.next().and_then(|v| v.parse().ok()).unwrap_or(120);

    let mut m = Machine::new();
    m.device_key = Some(cle);
    m.load_firmware_file(&dump).unwrap();
    m.restaurer(&Instantane::lire(std::path::Path::new(&etat_path)).expect("lecture de l'etat"));
    m.remplacer_la_pile();
    m.is_running = true;

    println!("== depart {}", etat(&m));

    // La scene d'extinction est reposee en memoire vive, puis le coeur est gare
    // a l'entree de la boucle de veille. Quelques milliers de pas suffisent a
    // l'y installer, elle ne fait que tourner sur elle meme.
    ecrire16(&mut m, SCENE, POWERDOWN);
    let e = lire16(&m, ETAT_MACHINE);
    ecrire16(&mut m, ETAT_MACHINE, e & !0x07);
    m.cpu.regs.pc = ENTREE_VEILLE;
    for _ in 0..20_000u32 {
        let _ = m.step();
    }
    println!("== gare  {}", etat(&m));
    if !m.en_veille_profonde() {
        println!("   la console n'est pas garee dans la boucle, la sonde ne vaut rien");
        return;
    }

    // Le bouton est celui du milieu, celui qu'on presse pour rallumer.
    let bouton = match std::env::var("BOUTON").as_deref() {
        Ok("A") => Machine::BOUTON_A,
        Ok("C") => Machine::BOUTON_C,
        _ => Machine::BOUTON_B,
    };

    println!("\n== appui, puis {tenues} images tenues");
    let mut reveils = 0u32;
    for image in 0..6000usize {
        // L'interface tient la broche tant que le bouton est enfonce. Au dela,
        // elle la relache.
        if image < tenues {
            if m.en_veille_profonde() {
                if m.reveiller_par_broche() {
                    reveils += 1;
                    println!("  image {image:>3} : REVEIL  {}", etat(&m));
                    continue;
                }
            }
            m.appuyer(bouton);
        } else {
            m.relacher(bouton);
        }

        let _ = m.run_frame();

        if image < 6 || image % 400 == 0 || image == tenues {
            println!("  image {image:>3} :         {}", etat(&m));
        }
    }

    {
        // L'emetteur serie est le premier suspect : le firmware ecrit son
        // message de demarrage avant tout le reste, et attend qu'il parte.
        let u = &m.periph.uart;
        println!(
            "\n== uart : ctrl={:#06x} lcr={:#06x} dll={:#04x} dlm={:#04x} debit={} tx_fifo={} tx_out={}",
            u.ctrl,
            u.lcr,
            u.dll,
            u.dlm,
            u.baud_rate(capybara::emulator::peripherals::snsys::CYCLES_PAR_SECONDE as u32),
            u.tx_fifo.len(),
            u.tx_out.len()
        );
        println!(
            "   emetteur arme : {}   recepteur arme : {}",
            (u.ctrl & 0x81) == 0x81,
            (u.ctrl & 0x41) == 0x41
        );
    }

    println!("\n== {reveils} reveils declenches");
    println!("== arrivee {}", etat(&m));
    if !m.console.is_empty() {
        println!("\n== console du firmware\n{}", m.console);
    }
}
