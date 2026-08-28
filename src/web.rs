//! Petit serveur local, pour suivre l'emulation depuis un navigateur.
//!
//! Il n'ajoute aucune dependance : une ecoute TCP, un decoupage sommaire de la
//! requete, trois routes. L'ecran part en hexadecimal et c'est la page qui le
//! peint sur une toile, ce qui evite d'encoder une image cote emulateur.
//!
//! Il n'ecoute que sur la boucle locale.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// Commande envoyee par la page, a rejouer sur les vraies broches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commande {
    BoutonA,
    BoutonB,
    BoutonC,
    Molette,
    Tourner(i32),
    Reculer,
}

/// Ce que l'emulateur publie et ce qu'il recoit.
#[derive(Default)]
pub struct Partage {
    /// Image courante, un mot RGB565 par pixel.
    pub ecran: Vec<u16>,
    pub largeur: usize,
    pub hauteur: usize,
    pub diagnostic: String,
    pub trames: u64,
    /// Commandes recues et pas encore appliquees.
    pub commandes: Vec<Commande>,
}

const PAGE: &str = r#"<!doctype html><html lang="fr"><head><meta charset="utf-8">
<title>Tamagotchi Paradise, emulateur</title><style>
body{background:#11131a;color:#dfe3ee;font-family:system-ui,sans-serif;margin:0;padding:18px}
h1{font-size:16px;font-weight:600;margin:0 0 12px}
.rangee{display:flex;gap:20px;align-items:flex-start;flex-wrap:wrap}
canvas{image-rendering:pixelated;border:3px solid #2a2f3d;border-radius:6px;background:#000}
button{background:#2b3346;color:#eef;border:1px solid #3d4761;border-radius:6px;
padding:9px 14px;font-size:14px;cursor:pointer;margin:0 6px 6px 0}
button:hover{background:#39435c}
pre{background:#181c26;border:1px solid #2a2f3d;border-radius:6px;padding:12px;
font-size:12px;line-height:1.45;white-space:pre-wrap;max-width:640px;overflow:auto}
</style></head><body>
<h1>Tamagotchi Paradise, emulateur</h1>
<div class="rangee">
<div>
<canvas id="ecran" width="128" height="128" style="width:384px;height:384px"></canvas>
<div style="margin-top:10px">
<button onclick="cmd('a')">A</button>
<button onclick="cmd('ok')">OK</button>
<button onclick="cmd('c')">C</button>
<button onclick="cmd('b')">B</button>
<button onclick="cmd('haut')">Molette +</button>
<button onclick="cmd('bas')">Molette -</button>
<button onclick="cmd('reculer')">Revenir en arriere</button>
</div>
</div>
<pre id="diag">chargement...</pre>
</div>
<script>
const toile = document.getElementById('ecran').getContext('2d');
const image = toile.createImageData(128, 128);
function cmd(n){ fetch('/bouton?nom=' + n); }
async function boucle(){
  try{
    const r = await fetch('/etat.json', {cache:'no-store'});
    const e = await r.json();
    const p = e.pixels;
    for(let i = 0; i < 128*128; i++){
      const v = parseInt(p.substr(i*4, 4), 16);
      image.data[i*4]   = ((v >> 11) & 31) * 255 / 31;
      image.data[i*4+1] = ((v >> 5) & 63) * 255 / 63;
      image.data[i*4+2] = (v & 31) * 255 / 31;
      image.data[i*4+3] = 255;
    }
    toile.putImageData(image, 0, 0);
    document.getElementById('diag').textContent = e.diagnostic;
  }catch(err){
    document.getElementById('diag').textContent = 'emulateur injoignable : ' + err;
  }
  setTimeout(boucle, 250);
}
boucle();
document.addEventListener('keydown', ev => {
  const t = {a:'a', b:'b', c:'c', ' ':'ok', Enter:'ok', ArrowUp:'haut', ArrowDown:'bas'};
  if(t[ev.key]){ ev.preventDefault(); cmd(t[ev.key]); }
});
</script></body></html>"#;

/// Echappe une chaine pour la glisser dans du JSON.
fn echapper(s: &str) -> String {
    let mut sortie = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '"' => sortie.push_str("\\\""),
            '\\' => sortie.push_str("\\\\"),
            '\n' => sortie.push_str("\\n"),
            '\r' => {}
            '\t' => sortie.push_str("\\t"),
            c if (c as u32) < 0x20 => sortie.push(' '),
            c => sortie.push(c),
        }
    }
    sortie
}

fn repondre(flux: &mut TcpStream, type_contenu: &str, corps: &str) {
    let entete = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        type_contenu,
        corps.len()
    );
    let _ = flux.write_all(entete.as_bytes());
    let _ = flux.write_all(corps.as_bytes());
}

fn servir(mut flux: TcpStream, partage: &Arc<Mutex<Partage>>) {
    let mut ligne = String::new();
    if BufReader::new(&flux).read_line(&mut ligne).is_err() {
        return;
    }
    let chemin = ligne.split_whitespace().nth(1).unwrap_or("/").to_string();

    if chemin.starts_with("/etat.json") {
        let etat = partage.lock().unwrap();
        let mut pixels = String::with_capacity(etat.ecran.len() * 4);
        for &p in &etat.ecran {
            pixels.push_str(&format!("{:04x}", p));
        }
        let corps = format!(
            "{{\"pixels\":\"{}\",\"trames\":{},\"diagnostic\":\"{}\"}}",
            pixels,
            etat.trames,
            echapper(&etat.diagnostic)
        );
        drop(etat);
        repondre(&mut flux, "application/json", &corps);
        return;
    }

    if chemin.starts_with("/bouton") {
        let nom = chemin.split("nom=").nth(1).unwrap_or("").trim_end_matches(|c: char| c.is_whitespace());
        let commande = match nom {
            "a" => Some(Commande::BoutonA),
            "b" => Some(Commande::BoutonB),
            "c" => Some(Commande::BoutonC),
            "ok" => Some(Commande::Molette),
            "haut" => Some(Commande::Tourner(-1)),
            "bas" => Some(Commande::Tourner(1)),
            "reculer" => Some(Commande::Reculer),
            _ => None,
        };
        if let Some(c) = commande {
            partage.lock().unwrap().commandes.push(c);
        }
        repondre(&mut flux, "text/plain", "ok");
        return;
    }

    repondre(&mut flux, "text/html; charset=utf-8", PAGE);
}

/// Demarre l'ecoute sur la boucle locale. Rend le port retenu.
pub fn demarrer(partage: Arc<Mutex<Partage>>, port: u16) -> Result<u16, String> {
    let ecoute = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    let port = ecoute.local_addr().map_err(|e| e.to_string())?.port();
    std::thread::spawn(move || {
        for flux in ecoute.incoming().flatten() {
            let partage = Arc::clone(&partage);
            // Un fil par requete : la page en fait quatre par seconde, ca suffit
            // largement et ca evite qu'une connexion lente bloque les autres.
            std::thread::spawn(move || servir(flux, &partage));
        }
    });
    Ok(port)
}
