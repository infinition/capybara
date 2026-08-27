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
    // `page=<hex>` journalise dans l'ordre tous les acces a une page de 4 Ko,
    // ce que les compteurs ne permettent pas de reconstituer.
    let page = std::env::var("MMIO_PAGE")
        .ok()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());

    let report_page = page;
    let report = match m.load_firmware_file(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("chargement impossible : {}", e);
            std::process::exit(1);
        }
    };

    m.bus.mmio_trace.log_page = report_page;
    // MMIO_FORCE="adresse:valeur,adresse:valeur" impose des lectures sur des
    // registres non modelises, pour eprouver une hypothese sans coder un
    // peripherique entier.
    m.bus.mmio_trace.log_ecritures_seules = std::env::var("MMIO_ECR").is_ok();
    if let Ok(v) = std::env::var("MMIO_FORCE") {
        for paire in v.split(',') {
            if let Some((a, val)) = paire.split_once(':') {
                let a = u32::from_str_radix(a.trim().trim_start_matches("0x"), 16);
                let val = u32::from_str_radix(val.trim().trim_start_matches("0x"), 16);
                if let (Ok(a), Ok(val)) = (a, val) {
                    m.bus.mmio_trace.forcees.insert(a, val);
                    println!("force: [{:#010x}] = {:#010x}", a, val);
                }
            }
        }
    }
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
    // Le printf de debug reutilise le meme tampon, donc seul le dernier message
    // y subsiste. On l'echantillonne en cours de route pour reconstituer le
    // journal complet du firmware.
    let tampon = std::env::var("CONSOLE_ADDR")
        .ok()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x1801_C720);
    let mut journal: Vec<String> = Vec::new();
    let mut dernier = String::new();
    // Dans la boucle de formatage, cette instruction appelle la fonction de
    // sortie avec le caractere dans r0. L'intercepter donne la console complete,
    // quel que soit le tampon de destination.
    let sortie_pc = std::env::var("PRINTF_PC")
        .ok()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x0000_1070);
    let mut console = String::new();

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

        if m.cpu.regs.pc == sortie_pc && console.len() < 8000 {
            let c = (m.cpu.regs.get_reg(0) & 0xFF) as u8;
            console.push(if (0x20..0x7f).contains(&c) || c == 10 { c as char } else { ' ' });
        }

        if executed % 20_000 == 0 {
            let mut texte = String::new();
            for k in 0..160u32 {
                let b = m.bus.read_u8(tampon + k, &mut m.periph, &m.cpu.nvic);
                if b == 0 {
                    break;
                }
                texte.push(if (0x20..0x7f).contains(&b) { b as char } else { ' ' });
            }
            let texte = texte.trim().to_string();
            if !texte.is_empty() && texte != dernier {
                journal.push(texte.clone());
                dernier = texte;
            }
        }

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
    let top = hot_pc.first().map(|(a, _)| *a).unwrap_or(0);
    for (addr, count) in &hot_pc {
        println!("  {:#010x}  {:>9} fois", addr, count);
    }
    println!("
== desassemblage autour de la boucle");
    for inst in m.get_disassembly_at(top.saturating_sub(16), 32) {
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

    let all = m.bus.mmio_trace.hottest_all(60);
    println!("
== tous les acces peripheriques ({} registres distincts)", m.bus.mmio_trace.all.len());
    if all.is_empty() {
        println!("  aucun");
    }
    for (addr, name, s) in all {
        println!(
            "  {:#010x}  {:<9} +{:#05x}  lect {:>9}  ecr {:>7}  derniere {:#010x}  depuis {:#010x}",
            addr, name, addr & 0xFFF, s.reads, s.writes, s.last_write, s.first_pc
        );
    }

    println!("
== acces hors carte memoire ({} adresses)", m.bus.mmio_trace.off_map.len());
    for (addr, s) in m.bus.mmio_trace.off_map.iter().take(20) {
        println!("  {:#010x}  lect {:>9}  ecr {:>7}  derniere {:#010x}", addr, s.reads, s.writes, s.last_write);
    }

    if let Some(pg) = page {
        println!("
== journal des acces a la page {:#010x}", pg);
        for e in m.bus.mmio_trace.log.iter().rev().take(60).collect::<Vec<_>>().into_iter().rev() {
            let sens = if e.is_write { "ecrit" } else { "lit  " };
            println!(
                "  {:#010x}  {} {:#010x}  {:#010x}",
                e.pc, sens, e.addr, e.value
            );
        }
    }

    // Le printf de debug formate dans un tampon en SRAM avant tout transfert.
    // Relever les suites imprimables suffit donc a lire ce que le firmware dit,
    // sans avoir a modeliser le DMA de l'UART.
    let n = &m.cpu.nvic;
    println!("
== etat des interruptions");
    println!("  SysTick CSR={:#010x} RVR={:#010x} CVR={:#010x}  actif={} irq={}",
        n.syst_csr, n.syst_rvr, n.syst_cvr, n.syst_csr & 1 != 0, n.syst_csr & 2 != 0);
    println!("  VTOR={:#010x}  PRIMASK={}", n.vtor, m.cpu.regs.primask);
    for i in 0..4 {
        if n.iser[i] != 0 || n.ispr[i] != 0 {
            println!("  IRQ {:>3}..{:<3} activees={:#010x} en attente={:#010x}",
                i * 32, i * 32 + 31, n.iser[i], n.ispr[i]);
        }
    }

    println!("
== transferts du controleur de flash ({})", m.periph.flashctl.transferts.len());
    for (i, (f, mem, len, vers_mem)) in m.periph.flashctl.transferts.iter().enumerate().take(12) {
        println!(
            "  {:>2}  flash {:#08x}  {}  memoire {:#010x}  {:#x} octets",
            i,
            f,
            if *vers_mem { "->" } else { "<-" },
            mem,
            len
        );
    }

    println!("
== console du firmware ({} caracteres)", console.len());
    for l in console.lines().take(40) {
        if !l.trim().is_empty() {
            println!("  {}", l.trim_end());
        }
    }

    println!("
== journal du firmware ({} messages)", journal.len());
    for l in journal.iter().take(40) {
        println!("  {}", l);
    }

    println!("
== chaines lisibles en SRAM");
    let mut courant = String::new();
    let mut trouvees: Vec<(u32, String)> = Vec::new();
    let mut debut = 0u32;
    for (i, &b) in m.bus.sram.data.iter().enumerate() {
        let c = b as char;
        if (0x20..0x7f).contains(&b) || b == 10 || b == 13 {
            if courant.is_empty() {
                debut = 0x1800_0000 + i as u32;
            }
            courant.push(if b == 13 { ' ' } else { c });
        } else {
            if courant.trim().len() >= 6 {
                trouvees.push((debut, courant.clone()));
            }
            courant.clear();
        }
    }
    if courant.trim().len() >= 6 {
        trouvees.push((debut, courant.clone()));
    }
    if trouvees.is_empty() {
        println!("  aucune");
    }
    for (a, t) in trouvees.iter().take(30) {
        println!("  {:#010x}  {}", a, t.replace(char::from(10), " / ").trim());
    }

    if !m.periph.uart.console_history.is_empty() {
        println!("\n== UART");
        println!("{}", m.periph.uart.console_history);
    }
}
