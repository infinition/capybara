//! Sonde d'ecran : execute jusqu'au depart du transfert vers l'afficheur, puis
//! rend le tampon d'image en PPM.
//!
//! Usage : cargo run --release --example ecran_probe --
//!             <dump.bin> <cle hex> [sortie.ppm] [largeur] [budget]
//!
//! La source et le nombre d'unites sont lus dans le canal, pas supposes : le
//! pilote les programme en 0x000044B8 avant de poser son bit de depart.

use std::io::Write;
use tamagotchi_paradise_rs::emulator::peripherals::dma;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

/// Instruction qui pose le bit de depart du canal, en PRAM.
const DEPART_CANAL: u32 = 0x0000_4576;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let sortie = a.next().unwrap_or_else(|| "ecran.ppm".into());
    let largeur: u32 = a.next().and_then(|v| v.parse().ok()).unwrap_or(128);
    let budget: u64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(400_000_000);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    // Le dump d'origine porte le drapeau de pile faible : sans PILE_USEE, on
    // remplace la pile, sinon le firmware affiche son message et s'eteint.
    if std::env::var("PILE_USEE").is_err() {
        m.remplacer_la_pile();
    }

    // ECRAN_DEPART=n s'arrete au nieme transfert. Sans cela on va jusqu'au bout
    // du budget : le tampon contient alors le dernier rendu, alors que le
    // premier transfert n'affiche souvent qu'un effacement.
    let arret: Option<u64> =
        std::env::var("ECRAN_DEPART").ok().and_then(|v| v.parse().ok());
    let mut pas = 0u64;
    let mut departs = 0u64;
    while pas < budget {
        if m.cpu.regs.pc == DEPART_CANAL {
            departs += 1;
            if Some(departs) == arret {
                break;
            }
        }
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
    }
    if departs == 0 {
        println!("aucun transfert vers l'afficheur en {} pas", pas);
        return;
    }
    println!("== {} transferts vers l'afficheur", departs);

    let canal = &m.periph.dma.canaux[0];
    let source = canal.source;
    let unites = canal.compte & dma::MASQUE_COMPTE;
    println!("== transfert vers l'afficheur, apres {} pas", pas);
    println!("  source      {:#010x}", source);
    println!("  destination {:#010x}", canal.destination);
    println!("  unites      {} ({:#x})", unites, unites);
    println!("  controle    {:#010x}  configuration {:#010x}", canal.ctrl, canal.config);

    let hauteur = unites / largeur;
    if hauteur == 0 {
        println!("largeur {} incompatible avec {} unites", largeur, unites);
        return;
    }
    println!("  rendu en {} x {}, RGB565", largeur, hauteur);

    let nvic = m.cpu.nvic.clone();
    let mut donnees = Vec::with_capacity((unites * 3) as usize);
    for i in 0..unites {
        let a = source + 2 * i;
        let bas = m.bus.read_u8(a, &mut m.periph, &nvic) as u16;
        let haut = m.bus.read_u8(a + 1, &mut m.periph, &nvic) as u16;
        let px = bas | (haut << 8);
        // RGB565 etendu sur huit bits par composante.
        let r = ((px >> 11) & 0x1F) as u8;
        let v = ((px >> 5) & 0x3F) as u8;
        let b = (px & 0x1F) as u8;
        donnees.push((r << 3) | (r >> 2));
        donnees.push((v << 2) | (v >> 4));
        donnees.push((b << 3) | (b >> 2));
    }

    let mut f = std::fs::File::create(&sortie).expect("creation du fichier");
    write!(f, "P6\n{} {}\n255\n", largeur, hauteur).unwrap();
    f.write_all(&donnees).unwrap();
    println!("  ecrit dans {}", sortie);

    // Un tampon entierement uniforme trahit un rendu qui n'a pas eu lieu.
    let distinctes: std::collections::HashSet<&[u8]> = donnees.chunks(3).collect();
    println!("  {} couleurs distinctes", distinctes.len());
}
