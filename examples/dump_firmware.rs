//! Extrait le firmware dechiffre vers un dossier de sortie.
//!
//! Usage : cargo run --release --example dump_firmware -- <dump.bin> <deviceKey hex> [dossier]
//!
//! Ecrit flash_dechiffre.bin (flash 16 Mo, regions dechiffrees sur place) et
//! pram_dechiffre.bin (bootloader dechiffre, 64 Ko, execute a l'adresse 0).

use std::fs;
use std::path::PathBuf;
use tamagotchi_paradise_rs::emulator::Machine;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: dump_firmware <dump.bin> <deviceKey hex> [dossier]");
        std::process::exit(2);
    };
    let key: Option<u32> = args
        .next()
        .map(|k| u32::from_str_radix(k.trim_start_matches("0x"), 16).expect("cle hexadecimale"));
    let out_dir: PathBuf = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
            PathBuf::from(home).join("Desktop").join("tamagotchi_firmware")
        });

    let mut m = Machine::new();
    m.device_key = key;
    let report = match m.load_firmware_file(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("chargement impossible : {}", e);
            std::process::exit(1);
        }
    };

    fs::create_dir_all(&out_dir).expect("creation du dossier de sortie");

    let flash_path = out_dir.join("flash_dechiffre.bin");
    fs::write(&flash_path, &m.bus.flash.data).expect("ecriture flash");

    let pram_path = out_dir.join("pram_dechiffre.bin");
    fs::write(&pram_path, &m.bus.pram.data).expect("ecriture pram");

    println!("dechiffre : {}", report.encrypted);
    println!("demarrable: {}", report.bootable);
    println!("SP={:#010x} PC={:#010x}", report.entry_sp, report.entry_pc);
    for r in &report.regions {
        println!("  region {:<10} {:#010x} .. {:#010x} ({} octets)", r.label, r.addr, r.addr + r.len, r.len);
    }
    println!("flash  -> {}", flash_path.display());
    println!("pram   -> {}", pram_path.display());
}
