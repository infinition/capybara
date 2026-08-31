//! Retrouve la deviceKey d'un dump sans la connaitre.
//!
//! Usage : cargo run --release --example cle_probe -- <dump.bin> [fils]
//!
//! Sert a eprouver la recherche avant de la mettre dans l'interface : sur un
//! dump dont on connait deja la cle, elle doit rendre exactement celle la.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let mut a = std::env::args().skip(1);
    let chemin = a.next().expect("dump.bin");
    let fils: usize = a
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));

    let buf = std::fs::read(&chemin).expect("dump illisible");
    println!("dump de {} octets, {} fils", buf.len(), fils);

    let avancement = Arc::new(AtomicU64::new(0));
    let arret = Arc::new(AtomicBool::new(false));
    let suivi = Arc::clone(&avancement);
    let fini = Arc::clone(&arret);
    std::thread::spawn(move || {
        let debut = Instant::now();
        while !fini.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let n = suivi.load(Ordering::Relaxed);
            let s = debut.elapsed().as_secs_f64().max(0.001);
            println!(
                "  {:.1} % essayes, {:.0} millions par seconde",
                n as f64 * 100.0 / 4_294_967_296.0,
                n as f64 / s / 1e6
            );
        }
    });

    let debut = Instant::now();
    let trouvee = capybara::emulator::sonix::recherche_cle::chercher(
        &buf,
        fils,
        Arc::clone(&avancement),
        Arc::clone(&arret),
    );
    arret.store(true, Ordering::Relaxed);
    match trouvee {
        Some(cle) => println!("\ncle trouvee : {cle:#010X}  en {:.1} s", debut.elapsed().as_secs_f64()),
        None => println!("\naucune cle ne convient, en {:.1} s", debut.elapsed().as_secs_f64()),
    }
}
