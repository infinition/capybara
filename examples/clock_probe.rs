//! Sonde de la sequence d'horloge : releve le PC de chaque acces a la zone
//! systeme SN_SYS0 (0x45000000..0x45001000) dans l'ordre d'execution.
//!
//! Usage : cargo run --release --example clock_probe -- <dump.bin> <deviceKey hex>

use std::collections::BTreeMap;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: clock_probe <dump.bin> <deviceKey hex>");
        std::process::exit(2);
    };
    let key = args
        .next()
        .map(|k| u32::from_str_radix(k.trim_start_matches("0x"), 16).expect("cle hexadecimale"));

    let mut m = Machine::new();
    m.device_key = key;
    if let Err(e) = m.load_firmware_file(&path) {
        eprintln!("chargement impossible : {}", e);
        std::process::exit(1);
    }

    // On ne veut que la zone systeme, sans les fusibles FEUSE (0x30..=0x3f).
    let mut prev: BTreeMap<u32, (u64, u64)> = BTreeMap::new();
    let mut log: Vec<(u32, u32, bool, u32, [u32; 4])> = Vec::new(); // pc, addr, is_write, val, regs

    for _ in 0..200_000 {
        let pc = m.cpu.regs.pc;
        match m.step() {
            StepResult::Ok(_) => {}
            StepResult::Undefined(op) => {
                println!("ARRET instruction non decodee {:#06x} a PC={:#010x}", op, pc);
                break;
            }
            StepResult::Halt => {
                println!("ARRET halt a PC={:#010x}", pc);
                break;
            }
            StepResult::Breakpoint => {
                println!("ARRET breakpoint a PC={:#010x}", pc);
                break;
            }
        }

        // Comparer la trace MMIO avant/après pour détecter les nouveaux accès.
        let mut news: Vec<(u32, u64, u64)> = Vec::new();
        for (&a, s) in &m.bus.mmio_trace.all {
            if (0x4500_0000..0x4500_1000).contains(&a) {
                let cur = (s.reads, s.writes);
                match prev.get(&a) {
                    Some(&p) if p != cur => news.push((a, cur.0 - p.0, cur.1 - p.1)),
                    None if cur != (0, 0) => news.push((a, cur.0, cur.1)),
                    _ => {}
                }
            }
        }
        for (a, _dr, dw) in news {
            let is_write = dw > 0;
            let val = if is_write {
                m.bus.mmio_trace.all.get(&a).map(|s| s.last_write).unwrap_or(0)
            } else {
                0
            };
            let regs = [
                m.cpu.regs.get_reg(0),
                m.cpu.regs.get_reg(1),
                m.cpu.regs.get_reg(2),
                m.cpu.regs.get_reg(3),
            ];
            log.push((pc, a, is_write, val, regs));
        }
        prev = m
            .bus
            .mmio_trace
            .all
            .iter()
            .filter(|(&a, _)| (0x4500_0000..0x4500_1000).contains(&a))
            .map(|(&a, s)| (a, (s.reads, s.writes)))
            .collect();

        if log.len() >= 64 {
            break;
        }
    }

    println!("== sequence d'acces SN_SYS0 (ordre chronologique)");
    for (pc, addr, is_write, val, regs) in &log {
        let kind = if *is_write { "ECR" } else { "LECT" };
        println!(
            "  PC={:#010x}  {:<4} {:#010x}  val={:#010x}  r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x}",
            pc, kind, addr, val, regs[0], regs[1], regs[2], regs[3]
        );
    }

    // Desassemblage autour des premiers acces, pour voir la logique du code.
    println!("\n== boucle de poll (autour de 0x8a60)");
    for inst in m.get_disassembly_at(0x8a60, 16) {
        println!("  {:#010x}  {:<8} {:<20}", inst.address, inst.mnemonic, inst.operands);
    }

    println!("\n== second poll (autour de 0x8aa0)");
    for inst in m.get_disassembly_at(0x8aa0, 24) {
        println!("  {:#010x}  {:<8} {:<20}", inst.address, inst.mnemonic, inst.operands);
    }

    println!("\n== contexte autour des premiers acces");
    let mut seen = std::collections::HashSet::new();
    for (pc, _, _, _, _) in &log {
        if seen.insert(*pc) {
            println!("--- PC={:#010x}", pc);
            for inst in m.get_disassembly_at(pc.saturating_sub(8), 10) {
                let here = if inst.address == *pc { "  <--" } else { "" };
                println!(
                    "  {:#010x}  {:<8} {:<20}{}",
                    inst.address, inst.mnemonic, inst.operands, here
                );
            }
        }
        if seen.len() >= 8 {
            break;
        }
    }
}
