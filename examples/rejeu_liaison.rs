//! Rejoue un echange serie capture, et attrape le firmware au moment ou il
//! refuse un paquet.
//!
//! Usage : cargo run --release --example rejeu_liaison --
//!             <dump.bin> <cle hex> <etat.tamastate> <capture.recu> [scene]
//!
//! Un echange avec un outil exterieur ne s'instrumente pas en direct : la
//! moindre sonde change la fenetre de temps et l'echange n'a plus lieu. Le
//! capturer une fois puis le rejouer hors interface donne au contraire tout
//! loisir de suivre le firmware pas a pas.
//!
//! La sonde injecte les octets au rythme du debit programme, releve chaque
//! ligne que la console emet, et sur la premiere qui n'est pas un
//! acquittement, rend la pile d'appels et les registres du moment.

use capybara::emulator::etat::Instantane;
use capybara::emulator::{Machine, StepResult};

/// Scene du menu de telechargement, celle qu'attend l'outil de transfert.
const SCENE_TELECHARGEMENT: u32 = 119;
const SCENE_COURANTE: u32 = 0x1800_1BF4;
/// Etat de la machine a scenes, dont les trois bits bas donnent l'etape.
const ETAT_MACHINE: u32 = 0x1800_1BFA;

fn lire16(m: &Machine, adr: u32) -> u16 {
    let o = (adr - 0x1800_0000) as usize;
    let d = &m.bus.sram.data;
    d.get(o).copied().unwrap_or(0) as u16 | ((d.get(o + 1).copied().unwrap_or(0) as u16) << 8)
}

