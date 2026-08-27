//! Sonde de demarrage : execute le vrai firmware et rend compte de ou il va.
//!
//! Usage : cargo run --example boot_probe -- <dump.bin> <deviceKey hex> [pas]
//!
//! Sert a savoir quels peripheriques le firmware attend, en relevant les
//! registres non modelises qu'il touche avant de caler.

use std::collections::BTreeMap;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult, StopReason};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: boot_probe <dump.bin> <deviceKey hex> [pas]");
        std::process::exit(2);
    };
    let key = args
        .next()
        .map(|k| u32::from_str_radix(k.trim_start_matches("0x"), 16).expect("cle hexadecimale"));
    let budget: u64 = args.next().and_then(|n| n.parse().ok()).unwrap_or(2_000_000);

    let mut m = Machine::new();
    m.device_key = key;

    let report = match m.load_firmware_file(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("chargement impossible : {}", e);
            std::process::exit(1);
        }
    };

    println!("== chargement");
    println!("  {} octets, image {:?}", report.bytes, report.kind);
    println!("  chiffre   : {}", report.encrypted);
    println!("  demarrable: {}", report.bootable);
    for r in &report.regions {
        println!("  region {:<10} {:#010x} .. {:#010x}", r.label, r.addr, r.addr + r.len);
    }
    if !report.bootable {
        println!("\nPas de vecteur de reset exploitable, arret.");
        return;
    }
    println!("  SP={:#010x}  PC={:#010x}", report.entry_sp, report.entry_pc);

    // Histogramme grossier des zones parcourues, pour voir ou le code se promene.
    let mut zones: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut pc_hist: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    let mut executed = 0u64;
    let mut stop = None;

    while executed < budget {
        let pc = m.cpu.regs.pc;
        let zone = match pc {
            0x0000_0000..=0x0000_FFFF => "PRAM",
            0x0800_0000..=0x0800_FFFF => "ROM",
            0x1000_0000..=0x100F_FFFF => "XIP (icache)",
            0x1800_0000..=0x1801_FFFF => "SRAM",
            0x6000_0000..=0x6FFF_FFFF => "XIP (direct)",
            _ => "hors carte",
        };
        *zones.entry(zone).or_default() += 1;
        *pc_hist.entry(pc).or_default() += 1;

        match m.step() {
            StepResult::Ok(_) => executed += 1,
            StepResult::Undefined(op) => {
                stop = Some(StopReason::Undefined { pc, opcode: op as u32 });
                break;
            }
            StepResult::Halt => {
                stop = Some(StopReason::Halted(pc));
                break;
            }
            StepResult::Breakpoint => {
                stop = Some(StopReason::Breakpoint(pc));
                break;
            }
        }
    }

    println!("\n== execution");
    println!("  {} instructions executees", executed);
    match &stop {
        None => println!("  toujours en vie a la fin du budget, PC={:#010x}", m.cpu.regs.pc),
        Some(StopReason::Undefined { pc, opcode }) => {
            println!("  ARRET instruction non decodee {:#06x} a PC={:#010x}", opcode, pc)
        }
        Some(StopReason::Halted(pc)) => println!("  ARRET halt a PC={:#010x}", pc),
        Some(StopReason::Breakpoint(pc)) => println!("  ARRET breakpoint a PC={:#010x}", pc),
    }

    println!("\n== zones parcourues");
    let total: u64 = zones.values().sum();
    let mut z: Vec<_> = zones.into_iter().collect();
    z.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    for (name, count) in z {
        println!("  {:<14} {:>9}  {:>5.1}%", name, count, count as f64 * 100.0 / total as f64);
    }

    let mut hot_pc: Vec<_> = pc_hist.into_iter().collect();
    hot_pc.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    hot_pc.truncate(16);
    println!("
== adresses les plus executees");
    let lo = hot_pc.iter().map(|(a, _)| *a).min().unwrap_or(0);
    for (addr, count) in &hot_pc {
        println!("  {:#010x}  {:>9} fois", addr, count);
    }
    println!("
== desassemblage autour de la boucle");
    for inst in m.get_disassembly_at(lo.saturating_sub(16), 32) {
        let here = if inst.address == m.cpu.regs.pc { "<-- PC" } else { "" };
        println!("  {:#010x}  {:<8} {:<20} {}", inst.address, inst.mnemonic, inst.operands, here);
    }

    let hot = m.bus.mmio_trace.hottest(25);
    println!("\n== registres peripheriques non modelises ({} distincts)", m.bus.mmio_trace.unknown.len());
    if hot.is_empty() {
        println!("  aucun");
    }
    for (addr, name, s) in hot {
        println!(
            "  {:#010x}  {:<9} +{:#05x}  lect {:>7}  ecr {:>6}  derniere {:#010x}",
            addr,
            name,
            addr & 0xFFF,
            s.reads,
            s.writes,
            s.last_write
        );
    }

    let all = m.bus.mmio_trace.hottest_all(20);
    println!("
== tous les acces peripheriques ({} registres distincts)", m.bus.mmio_trace.all.len());
    if all.is_empty() {
        println!("  aucun");
    }
    for (addr, name, s) in all {
        println!(
            "  {:#010x}  {:<9} +{:#05x}  lect {:>9}  ecr {:>7}  derniere {:#010x}",
            addr, name, addr & 0xFFF, s.reads, s.writes, s.last_write
        );
    }

    println!("
== acces hors carte memoire ({} adresses)", m.bus.mmio_trace.off_map.len());
    for (addr, s) in m.bus.mmio_trace.off_map.iter().take(20) {
        println!("  {:#010x}  lect {:>9}  ecr {:>7}  derniere {:#010x}", addr, s.reads, s.writes, s.last_write);
    }

    if !m.periph.uart.console_history.is_empty() {
        println!("\n== UART");
        println!("{}", m.periph.uart.console_history);
    }
}
