//! Sonde de desassemblage : rend un intervalle d'instructions a froid, sans
//! executer le firmware. Sert a lire un dispatcher avant de l'instrumenter.
//!
//! Usage : cargo run --release --example dis_probe --
//!             <dump.bin> <cle hex> <adresse hex> [nombre d'instructions]

use tamagotchi_paradise_rs::emulator::Machine;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let debut =
        u32::from_str_radix(a.next().expect("adresse hex").trim_start_matches("0x"), 16).unwrap();
    let nombre: usize = a.next().and_then(|v| v.parse().ok()).unwrap_or(32);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();
    // Le dump d'origine porte le drapeau de pile faible : sans PILE_USEE, on
    // remplace la pile, sinon le firmware affiche son message et s'eteint.
    if std::env::var("PILE_USEE").is_err() {
        m.remplacer_la_pile();
    }

    // Sans cela la fenetre XIP reste sur son offset par defaut et tout le code
    // au dela de 0x10000000 se lit decale de 0x11000, ce qui donne un
    // desassemblage plausible mais faux. Le firmware y installe la base issue
    // du bloc boot-info ; XIP_BASE permet de la changer si une edition differe.
    let base = std::env::var("XIP_BASE")
        .ok()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x6001_1000);
    m.periph.xip.base = base;
    m.periph.xip.ctrl = 3;

    println!("== desassemblage a {:#010x} ({} instructions)", debut, nombre);
    for inst in m.get_disassembly_at(debut, nombre) {
        println!("  {:#010x}  {:<8} {}", inst.address, inst.mnemonic, inst.operands);
    }
}
