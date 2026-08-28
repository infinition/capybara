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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commande {
    /// Appui bref sur une broche, designee comme le firmware la designe.
    Presser(u32),
    /// Debut ou fin d'un appui tenu.
    Tenir(u32, bool),
    /// Appui long calibre, en secondes de temps console. L'emulateur ne
    /// tournant pas a la vitesse de la console, tenir a la main ne donne pas une
    /// duree connue : celle-ci en est une.
    Long(u32, u32),
    Tourner(i32),
    Reculer,
    /// Chemin d'un dump de flash a charger.
    Charger(String),
    /// Temps accorde a l'emulation par image, en millisecondes.
    Vitesse(u64),
    /// Ecrire l'etat courant dans un fichier.
    SauverEtat(String),
    /// Repartir d'un etat enregistre.
    ChargerEtat(String),
}

/// Decode les sequences pour cent d'une adresse.
///
/// Les chemins Windows portent des deux points, des barres obliques inverses et
/// des espaces : la page les encode, il faut les rendre tels quels.
fn decoder(s: &str) -> String {
    let octets = s.as_bytes();
    let mut sortie = Vec::with_capacity(octets.len());
    let mut i = 0;
    while i < octets.len() {
        match octets[i] {
            b'%' if i + 2 < octets.len() => {
                let paire = std::str::from_utf8(&octets[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(paire, 16) {
                    Ok(v) => {
                        sortie.push(v);
                        i += 3;
                    }
                    Err(_) => {
                        sortie.push(octets[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                sortie.push(b' ');
                i += 1;
            }
            c => {
                sortie.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&sortie).into_owned()
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
<button data-b="a">A, monter</button>
<button data-b="b">B, valider</button>
<button data-b="c">C, annuler</button>
<button data-b="ok">Molette, appui</button>
<button onclick="cmd('tourner_avant')">Molette droite</button>
<button onclick="cmd('tourner_arriere')">Molette gauche</button>
<button onclick="cmd('reculer')">Revenir en arriere</button>
</div>
<div style="margin-top:8px">Appui long, en secondes de temps console :
<button onclick="long_('a')">A</button>
<button onclick="long_('ok')">OK</button>
<button onclick="long_('c')">C</button>
<button onclick="long_('b')">B</button>
<select id="duree" style="padding:6px;border-radius:6px;background:#181c26;
color:#dfe3ee;border:1px solid #3d4761">
<option value="1">1 s</option><option value="2" selected>2 s</option>
<option value="3">3 s</option><option value="5">5 s</option>
</select>
</div>
<p style="font-size:12px;color:#9aa4bd;max-width:384px">
Garde le bouton enfonce, ou au clavier tiens la touche, pour un appui tenu.
L'emulateur tournant a environ un tiers de la vitesse de la console, tenir
trois secondes a la main n'en fait qu'une a ses yeux : les boutons d'appui
long ci-dessus tiennent une duree connue, mesuree en temps console.
<br><br>Clavier : A ou fleche gauche, B ou espace, C ou fleche droite, Entree
pour l'appui de molette, fleches haut et bas pour la tourner. Les touches
tenues se combinent, ce que les boutons de la page ne savent pas faire :
molette plus B pour le menu special, A plus C pour la remise a zero.
</p>
<div style="margin-top:6px">
<div style="margin-bottom:6px">Vitesse
<button onclick="reglage('vitesse','ms',0)">Pause</button>
<button onclick="reglage('vitesse','ms',12)">Normale</button>
<button onclick="reglage('vitesse','ms',40)">Rapide</button>
</div>
<div style="margin-bottom:6px">
<input id="chemin" style="width:330px;padding:7px;border-radius:6px;border:1px solid #3d4761;
background:#181c26;color:#dfe3ee" placeholder="C:\chemin\vers\firmware.bin">
<button onclick="reglage('firmware','chemin',ch())">Charger le .bin</button>
</div>
<div>
<input id="etat" style="width:330px;padding:7px;border-radius:6px;border:1px solid #3d4761;
background:#181c26;color:#dfe3ee" placeholder="C:\chemin\vers\partie.tamastate">
<button onclick="reglage('sauver','chemin',et())">Sauver l'etat</button>
<button onclick="reglage('restaurer','chemin',et())">Charger l'etat</button>
</div>
</div>
</div>
<pre id="diag">chargement...</pre>
</div>
<script>
const toile = document.getElementById('ecran').getContext('2d');
const image = toile.createImageData(128, 128);
function cmd(n, action){
  fetch('/bouton?nom=' + n + (action ? '&action=' + action : ''));
}
function reglage(quoi, cle, valeur){
  fetch('/reglage?quoi=' + quoi + '&' + cle + '=' + encodeURIComponent(valeur));
}
function long_(n){
  const s = document.getElementById('duree').value;
  fetch('/bouton?nom=' + n + '&action=long&secondes=' + s);
}
const ch = () => document.getElementById('chemin').value;
const et = () => document.getElementById('etat').value;
// Un bouton tenu envoie un debut puis une fin : c'est l'appui long.
for(const b of document.querySelectorAll('button[data-b]')){
  const nom = b.dataset.b;
  const bas = e => { e.preventDefault(); cmd(nom, 'bas'); };
  const haut = () => cmd(nom, 'haut');
  b.addEventListener('mousedown', bas);
  b.addEventListener('touchstart', bas, {passive:false});
  for(const ev of ['mouseup','mouseleave','touchend','touchcancel']){
    b.addEventListener(ev, haut);
  }
}
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
// Les touches tenues se combinent, comme les boutons de la console.
const touches = {a:'a', A:'a', q:'a', Q:'a', ArrowLeft:'a',
                 b:'b', B:'b', ' ':'b',
                 c:'c', C:'c', d:'c', D:'c', ArrowRight:'c',
                 Enter:'ok', s:'ok', S:'ok'};
const tenues = new Set();
document.addEventListener('keydown', ev => {
  if(ev.key === 'ArrowUp'){ ev.preventDefault(); cmd('tourner_avant'); return; }
  if(ev.key === 'ArrowDown'){ ev.preventDefault(); cmd('tourner_arriere'); return; }
  const n = touches[ev.key];
  if(n && !tenues.has(n)){ ev.preventDefault(); tenues.add(n); cmd(n, 'bas'); }
});
document.addEventListener('keyup', ev => {
  const n = touches[ev.key];
  if(n && tenues.delete(n)){ cmd(n, 'haut'); }
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

    // Chargement, vitesse et instantanes, pour piloter depuis la page.
    if chemin.starts_with("/reglage") {
        let valeur = |cle: &str| -> Option<String> {
            chemin
                .split(&format!("{}=", cle))
                .nth(1)
                .map(|reste| decoder(reste.split(['&', ' ']).next().unwrap_or("")))
        };
        let commande = match valeur("quoi").as_deref() {
            Some("firmware") => valeur("chemin").map(Commande::Charger),
            Some("vitesse") => valeur("ms").and_then(|v| v.parse().ok()).map(Commande::Vitesse),
            Some("sauver") => valeur("chemin").map(Commande::SauverEtat),
            Some("restaurer") => valeur("chemin").map(Commande::ChargerEtat),
            _ => None,
        };
        if let Some(c) = commande {
            partage.lock().unwrap().commandes.push(c);
        }
        repondre(&mut flux, "text/plain", "ok");
        return;
    }

    if chemin.starts_with("/bouton") {
        let parametre = |cle: &str| -> Option<String> {
            chemin.split(&format!("{}=", cle)).nth(1).map(|reste| {
                reste
                    .split(['&', ' '])
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
        };
        let nom = parametre("nom").unwrap_or_default();
        let action = parametre("action").unwrap_or_else(|| "clic".to_string());
        // Identifiants du firmware : molette 0x08, A 0x09, C 0x0A, B 0x0B.
        let broche = match nom.as_str() {
            "a" => Some(0x09),
            "b" => Some(0x0B),
            "c" => Some(0x0A),
            "ok" => Some(0x08),
            _ => None,
        };
        let commande = match (broche, action.as_str()) {
            (Some(b), "bas") => Some(Commande::Tenir(b, true)),
            (Some(b), "haut") => Some(Commande::Tenir(b, false)),
            (Some(b), "long") => Some(Commande::Long(
                b,
                parametre("secondes").and_then(|v| v.parse().ok()).unwrap_or(2),
            )),
            (Some(b), _) => Some(Commande::Presser(b)),
            (None, _) => match nom.as_str() {
                "tourner_avant" => Some(Commande::Tourner(1)),
                "tourner_arriere" => Some(Commande::Tourner(-1)),
                "reculer" => Some(Commande::Reculer),
                _ => None,
            },
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
