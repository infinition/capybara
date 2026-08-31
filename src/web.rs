//! Petit serveur local, pour suivre l'emulation depuis un navigateur.
//!
//! Il n'ajoute aucune dependance : une ecoute TCP, un decoupage sommaire de la
//! requete, trois routes. L'ecran part en hexadecimal et c'est la page qui le
//! peint sur une toile, ce qui evite d'encoder une image cote emulateur.
//!
//! Il n'ecoute que sur la boucle locale.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// Vitesse du temps de la console, en pourcentage du temps reel.
    Temps(u32),
    Son(bool),
    Volume(u8),
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
    pub edition: String,
    pub sauvegarde: String,
    pub langue: String,
    pub en_marche: bool,
    pub vitesse: u32,
    pub son: bool,
    pub volume: u8,
    pub titre: String,
    pub corps: [u8; 3],
    pub calotte: [u8; 3],
    pub ombre: [u8; 3],
    pub bouton: [u8; 3],
    pub accent: [u8; 3],
    pub motif: [u8; 3],
    /// Commandes recues et pas encore appliquees.
    pub commandes: Vec<Commande>,
}

const PAGE: &str = r#"<!doctype html>
<html lang="fr"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Capybara</title><style>
:root{color-scheme:dark;--corps:#70beea;--calotte:#b2e0f8;--ombre:#347ab4;--bouton:#807ad0;--accent:#7670ca;--motif:#4a98ce}
*{box-sizing:border-box}body{margin:0;background:#0d1017;color:#edf1fa;font:14px system-ui,-apple-system,"Segoe UI",sans-serif}
header{height:64px;display:flex;align-items:center;gap:14px;padding:0 24px;border-bottom:1px solid #252b38;background:#121620}
.logo{width:34px;height:34px;border-radius:10px;background:linear-gradient(145deg,var(--calotte),var(--corps));box-shadow:inset 0 0 0 2px #ffffff55}
h1{font-size:17px;margin:0}.sub{font-size:12px;color:#8993a8;margin-top:2px}.spacer{flex:1}
.tabs,.langues{display:flex;padding:3px;background:#0b0e14;border:1px solid #2a3140;border-radius:10px}
.tabs button,.langues button{border:0;background:transparent;color:#9ba5b9;padding:7px 12px;border-radius:7px;cursor:pointer}
.tabs button.actif,.langues button.actif{background:#293246;color:white}
main{max-width:1120px;margin:auto;padding:26px}.page{display:none}.page.actif{display:block}
.jeu{display:grid;grid-template-columns:minmax(440px,560px) minmax(260px,1fr);gap:42px;align-items:center}
.console-zone{display:flex;justify-content:center;padding:12px 40px 20px 0}
.tama{position:relative;width:420px;height:500px;border-radius:49% 49% 44% 44%/55% 55% 42% 42%;background:var(--corps);border:4px solid var(--ombre);box-shadow:inset 0 -18px 0 #0000000d,0 25px 70px #0008}
.calotte{position:absolute;inset:0 0 57% 0;border-radius:50% 50% 30% 30%/70% 70% 25% 25%;background:var(--calotte);overflow:hidden}
.calotte:after{content:"";position:absolute;left:-5%;right:-5%;bottom:-2px;height:68px;background:var(--corps);clip-path:polygon(0 65%,8% 30%,16% 66%,25% 22%,35% 68%,45% 25%,55% 66%,65% 20%,75% 68%,84% 28%,92% 66%,100% 32%,100% 100%,0 100%)}
.motif{position:absolute;left:76px;right:76px;top:128px;height:246px;background:var(--motif);clip-path:polygon(50% 0,62% 11%,76% 8%,82% 24%,96% 32%,88% 48%,96% 62%,82% 72%,76% 90%,60% 88%,50% 100%,39% 88%,23% 90%,18% 72%,4% 62%,12% 48%,4% 32%,18% 24%,24% 8%,39% 11%)}
.titre{position:absolute;top:137px;left:0;right:0;text-align:center;color:var(--accent);font-size:22px;font-weight:700;letter-spacing:1px}
.vitre{position:absolute;left:87px;top:172px;width:246px;height:246px;padding:8px;border-radius:10px;background:#edf8ff;border:4px solid var(--ombre);box-shadow:0 5px 16px #0005}
canvas{width:222px;height:222px;display:block;background:#111;image-rendering:pixelated;border:2px solid #37b7dc}
.roue{position:absolute;right:-55px;top:175px;width:76px;height:126px;border:4px solid var(--ombre);border-radius:28px;background:var(--bouton);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;box-shadow:0 7px 14px #0005}
.roue button{width:46px;height:42px;border:0;border-radius:13px;background:var(--accent);color:white;font-size:18px;cursor:pointer}
.commandes{position:absolute;left:90px;right:90px;bottom:38px;display:flex;justify-content:space-between}
.commande{width:58px;height:58px;border-radius:50%;border:3px solid var(--ombre);background:var(--bouton);color:white;font-weight:800;cursor:pointer;box-shadow:inset 0 -5px 0 #0002}.commande:active,.roue button:active{transform:translateY(2px)}
.panneau{background:#151a24;border:1px solid #293142;border-radius:16px;padding:22px;box-shadow:0 18px 45px #0004}
.panneau h2{font-size:18px;margin:0 0 6px}.etat{color:#98a3b8;margin-bottom:20px}.pastille{display:inline-block;width:8px;height:8px;border-radius:50%;background:#56d38b;margin-right:6px}
.actions{display:grid;grid-template-columns:1fr 1fr;gap:8px}.actions button,.primaire,.secondaire{border:1px solid #39445a;border-radius:9px;padding:10px;background:#252d3e;color:#f2f5fb;cursor:pointer}.actions button:hover,.primaire:hover{background:#303b51}
.aide{font-size:12px;color:#8993a8;line-height:1.55;margin-top:18px}.raccourcis{display:flex;gap:7px;flex-wrap:wrap;margin-top:16px}.raccourcis button{font-size:12px;padding:7px 9px;background:#171d29;border:1px solid #31394b;color:#cdd4e1;border-radius:8px;cursor:pointer}
.grille{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:16px}.carte{background:#151a24;border:1px solid #293142;border-radius:14px;padding:20px}.carte.large{grid-column:1/-1}.carte h2{font-size:15px;margin:0 0 16px}.ligne{display:flex;align-items:center;gap:10px;margin:12px 0}.ligne label{min-width:120px;color:#a9b3c5}.ligne input[type=range]{flex:1}.ligne input[type=text]{flex:1;min-width:80px;background:#0d1119;color:#eef;border:1px solid #343e52;padding:9px;border-radius:8px}
.segmente{display:flex;gap:5px}.segmente button{padding:7px 11px;border:1px solid #364158;background:#202737;color:#cbd3e2;border-radius:7px;cursor:pointer}.segmente button.actif{background:var(--accent);color:white;border-color:transparent}
pre{max-height:300px;overflow:auto;background:#0b0e14;border:1px solid #252c3b;border-radius:9px;padding:13px;color:#b8c1d3;font:12px/1.5 ui-monospace,Consolas,monospace;white-space:pre-wrap}
@media(max-width:850px){header{padding:0 12px}.jeu{grid-template-columns:1fr}.console-zone{transform:scale(.82);margin:-48px -45px}.grille{grid-template-columns:1fr}.carte.large{grid-column:auto}.sub{display:none}}
</style></head><body>
<header><div class="logo"></div><div><h1>Capybara</h1><div class="sub" id="edition">Paradise</div></div><div class="spacer"></div>
<div class="tabs"><button class="actif" data-page="console" data-t="console">Console</button><button data-page="reglages" data-t="settings">Réglages</button></div>
<div class="langues"><button class="actif" data-lang="fr">FR</button><button data-lang="en">EN</button></div></header>
<main><section id="console" class="page actif"><div class="jeu">
<div class="console-zone"><div class="tama"><div class="calotte"></div><div class="motif"></div><div class="titre" id="titre">CAPYBARA</div>
<div class="vitre"><canvas id="ecran" width="128" height="128"></canvas></div>
<div class="roue"><button onclick="cmd('tourner_avant')">▲</button><button data-b="ok">●</button><button onclick="cmd('tourner_arriere')">▼</button></div>
<div class="commandes"><button class="commande" data-b="a">A</button><button class="commande" data-b="b">B</button><button class="commande" data-b="c">C</button></div></div></div>
<aside class="panneau"><h2 data-t="remote">Télécommande</h2><div class="etat"><span class="pastille"></span><span id="statut">Connexion...</span></div>
<div class="actions"><button data-b="a" data-t="up">A · Monter</button><button data-b="b" data-t="validate">B · Valider</button><button data-b="c" data-t="back">C · Retour</button><button data-b="ok" data-t="wheel">Molette · Appui</button></div>
<div class="raccourcis"><button onclick="cmd('reculer')" data-t="undo">Annuler 2 secondes</button><button onclick="long_('ok')" data-t="longwheel">Appui long molette</button><button onclick="long_('b')" data-t="longb">Appui long B</button></div>
<p class="aide" data-t="help">Clavier : A ou flèche gauche, B ou espace, C ou flèche droite, Entrée pour la molette, flèches haut et bas pour la tourner.</p></aside>
</div></section>
<section id="reglages" class="page"><div class="grille">
<div class="carte"><h2 data-t="gameplay">Jeu</h2><div class="ligne"><label data-t="time">Vitesse du temps</label><div class="segmente" id="temps"><button data-v="0" data-t="pause">Pause</button><button data-v="100" data-t="realtime">Temps réel</button></div></div>
<div class="ligne"><label data-t="save">Sauvegarde</label><strong id="sauvegarde">aucune</strong></div></div>
<div class="carte"><h2 data-t="audio">Son</h2><div class="ligne"><label data-t="enabled">Actif</label><input id="son" type="checkbox"></div><div class="ligne"><label data-t="volume">Volume</label><input id="volume" type="range" min="0" max="100"></div></div>
<div class="carte large"><h2 data-t="advanced">Outils avancés</h2><div class="ligne"><label data-t="firmware">Firmware</label><input id="chemin" type="text" placeholder="C:\chemin\firmware.bin"><button class="primaire" onclick="reglage('firmware','chemin',ch())" data-t="load">Charger</button></div>
<div class="ligne"><label data-t="snapshot">Instantané</label><input id="etat" type="text" placeholder="C:\chemin\partie.tamastate"><button class="secondaire" onclick="reglage('sauver','chemin',et())" data-t="export">Exporter</button><button class="primaire" onclick="reglage('restaurer','chemin',et())" data-t="restore">Restaurer</button></div></div>
<div class="carte large"><h2 data-t="diagnostic">Diagnostic</h2><pre id="diag">chargement...</pre></div>
</div></section></main>
<script>
const traductions={fr:{console:'Console',settings:'Réglages',remote:'Télécommande',up:'A · Monter',validate:'B · Valider',back:'C · Retour',wheel:'Molette · Appui',undo:'Annuler 2 secondes',longwheel:'Appui long molette',longb:'Appui long B',help:'Clavier : A ou flèche gauche, B ou espace, C ou flèche droite, Entrée pour la molette, flèches haut et bas pour la tourner.',gameplay:'Jeu',time:'Vitesse du temps',pause:'Pause',realtime:'Temps réel',save:'Sauvegarde',audio:'Son',enabled:'Actif',volume:'Volume',advanced:'Outils avancés',firmware:'Firmware',load:'Charger',snapshot:'Instantané',export:'Exporter',restore:'Restaurer',diagnostic:'Diagnostic',connected:'Connecté au serveur local',unreachable:'Émulateur injoignable'},en:{console:'Console',settings:'Settings',remote:'Remote control',up:'A · Up',validate:'B · Select',back:'C · Back',wheel:'Wheel · Press',undo:'Undo 2 seconds',longwheel:'Long wheel press',longb:'Long B press',help:'Keyboard: A or Left, B or Space, C or Right, Enter presses the wheel, Up and Down rotate it.',gameplay:'Game',time:'Time speed',pause:'Pause',realtime:'Real time',save:'Save',audio:'Audio',enabled:'Enabled',volume:'Volume',advanced:'Advanced tools',firmware:'Firmware',load:'Load',snapshot:'Snapshot',export:'Export',restore:'Restore',diagnostic:'Diagnostic',connected:'Connected to local server',unreachable:'Emulator unreachable'}};
let langue='fr', image;
const ctx=document.getElementById('ecran').getContext('2d');
function traduire(){document.documentElement.lang=langue;document.querySelectorAll('[data-t]').forEach(n=>n.textContent=traductions[langue][n.dataset.t]);document.querySelectorAll('[data-lang]').forEach(b=>b.classList.toggle('actif',b.dataset.lang===langue))}
document.querySelectorAll('[data-lang]').forEach(b=>b.onclick=()=>{langue=b.dataset.lang;traduire()});
document.querySelectorAll('[data-page]').forEach(b=>b.onclick=()=>{document.querySelectorAll('[data-page]').forEach(x=>x.classList.toggle('actif',x===b));document.querySelectorAll('.page').forEach(x=>x.classList.toggle('actif',x.id===b.dataset.page))});
function cmd(n,action){fetch('/bouton?nom='+n+(action?'&action='+action:''))}
function reglage(quoi,cle,valeur){fetch('/reglage?quoi='+quoi+'&'+cle+'='+encodeURIComponent(valeur))}
function long_(n){fetch('/bouton?nom='+n+'&action=long&secondes=2')}
const ch=()=>document.getElementById('chemin').value,et=()=>document.getElementById('etat').value;
for(const b of document.querySelectorAll('[data-b]')){const nom=b.dataset.b,bas=e=>{e.preventDefault();cmd(nom,'bas')},haut=()=>cmd(nom,'haut');b.addEventListener('mousedown',bas);b.addEventListener('touchstart',bas,{passive:false});for(const ev of ['mouseup','mouseleave','touchend','touchcancel'])b.addEventListener(ev,haut)}
document.querySelectorAll('#temps button').forEach(b=>b.onclick=()=>reglage('temps','pourcent',b.dataset.v));
document.getElementById('son').onchange=e=>reglage('son','actif',e.target.checked?1:0);
document.getElementById('volume').onchange=e=>reglage('volume','valeur',e.target.value);
function couleur(nom,valeur){document.documentElement.style.setProperty('--'+nom,valeur)}
async function boucle(){try{const r=await fetch('/etat.json',{cache:'no-store'}),e=await r.json(),p=e.pixels,w=e.largeur||128,h=e.hauteur||128;if(!image||image.width!==w||image.height!==h){document.getElementById('ecran').width=w;document.getElementById('ecran').height=h;image=ctx.createImageData(w,h)}for(let i=0;i<w*h;i++){const v=parseInt(p.substr(i*4,4),16);image.data[i*4]=((v>>11)&31)*255/31;image.data[i*4+1]=((v>>5)&63)*255/63;image.data[i*4+2]=(v&31)*255/31;image.data[i*4+3]=255}ctx.putImageData(image,0,0);for(const n of ['corps','calotte','ombre','bouton','accent','motif'])couleur(n,e[n]);document.getElementById('titre').textContent=e.titre||'CAPYBARA';document.getElementById('edition').textContent='Paradise · '+e.edition;document.getElementById('sauvegarde').textContent=e.sauvegarde||'—';document.getElementById('son').checked=e.son;document.getElementById('volume').value=e.volume;document.querySelectorAll('#temps button').forEach(b=>b.classList.toggle('actif',+b.dataset.v===e.vitesse));document.getElementById('diag').textContent=e.diagnostic;document.getElementById('statut').textContent=traductions[langue].connected}catch(err){document.getElementById('statut').textContent=traductions[langue].unreachable;document.getElementById('diag').textContent=String(err)}setTimeout(boucle,150)}
const touches={a:'a',A:'a',q:'a',Q:'a',ArrowLeft:'a',b:'b',B:'b',' ':'b',c:'c',C:'c',d:'c',D:'c',ArrowRight:'c',Enter:'ok',s:'ok',S:'ok'},tenues=new Set();document.addEventListener('keydown',e=>{if(e.target.matches('input'))return;if(e.key==='ArrowUp'){e.preventDefault();cmd('tourner_avant');return}if(e.key==='ArrowDown'){e.preventDefault();cmd('tourner_arriere');return}const n=touches[e.key];if(n&&!tenues.has(n)){e.preventDefault();tenues.add(n);cmd(n,'bas')}});document.addEventListener('keyup',e=>{const n=touches[e.key];if(n&&tenues.delete(n))cmd(n,'haut')});
traduire();boucle();
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
        let couleur = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        let corps = format!(
            "{{\"pixels\":\"{}\",\"largeur\":{},\"hauteur\":{},\"trames\":{},\
             \"diagnostic\":\"{}\",\"edition\":\"{}\",\"sauvegarde\":\"{}\",\
             \"langue\":\"{}\",\"en_marche\":{},\"vitesse\":{},\"son\":{},\
             \"volume\":{},\"titre\":\"{}\",\"corps\":\"{}\",\"calotte\":\"{}\",\
             \"ombre\":\"{}\",\"bouton\":\"{}\",\"accent\":\"{}\",\"motif\":\"{}\"}}",
            pixels,
            etat.largeur,
            etat.hauteur,
            etat.trames,
            echapper(&etat.diagnostic),
            echapper(&etat.edition),
            echapper(&etat.sauvegarde),
            echapper(&etat.langue),
            etat.en_marche,
            etat.vitesse,
            etat.son,
            etat.volume,
            echapper(&etat.titre),
            couleur(etat.corps),
            couleur(etat.calotte),
            couleur(etat.ombre),
            couleur(etat.bouton),
            couleur(etat.accent),
            couleur(etat.motif),
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
            Some("temps") => valeur("pourcent").and_then(|v| v.parse().ok()).map(Commande::Temps),
            Some("son") => valeur("actif").map(|v| Commande::Son(v == "1")),
            Some("volume") => valeur("valeur").and_then(|v| v.parse().ok()).map(Commande::Volume),
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

/// Demarre l'ecoute sur la boucle locale. Rend le port retenu et le temoin qui
/// permet de l'arreter.
///
/// L'ecoute est non bloquante et le temoin relu entre deux tentatives : sans
/// cela le fil resterait pris dans `accept` et le serveur ne s'arreterait qu'a
/// la fermeture du logiciel.
pub fn demarrer(
    partage: Arc<Mutex<Partage>>,
    port: u16,
) -> Result<(u16, Arc<AtomicBool>), String> {
    let ecoute = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    let port = ecoute.local_addr().map_err(|e| e.to_string())?.port();
    ecoute.set_nonblocking(true).map_err(|e| e.to_string())?;
    let actif = Arc::new(AtomicBool::new(true));
    let temoin = Arc::clone(&actif);
    std::thread::spawn(move || {
        while temoin.load(Ordering::Relaxed) {
            match ecoute.accept() {
                Ok((flux, _)) => {
                    let partage = Arc::clone(&partage);
                    let _ = flux.set_nonblocking(false);
                    // Un fil par requete : la page en fait quatre par seconde,
                    // ca suffit largement et ca evite qu'une connexion lente
                    // bloque les autres.
                    std::thread::spawn(move || servir(flux, &partage));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
    });
    Ok((port, actif))
}
