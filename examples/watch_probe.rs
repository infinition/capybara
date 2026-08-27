//! Sonde d'observation : execute jusqu'a la Nieme visite d'une adresse, puis
//! rend compte des registres et de la memoire autour des pointeurs utiles.
//!
//! Usage : cargo run --release --example watch_probe --
//!             <dump.bin> <cle hex> <adresse hex> [visite] [budget]

use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("dump.bin");
    let key = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let watch =
        u32::from_str_radix(a.next().expect("adresse hex").trim_start_matches("0x"), 16).unwrap();
    let nth: u64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    let budget: u64 = a.next().and_then(|v| v.parse().ok()).unwrap_or(400_000_000);

    let mut m = Machine::new();
    m.device_key = Some(key);
    m.load_firmware_file(&path).unwrap();

    let mut seen = 0u64;
    let mut steps = 0u64;
    let mut reached = false;
    while steps < budget {
        if m.cpu.regs.pc == watch {
            seen += 1;
            if seen == nth {
                reached = true;
                break;
            }
        }
        match m.step() {
            StepResult::Ok(_) => steps += 1,
            StepResult::Undefined(op) => {
                println!("arret : encodage inconnu {:#06x} a PC={:#010x}", op, m.cpu.regs.pc);
                break;
            }
            _ => {
                println!("arret a PC={:#010x}", m.cpu.regs.pc);
                break;
            }
        }
    }

    if !reached {
        println!("adresse {:#010x} vue {} fois en {} pas", watch, seen, steps);
        return;
    }

    println!("== visite {} de {:#010x}, apres {} pas", nth, watch, steps);
    for i in 0..13 {
        print!("  r{:<2}={:#010x}", i, m.cpu.regs.get_reg(i as u8));
        if i % 4 == 3 {
            println!();
        }
    }
    println!(
        "\n  SP ={:#010x}  LR ={:#010x}  PC ={:#010x}",
        m.cpu.regs.get_sp(),
        m.cpu.regs.lr,
        m.cpu.regs.pc
    );

    // Suite de l'execution : a chaque nouveau passage sur l'adresse observee on
    // releve r0, ce qui donne directement le flux de caracteres d'un printf.
    println!("
== r0 aux 80 passages suivants");
    let mut flux = String::new();
    let mut vus = 0;
    while vus < 80 && steps < budget {
        m.step();
        steps += 1;
        if m.cpu.regs.pc == watch {
            let c = m.cpu.regs.get_reg(0);
            flux.push_str(&format!("{:02x} ", c & 0xFF));
            vus += 1;
        }
    }
    println!("  |{}|", flux);

    // Les registres tenant un pointeur plausible sont deroules, avec les octets
    // vises rendus en texte quand ils sont imprimables.
    println!("\n== memoire derriere les pointeurs");
    for i in 0..13u8 {
        let v = m.cpu.regs.get_reg(i);
        if !plausible(v) {
            continue;
        }
        let mut bytes = Vec::new();
        for k in 0..40u32 {
            bytes.push(m.bus.read_u8(v.wrapping_add(k), &mut m.periph, &m.cpu.nvic));
        }
        println!("  r{:<2} {:#010x} -> {}  |{}|", i, v, hex(&bytes), texte(&bytes));
    }
}

fn plausible(v: u32) -> bool {
    matches!(v,
        0x0000_0100..=0x0000_FFFF
        | 0x1000_0000..=0x100F_FFFF
        | 0x1800_0000..=0x1801_FFFF
        | 0x6000_0000..=0x60FF_FFFF)
}

fn hex(b: &[u8]) -> String {
    b.iter().take(24).map(|x| format!("{:02x}", x)).collect::<Vec<_>>().join(" ")
}

fn texte(b: &[u8]) -> String {
    b.iter()
        .map(|&c| if (0x20..0x7f).contains(&c) { c as char } else { '.' })
        .collect()
}
