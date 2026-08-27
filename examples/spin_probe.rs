//! Sonde de blocage : detecte la boucle serree ou le firmware se fige, puis
//! rend compte de son contexte exact.
//!
//! Usage : cargo run --release --example spin_probe -- <dump.bin> <cle hex>

use std::collections::HashMap;
use tamagotchi_paradise_rs::emulator::{Machine, StepResult};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("dump.bin");
    let key = u32::from_str_radix(
        args.next().expect("cle hex").trim_start_matches("0x"),
        16,
    )
    .expect("cle hexadecimale");

    let args: Vec<String> = args.collect();

    let mut m = Machine::new();
    m.device_key = Some(key);
    let report = m.load_firmware_file(&path).unwrap();
    // Le dump d'origine porte le drapeau de pile faible : sans PILE_USEE, on
    // remplace la pile, sinon le firmware affiche son message et s'eteint.
    if std::env::var("PILE_USEE").is_err() {
        m.remplacer_la_pile();
    }
    println!("charge: {} octets, demarrable={}", report.bytes, report.bootable);
    println!("SP={:#010x} PC={:#010x}", report.entry_sp, report.entry_pc);

    // `mb=<hex>` installe un pointeur de bloc boot-info en mailbox + 0xF60,
    // la ou le firmware va le chercher. Pointer vers une adresse hors carte
    // fait tomber chaque dereferencement dans la trace off_map, ce qui revele
    // les offsets reellement lus dans la structure.
    for a in &args {
        if let Some(v) = a.strip_prefix("mb=") {
            let ptr = u32::from_str_radix(v.trim_start_matches("0x"), 16).expect("mb=<hex>");
            m.bus.sram.mailbox[0xF60..0xF64].copy_from_slice(&ptr.to_le_bytes());
            println!("boot-info: [0x20000F60] = {:#010x}", ptr);
        }
        if let Some(v) = a.strip_prefix("poke=") {
            // poke=<adresse>:<valeur>, ecrit un mot dans la mailbox.
            let (addr, val) = v.split_once(':').expect("poke=<adr>:<val>");
            let addr = u32::from_str_radix(addr.trim_start_matches("0x"), 16).unwrap();
            let val = u32::from_str_radix(val.trim_start_matches("0x"), 16).unwrap();
            let off = (addr - 0x2000_0000) as usize;
            m.bus.sram.mailbox[off..off + 4].copy_from_slice(&val.to_le_bytes());
            println!("poke: [{:#010x}] = {:#010x}", addr, val);
        }
    }
    println!();

    // Phase 1 : avancer jusqu'a ce qu'une adresse soit vue 5000 fois.
    // `spin=<n>` regle le nombre de repetitions au-dela duquel on declare une
    // boucle morte. Une recopie de BSS repasse legitimement des milliers de fois.
    let seuil: u64 = args
        .iter()
        .find_map(|a| a.strip_prefix("spin=").and_then(|v| v.parse().ok()))
        .unwrap_or(5000);
    let budget: u64 = args
        .iter()
        .find_map(|a| a.strip_prefix("budget=").and_then(|v| v.parse().ok()))
        .unwrap_or(3_000_000);

    let mut seen: HashMap<u32, u64> = HashMap::new();
    let mut trail: Vec<u32> = Vec::new();
    let mut spin_pc = None;
    let mut executed = 0u64;

    // `stop=<hex>` fige l'execution a une adresse precise pour inspecter l'etat.
    let stop_at: Option<u32> = args.iter().find_map(|a| {
        a.strip_prefix("stop=")
            .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
    });
    let trap: Option<(u32, u32)> = args.iter().find_map(|a| {
        let v = a.strip_prefix("trap=")?;
        let (lo, hi) = v.split_once(':')?;
        Some((
            u32::from_str_radix(lo.trim_start_matches("0x"), 16).ok()?,
            u32::from_str_radix(hi.trim_start_matches("0x"), 16).ok()?,
        ))
    });

    while executed < budget {
        let pc = m.cpu.regs.pc;
        if stop_at == Some(pc) {
            println!("arret demande a {:#010x} apres {} pas", pc, executed);
            break;
        }
        // `trap=<lo>:<hi>` fige des que le PC entre dans une plage, pour attraper
        // l'instant ou l'execution part dans les donnees.
        if let Some((lo, hi)) = trap {
            if pc >= lo && pc <= hi {
                println!("piege: PC entre dans {:#010x}..{:#010x} apres {} pas", lo, hi, executed);
                break;
            }
        }
        // Trail compresse : on n'enregistre pas les repetitions consecutives,
        // sinon une boucle serree efface tout l'historique utile.
        if trail.last() != Some(&pc) {
            trail.push(pc);
            if trail.len() > 400 {
                trail.remove(0);
            }
        }
        let c = seen.entry(pc).or_default();
        *c += 1;
        if *c >= seuil {
            spin_pc = Some(pc);
            break;
        }
        match m.step() {
            StepResult::Ok(_) => executed += 1,
            StepResult::Undefined(op) => {
                println!("ARRET instruction non decodee {:#06x} a {:#010x} apres {} pas", op, pc, executed);
                break;
            }
            StepResult::Halt => {
                println!("ARRET halt a {:#010x} apres {} pas", pc, executed);
                break;
            }
            StepResult::Breakpoint => {
                println!("ARRET breakpoint a {:#010x}", pc);
                break;
            }
        }
    }

    println!("== {} instructions executees", executed);
    match spin_pc {
        None => println!("pas de boucle detectee, PC={:#010x}", m.cpu.regs.pc),
        Some(pc) => println!("boucle detectee autour de {:#010x}", pc),
    }

    // Phase 2 : chemin d'arrivee, avec l'instruction correspondante.
    println!("\n== 60 derniers pas avant le blocage");
    let debut = trail.len().saturating_sub(60);
    for &p in &trail[debut..] {
        let d = m.get_disassembly_at(p, 1);
        println!("  {:#010x}  {:<8} {}", p, d[0].mnemonic, d[0].operands);
    }

    // Phase 2b : desassemblage des plages demandees en ligne de commande.
    for plage in &args {
        let mut it = plage.split('-');
        let (Some(a), Some(b)) = (it.next(), it.next()) else { continue };
        let a = u32::from_str_radix(a.trim_start_matches("0x"), 16).unwrap_or(0);
        let b = u32::from_str_radix(b.trim_start_matches("0x"), 16).unwrap_or(0);
        if b <= a {
            continue;
        }
        println!("\n== desassemblage {:#010x}..{:#010x}", a, b);
        let mut cur = a;
        while cur < b {
            let d = m.get_disassembly_at(cur, 1);
            let i = &d[0];
            println!("  {:#010x}  {:<8} {}", cur, i.mnemonic, i.operands);
            cur += if i.is_32bit { 4 } else { 2 };
        }
    }

    // Phase 3 : registres.
    println!("\n== registres au blocage");
    for i in 0..13 {
        print!("  r{:<2}={:#010x}", i, m.cpu.regs.get_reg(i));
        if i % 4 == 3 {
            println!();
        }
    }
    println!();
    println!(
        "  SP ={:#010x}  LR ={:#010x}  PC ={:#010x}  xPSR={:#010x}",
        m.cpu.regs.get_sp(),
        m.cpu.regs.lr,
        m.cpu.regs.pc,
        m.cpu.regs.xpsr
    );

    // Phase 4 : une iteration de boucle en pas a pas.
    println!("\n== 24 pas a partir du blocage");
    for _ in 0..24 {
        let pc = m.cpu.regs.pc;
        let d = m.get_disassembly_at(pc, 1);
        let i = &d[0];
        println!(
            "  {:#010x}  {:<8} {:<22} r0={:#010x} r1={:#010x} r2={:#010x} r3={:#010x}",
            pc,
            i.mnemonic,
            i.operands,
            m.cpu.regs.get_reg(0),
            m.cpu.regs.get_reg(1),
            m.cpu.regs.get_reg(2),
            m.cpu.regs.get_reg(3)
        );
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
    }

    // Phase 4b : vidages memoire demandes, sous la forme dump=<adr>:<octets>.
    for a in &args {
        let Some(v) = a.strip_prefix("dump=") else { continue };
        let (adr, len) = v.split_once(':').expect("dump=<adr>:<len>");
        let adr = u32::from_str_radix(adr.trim_start_matches("0x"), 16).unwrap();
        let len = usize::from_str_radix(len.trim_start_matches("0x"), 16).unwrap();
        println!("\n== memoire {:#010x}, {} octets", adr, len);
        for row in 0..len.div_ceil(16) {
            let base = adr + (row * 16) as u32;
            print!("  {:#010x} ", base);
            for i in 0..16 {
                let b = m.bus.read_u8(base + i, &mut m.periph, &m.cpu.nvic);
                print!("{:02x} ", b);
            }
            println!();
        }
    }

    // Phase 5 : etat de la mailbox.
    println!("\n== mailbox 0x20000000, octets non nuls");
    let mb = &m.bus.sram.mailbox;
    let mut nonzero = 0;
    for (i, chunk) in mb.chunks(16).enumerate() {
        if chunk.iter().any(|&b| b != 0) {
            nonzero += 1;
            print!("  {:#010x} ", 0x2000_0000u32 + (i * 16) as u32);
            for b in chunk {
                print!("{:02x} ", b);
            }
            println!();
        }
    }
    if nonzero == 0 {
        println!("  entierement a zero, personne ne l'a ecrite");
    }

    // Phase 6 : SRAM, premiers blocs non nuls, pour voir si le firmware a
    // initialise quoi que ce soit.
    println!("\n== SRAM AHB, nombre d'octets non nuls");
    let nz = m.bus.sram.data.iter().filter(|&&b| b != 0).count();
    println!("  {} / {} octets", nz, m.bus.sram.data.len());

    // Phase 7 : peripheriques non modelises les plus sollicites.
    println!("\n== registres peripheriques non modelises");
    for (addr, name, s) in m.bus.mmio_trace.hottest(20) {
        println!(
            "  {:#010x}  {:<9} +{:#05x}  lect {:>8}  ecr {:>7}  derniere {:#010x}",
            addr,
            name,
            addr & 0xFFF,
            s.reads,
            s.writes,
            s.last_write
        );
    }

    println!("\n== acces hors carte memoire");
    if m.bus.mmio_trace.off_map.is_empty() {
        println!("  aucun");
    }
    for (addr, s) in m.bus.mmio_trace.off_map.iter().take(20) {
        println!(
            "  {:#010x}  lect {:>8}  ecr {:>7}  derniere {:#010x}",
            addr, s.reads, s.writes, s.last_write
        );
    }

    if !m.periph.uart.console_history.is_empty() {
        println!("\n== UART\n{}", m.periph.uart.console_history);
    }
}
