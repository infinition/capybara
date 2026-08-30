//! Compte les octets qui arrivent sur un port serie, sans rien emuler.
//!
//! Usage : cargo run --release --example serie_compteur -- COM10 [debit]
//!
//! Sert a trancher une question et une seule : quand un outil de transfert
//! annonce avoir envoye N octets et que la console n'en recoit que N moins
//! quarante, la perte vient elle du pilote de ports virtuels ou de notre
//! propre chaine ? Ce programme ne fait qu'ouvrir le port et lire. S'il compte
//! juste, la perte est chez nous. S'il compte faux, elle est en dessous.
//!
//! Il affiche un point par tranche de mille octets, le total a chaque seconde
//! de silence, et les premiers octets recus en hexadecimal.

use std::io::Read;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let port_nom = args.next().unwrap_or_else(|| "COM10".to_string());
    let debit: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(460_800);

    let mut port = match serialport::new(&port_nom, debit)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .timeout(Duration::from_millis(50))
        .open()
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Ouverture de {port_nom} impossible : {e}");
            std::process::exit(1);
        }
    };

    println!("Ecoute de {port_nom} a {debit} bauds. Ctrl-C pour arreter.");
    println!("Lance maintenant l'envoi depuis l'outil de transfert.\n");

    let mut total = 0usize;
    let mut debut = Vec::new();
    let mut dernier_octet = Instant::now();
    let mut annonce = 0usize;
    let mut buf = [0u8; 8192];

    loop {
        match port.read(&mut buf) {
            Ok(n) if n > 0 => {
                total += n;
                if debut.len() < 64 {
                    let reste = 64 - debut.len();
                    debut.extend(buf[..n].iter().take(reste).copied());
                }
                dernier_octet = Instant::now();
                if total / 1000 > annonce {
                    annonce = total / 1000;
                    print!(".");
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                eprintln!("\nLecture interrompue : {e}");
                break;
            }
        }

        // Une seconde sans rien : la rafale est finie, on rend le compte.
        if total > 0 && dernier_octet.elapsed() > Duration::from_secs(1) {
            println!("\n\n=== {total} octets recus");
            let hex: Vec<String> = debut.iter().map(|b| format!("{b:02x}")).collect();
            println!("    debut : {}", hex.join(" "));
            let texte: String = debut
                .iter()
                .map(|&c| if (0x20..0x7f).contains(&c) { c as char } else { '.' })
                .collect();
            println!("    texte : {texte}");
            println!("\nCompare ce nombre a celui qu'annonce l'outil de transfert.");
            println!("Ecoute a nouveau, Ctrl-C pour quitter.\n");
            total = 0;
            annonce = 0;
            debut.clear();
        }
    }
}
