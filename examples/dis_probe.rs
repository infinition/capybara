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

    println!("== desassemblage a {:#010x} ({} instructions)", debut, nombre);
    for inst in m.get_disassembly_at(debut, nombre) {
        println!("  {:#010x}  {:<8} {}", inst.address, inst.mnemonic, inst.operands);
    }
}
