//! Montre ce que donne le dechiffrement avec une cle connue, region par region.
//!
//! Usage : cargo run --release --example verite_probe -- <dump.bin> <cle hex>

use capybara::emulator::sonix::SonixImage;

fn main() {
    let mut a = std::env::args().skip(1);
    let chemin = a.next().expect("dump.bin");
    let cle = u32::from_str_radix(
        a.next().expect("cle hex").trim_start_matches("0x"),
        16,
    )
    .unwrap();
    let buf = std::fs::read(&chemin).expect("dump illisible");

    let brut = SonixImage::load(&buf, None).expect("pas de table");
    for (i, t) in brut.tables.iter().enumerate() {
        println!(
            "table {i} : chiffree {} mode {:?} user {:#x}+{:#x} sram {:#x}+{:#x} dpd {:#x}+{:#x}",
            t.encrypted, t.mode,
            t.user_code.addr, t.user_code.len,
            t.sram_code.addr, t.sram_code.len,
            t.dpd_code.addr, t.dpd_code.len,
        );
    }

    let clair = SonixImage::load(&buf, Some(cle)).expect("pas de table");
    for (i, t) in clair.tables.iter().enumerate() {
        for r in t.regions() {
            let off = r.flash_offset();
            if off + 16 > clair.flash.len() {
                continue;
            }
            let b = &clair.flash[off..off + 16];
            let mot = |k: usize| u32::from_le_bytes(b[k * 4..k * 4 + 4].try_into().unwrap());
            println!(
                "table {i} region {:#010x} len {:#x} : {:08x} {:08x} {:08x} {:08x}",
                r.addr, r.len, mot(0), mot(1), mot(2), mot(3)
            );
        }
    }
}