fn ecrire16(m: &mut Machine, adr: u32, val: u16) {
    let o = (adr - 0x1800_0000) as usize;
    if o + 1 < m.bus.sram.data.len() {
        m.bus.sram.data[o] = (val & 0xFF) as u8;
        m.bus.sram.data[o + 1] = (val >> 8) as u8;
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let dump = a.next().expect("dump.bin");
    let cle = u32::from_str_radix(a.next().expect("cle hex").trim_start_matches("0x"), 16).unwrap();
    let etat = a.next().expect("etat.tamastate");
    let capture = a.next().expect("capture.recu");
    let scene: u32 = a.next().and_then(|v| v.parse().ok()).unwrap_or(SCENE_TELECHARGEMENT);

    let octets = std::fs::read(&capture).expect("capture illisible");
    println!("capture : {} octets", octets.len());

    let mut m = Machine::new();
    m.device_key = Some(cle);
    m.load_firmware_file(&dump).unwrap();
    if let Ok(snap) = Instantane::lire(std::path::Path::new(&etat)) {
        m.restaurer(&snap);
        println!("instantane repris, scene {}", lire16(&m, SCENE_COURANTE));
    }
    m.remplacer_la_pile();
    m.is_running = true;

    // Scene 0 : on ne force rien, l'instantane est deja au bon endroit.
    if scene != 0 {
    // La machine a scenes garde son etape dans les trois bits bas de son mot
    // d'etat. Ecrire la scene voulue et remettre ces bits a zero revient a lui
    // demander d'y entrer au tour suivant, sans avoir a naviguer a l'aveugle.
    ecrire16(&mut m, SCENE_COURANTE, scene as u16);
    let etat = lire16(&m, ETAT_MACHINE);
    ecrire16(&mut m, ETAT_MACHINE, etat & !0x07);
    // Quelques millions de pas pour que la scene s'installe et arme le lien.
    for _ in 0..30_000_000u64 {
        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
    }
    println!("scene atteinte : {}", lire16(&m, SCENE_COURANTE));

    }

    // La scene attend un appui pour ouvrir le lien, et l'enchainement exact
    // varie. On appuie donc jusqu'a ce que le controleur serie s'allume, en
    // regardant son registre de controle plutot qu'en supposant un chemin.
    let bouton = match std::env::var("BOUTON").as_deref() {
        Ok("A") => Machine::BOUTON_A,
        Ok("C") => Machine::BOUTON_C,
        _ => Machine::BOUTON_B,
    };
    for essai in 1..=4 {
        if m.periph.uart.ctrl & 0x41 == 0x41 {
            break;
        }
        m.appuyer(bouton);
        for _ in 0..4_800_000u64 {
            if !matches!(m.step(), StepResult::Ok(_)) {
                break;
            }
        }
        m.relacher(bouton);
        for _ in 0..24_000_000u64 {
            if !matches!(m.step(), StepResult::Ok(_)) {
                break;
            }
        }
        println!(
            "  appui {essai} : scene {}, controle serie {:#06x}",
            lire16(&m, SCENE_COURANTE),
            m.periph.uart.ctrl
        );
    }

    {
        let u = &m.periph.uart;
        println!(
            "uart : ctrl={:#06x} lcr={:#06x} ier={:#06x} fcr={:#06x} debit={} rx_fifo={} rx_in={}",
            u.ctrl,
            u.lcr,
            u.ier,
            u.fcr,
            u.baud_rate(capybara::emulator::peripherals::snsys::CYCLES_PAR_SECONDE as u32),
            u.rx_fifo.len(),
            u.rx_in.len()
        );
        println!("  reception active : {}", (u.ctrl & 0x41) == 0x41);
    }

    // Injection au fil de l'execution, en suivant ce que la console repond.
    let mut reste = &octets[..];
    let mut sortie: Vec<u8> = Vec::new();
    let mut lignes: Vec<String> = Vec::new();
    let mut refus: Option<u64> = None;
    let mut pas = 0u64;

    while pas < 400_000_000 {
        // On alimente la ligne des qu'elle se vide : le controleur la debite
        // ensuite au rythme programme par le firmware.
        if m.periph.uart.rx_in.len() < 2048 && !reste.is_empty() {
            let n = reste.len().min(2048);
            m.periph.uart.inject_rx_bytes(&reste[..n]);
            reste = &reste[n..];
        }

        if !matches!(m.step(), StepResult::Ok(_)) {
            break;
        }
        pas += 1;

        for b in m.periph.uart.drain_hote() {
            sortie.push(b);
            if b == b'\n' {
                let ligne: String = String::from_utf8_lossy(&sortie).trim().to_string();
                if !ligne.is_empty() {
                    println!("  [{pas:>10}] console : {ligne}");
                    if refus.is_none() && (ligne.contains("NAK") || ligne.contains("CAN")) {
                        refus = Some(pas);
                    }
                    lignes.push(ligne);
                }
                sortie.clear();
            }
        }

        if refus.is_some() {
            break;
        }
    }

    println!("\n== {} lignes emises, {} octets restants a injecter", lignes.len(), reste.len());
    match refus {
        None => println!("  aucun refus rencontre"),
        Some(p) => {
            println!("  refus au pas {p}, PC = {:#010x}", m.cpu.regs.pc);
            println!("\n== registres");
            for i in 0..13u8 {
                print!("  r{i:<2}={:#010x}", m.cpu.regs.get_reg(i));
                if i % 4 == 3 {
                    println!();
                }
            }
            println!(
                "\n  SP={:#010x}  LR={:#010x}",
                m.cpu.regs.get_sp(),
                m.cpu.regs.lr
            );
            println!("\n== adresses de retour lues sur la pile");
            let sp = m.cpu.regs.get_sp();
            for k in 0..32u32 {
                let v = m.bus.read_u32(sp + k * 4, &mut m.periph, &m.cpu.nvic);
                let code = v & 1 == 1
                    && ((0x100..0x10000).contains(&(v & !1))
                        || (0x1000_0000..0x1010_0000).contains(&(v & !1)));
                if code {
                    println!("  [sp+{:#05x}] retour vers {:#010x}", k * 4, v & !1);
                }
            }
        }
    }
}
