//! Sonde de concurrence : verifie si l'etat vivant du jeu change entre deux
//! points d'un meme enchainement, et compte les exceptions qui s'y intercalent.
//!
//! Sert a distinguer un peripherique manquant d'un defaut de fidelite
//! temporelle. Le firmware calcule une somme de controle, ecrit sa sauvegarde,
//! puis recalcule la somme sur la meme zone : si elle a bouge entre-temps, c'est
//! qu'un gestionnaire d'interruption a modifie l'etat.
//!
//! Usage : cargo run --release --example race_probe -- <dump.bin> <cle hex>

use tamagotchi_paradise_rs::emulator::peripherals::crc::{ChecksumUnit, POLY_ARC_REFLECHI};
use tamagotchi_paradise_rs::emulator::{Machine, Mode, StepResult};

/// Zone ou vit l'etat du jeu, celle que le firmware sauvegarde.
const ETAT: u32 = 0x1800_0BA0;
const TAILLE: u32 = 0x0FFC;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let depart = u32::from_str_radix(
        &a.next().unwrap_or_else(|| "b8cc".into()).replace("0x", ""),
        16,
    )
    .unwrap();
    let arrivee = u32::from_str_radix(
        &a.next().unwrap_or_else(|| "b8e8".into()).replace("0x", ""),
        16,
    )
    .unwrap();

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();

    let mut pas = 0u64;
    while m.cpu.regs.pc != depart && pas < 400_000_000 {
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
    }
    if m.cpu.regs.pc != depart {
        println!("point de depart {:#010x} jamais atteint en {} pas", depart, pas);
        return;
    }

    let somme = |m: &mut Machine| {
        let mut octets = Vec::with_capacity(TAILLE as usize);
        for i in 0..TAILLE {
            octets.push(m.bus.read_u8(ETAT + i, &mut m.periph, &m.cpu.nvic));
        }
        ChecksumUnit::crc16(octets.into_iter(), POLY_ARC_REFLECHI)
    };

    let avant = somme(&mut m);
    println!("== depart {:#010x} apres {} pas", depart, pas);
    println!("  somme de l'etat vivant : {:#06x}", avant);

    let mut exceptions = 0u64;
    let mut mode_precedent = m.cpu.regs.mode;
    let mut ecarts = 0u64;
    while m.cpu.regs.pc != arrivee && pas < 400_000_000 {
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;
        if m.cpu.regs.mode == Mode::Handler && mode_precedent == Mode::Thread {
            exceptions += 1;
        }
        mode_precedent = m.cpu.regs.mode;
        ecarts += 1;
    }

    let apres = somme(&mut m);
    println!("\n== arrivee {:#010x}, {} instructions plus loin", arrivee, ecarts);
    println!("  somme de l'etat vivant : {:#06x}", apres);
    println!("  exceptions prises entre les deux : {}", exceptions);
    if avant == apres {
        println!("\n  L'etat n'a pas bouge : la divergence vient d'ailleurs.");
    } else {
        println!("\n  L'etat a change en cours de route.");
    }
}
