//! Emulation on a thread of its own, separate from the interface.
//!
//! # Why
//!
//! The interface loop gave emulation a slice of `budget_ms` milliseconds, forty
//! by default, and then drew. A frame therefore lasted the slice plus the
//! drawing: forty-five milliseconds, some twenty frames per second, whatever
//! the speed of the core. The two jobs contended for one thread although they
//! have no reason to wait for each other.
//!
//! # How
//!
//! The worker thread owns the machine. It never lends it out: the interface
//! reads only a mirror, the [`Vitrine`], which the thread republishes on every
//! slice, and sends only [`Consigne`]s. Nothing is shared but that mirror and
//! the command channel, which avoids having to lock the machine itself while
//! drawing — a lock held for the length of a frame would make the split
//! pointless.
//!
//! The thread also owns whatever must advance at the pace of emulation rather
//! than of the display: the snapshot ring, the recovery-point journal, melody
//! tracking and the periodic save. Leaving those on the interface thread would
//! have meant fetching the machine back on every frame.
//!
//! # Handing back
//!
//! [`Fil::reprendre`] stops the thread and returns the machine and everything
//! that travels with it. The interface uses that whenever it needs the machine
//! itself: inspection mode, an open menu, restoring a snapshot. The
//! single-threaded path therefore stays intact and serves as the fallback.

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::emulator::peripherals::snsys::CYCLES_PAR_SECONDE;
use crate::emulator::reprises::Journal;
use crate::emulator::scribe::{Scribe, Tache};
use crate::emulator::{etat::Historique, Machine, StepResult};

/// System clock resolution, for as long as the thread lives.
///
/// On Windows, `Sleep` is granted at the granularity of the global timer,
/// fifteen and a half milliseconds by default. The thread, which wants to yield
/// for two or three milliseconds between slices, therefore slept five times too
/// long, built up debt, then made it up in one burst: exactly the uneven speed
/// one observes. `timeBeginPeriod(1)` brings that granularity down to a
/// millisecond. The request is released when the thread dies, as the
/// documentation requires — it is system-wide.
///
/// Elsewhere sleeping is already fine-grained and there is nothing to ask for.
#[cfg(windows)]
mod horloge {
    #[link(name = "winmm")]
    extern "system" {
        fn timeBeginPeriod(periode: u32) -> u32;
        fn timeEndPeriod(periode: u32) -> u32;
    }

    pub struct Precision;

    impl Precision {
        pub fn demander() -> Self {
            unsafe {
                timeBeginPeriod(1);
            }
            Self
        }
    }

    impl Drop for Precision {
        fn drop(&mut self) {
            unsafe {
                timeEndPeriod(1);
            }
        }
    }
}

#[cfg(not(windows))]
mod horloge {
    pub struct Precision;

    impl Precision {
        pub fn demander() -> Self {
            Self
        }
    }
}

/// Length of one work slice between two readings of the command queue.
///
/// Four milliseconds: short enough that a press leaves on the next frame, long
/// enough that rereading the commands and republishing the mirror weigh
/// nothing.
const TRANCHE: Duration = Duration::from_millis(4);

/// What the interface needs to know about the machine while the worker thread
/// holds it.
///
/// Everything game mode draws comes from here. The single-threaded path fills
/// the same structure from the machine, so the interface draws the mirror
/// either way and nothing in the drawing knows which path is active.
#[derive(Clone)]
pub struct Vitrine {
    pub vram: Vec<u16>,
    pub largeur: usize,
    pub hauteur: usize,
    /// True when the screen memory has changed since the interface last read
    /// it. The thread sets it, the interface clears it on its copy.
    pub ecran_change: bool,
    pub trames: u64,
    pub cycles: u64,
    /// Low level of each control pin, in `TamagotchiApp::COMMANDES` order.
    pub broches: [bool; 4],
    pub en_marche: bool,
    /// Notes sampled during the elapsed slices, in order. The interface
    /// consumes and empties them.
    pub notes: Vec<(f32, u64)>,
    pub instantanes: usize,
    /// True when the machine has come out of deep sleep since the last read:
    /// the interface must then flush its press queues.
    pub reveil: bool,
    /// Message to show, typically a save that could not be written.
    pub message: Option<String>,
    /// Measured throughput, in cycles per second.
    pub debit: f64,
    /// True once the thread has published at least once.
    ///
    /// The mirror is empty between the thread starting and its first slice.
    /// Adopting it as is reset the cycle counter to zero for one frame: a
    /// button pulse computed at that moment got a tiny deadline, which the real
    /// counter — several billion — passed immediately. The press was
    /// swallowed.
    pub publie: bool,
}

impl Default for Vitrine {
    fn default() -> Self {
        Self {
            vram: Vec::new(),
            largeur: 0,
            hauteur: 0,
            ecran_change: true,
            trames: 0,
            cycles: 0,
            broches: [false; 4],
            en_marche: false,
            notes: Vec::new(),
            instantanes: 0,
            reveil: false,
            message: None,
            debit: 0.0,
            publie: false,
        }
    }
}

/// Command sent by the interface to the worker thread.
pub enum Consigne {
    /// Wanted level of the four control pins, and any encoder phases pending.
    ///
    /// The interface keeps the press logic — it alone knows the pointer, the
    /// keyboard and commands arriving from the browser — and sends only the
    /// result here.
    Entrees {
        basses: [bool; 4],
        /// Pending encoder phases, in order.
        ///
        /// The interface produces them in bursts of four — one detent — and far
        /// faster than it draws when a key repeats. It therefore sends them
        /// all, and the worker thread spreads them out, one per slice: the
        /// firmware sees every transition, at a rate that does not depend on
        /// the display's.
        encodeur: Vec<(bool, bool)>,
        /// True when input is pending: the thread then tries to bring the
        /// console out of deep sleep.
        reveil: bool,
    },
    /// Speed multiplier. Zero pauses.
    Vitesse(f32),
    /// Stops the loop and returns everything the thread owns.
    Rendre,
}

/// What the thread returns when stopped.
pub struct Rendu {
    pub machine: Box<Machine>,
    pub historique: Historique,
    pub reprises: Journal,
    pub suivi: SuiviNote,
    pub notes: Vec<(f32, u64)>,
}

/// Melody tracking state, which must travel with the machine.
///
/// The buzzer changes note several times in a hundred and fifty milliseconds:
/// one sample per interface frame would catch only fragments. Sampling
/// therefore happens between two `run_frame` calls, where it always did, but on
/// the worker thread's side.
#[derive(Clone, Copy)]
pub struct SuiviNote {
    pub son_jouait: bool,
    pub note_perimee: f32,
    pub perimee_jusqu: u64,
    pub note_courante: f32,
    pub note_depuis: u64,
}

impl Default for SuiviNote {
    fn default() -> Self {
        Self {
            son_jouait: false,
            note_perimee: 0.0,
            perimee_jusqu: 0,
            note_courante: 0.0,
            note_depuis: 0,
        }
    }
}

/// The note to play right now, with the value inherited from the previous
/// melody discarded.
///
/// Extracted from `TamagotchiApp::note_jouee` so that both paths, one thread
/// and two, call exactly the same code. The search for the voice table is
/// spaced out by `derniere_recherche`: it sweeps all of RAM and cannot happen
/// on every slice.
pub fn suivre_la_note(
    m: &mut Machine,
    s: &mut SuiviNote,
    derniere_recherche: &mut Instant,
) -> f32 {
    let joue = m.son_en_cours();
    // The sweep reads all of RAM, a hundred and twenty-eight kilobytes, then
    // compares the addresses found pairwise. It was redone at the start of
    // every sound with no rate limit: a screen that beeps twenty times a second
    // paid for it twenty times, on the interpreting thread, and it was audible.
    // So it is only redone when the retained addresses no longer carry the
    // clock — the firmware has moved its table — and, as a precaution, at the
    // start of a sound if the last sweep is more than half a second old.
    let ecoule = derniere_recherche.elapsed().as_secs_f32();
    if joue && ((!m.voix_encore_valides() && ecoule > 0.1) || (!s.son_jouait && ecoule > 0.5)) {
        m.localiser_les_voix();
        *derniere_recherche = Instant::now();
    }
    if joue && !s.son_jouait {
        s.note_perimee = m.note_courante();
        s.perimee_jusqu = m.cpu.cycles + CYCLES_PAR_SECONDE as u64 / 20;
    }
    s.son_jouait = joue;

    let note = m.note_courante();
    if note > 0.0 && m.cpu.cycles < s.perimee_jusqu {
        if (note - s.note_perimee).abs() < 0.5 {
            return 0.0;
        }
        s.perimee_jusqu = 0;
    }
    note
}

/// Fills the mirror from the machine.
///
/// Called by the worker thread after each slice, and by the single-threaded
/// path on every frame: the drawing sees the same structure either way.
pub fn garnir(v: &mut Vitrine, m: &Machine, broches: [u32; 4]) {
    let d = &m.periph.display;
    if d.dirty || v.vram.len() != d.vram.len() {
        v.vram.clear();
        v.vram.extend_from_slice(&d.vram);
        v.ecran_change = true;
    }
    v.publie = true;
    v.largeur = d.width;
    v.hauteur = d.height;
    v.trames = d.trames;
    v.cycles = m.cpu.cycles;
    v.en_marche = m.is_running;
    for (i, broche) in broches.iter().enumerate() {
        v.broches[i] = m.broche_basse(*broche);
    }
}

/// Handle on the worker thread, interface side.
pub struct Fil {
    consignes: Sender<Consigne>,
    rendus: Receiver<Rendu>,
    vitrine: Arc<Mutex<Vitrine>>,
    fini: Option<std::thread::JoinHandle<()>>,
}

impl Fil {
    /// Starts the thread and hands it the machine.
    ///
    /// `broches` gives the order of the four controls, the same as that of the
    /// input commands and of the mirror.
    pub fn demarrer(
        machine: Box<Machine>,
        historique: Historique,
        reprises: Journal,
        suivi: SuiviNote,
        vitesse: f32,
        broches: [u32; 4],
        ctx: egui::Context,
    ) -> Self {
        let (env_consignes, consignes) = std::sync::mpsc::channel::<Consigne>();
        let (env_rendus, rendus) = std::sync::mpsc::channel::<Rendu>();
        let vitrine = Arc::new(Mutex::new(Vitrine::default()));
        let miroir = Arc::clone(&vitrine);

        let fini = std::thread::Builder::new()
            .name("emulation".to_string())
            .spawn(move || {
                boucle(
                    machine, historique, reprises, suivi, vitesse, broches, ctx, consignes,
                    env_rendus, miroir,
                );
            })
            .expect("le fil d'emulation n'a pas pu demarrer");

        Self {
            consignes: env_consignes,
            rendus,
            vitrine,
            fini: Some(fini),
        }
    }

    /// Sends a command. An already-stopped thread swallows it silently: that is
    /// the normal case between asking for the machine back and receiving it.
    pub fn ordonner(&self, c: Consigne) {
        let _ = self.consignes.send(c);
    }

    /// Copies the mirror. The screen-changed flag is cleared on the way, so the
    /// interface does not rebuild the texture for nothing.
    pub fn lire(&self) -> Vitrine {
        let mut v = self.vitrine.lock().unwrap_or_else(|e| e.into_inner());
        let copie = v.clone();
        v.ecran_change = false;
        v.notes.clear();
        v.reveil = false;
        v.message = None;
        copie
    }

    /// Stops the thread and takes the machine back.
    ///
    /// Blocks until the slice in progress finishes, a few milliseconds at most.
    /// Returns `None` if the thread died of a panic: the machine is then lost,
    /// and the caller must start again from a load.
    pub fn reprendre(mut self) -> Option<Rendu> {
        let _ = self.consignes.send(Consigne::Rendre);
        let rendu = self.rendus.recv().ok();
        if let Some(h) = self.fini.take() {
            let _ = h.join();
        }
        rendu
    }
}

/// Body of the worker thread.
#[allow(clippy::too_many_arguments)]
fn boucle(
    mut machine: Box<Machine>,
    mut historique: Historique,
    mut reprises: Journal,
    mut suivi: SuiviNote,
    mut vitesse: f32,
    broches: [u32; 4],
    ctx: egui::Context,
    consignes: Receiver<Consigne>,
    rendus: Sender<Rendu>,
    miroir: Arc<Mutex<Vitrine>>,
) {
    // Held for the life of the thread, released when it returns.
    let _precision = horloge::Precision::demander();
    let scribe = Scribe::demarrer();
    reprises.confier_a(scribe.clone());
    let mut cycles_dus = 0.0f64;
    let mut pendule = Instant::now();
    let mut notes: Vec<(f32, u64)> = Vec::new();
    let mut derniere_recherche = Instant::now();
    let mut derniere_ecriture = Instant::now();
    let mut reveil_vu = false;
    let mut message: Option<String> = None;
    let mut debit_depart = (machine.cpu.cycles, Instant::now());
    let mut debit = 0.0f64;
    let mut file_encodeur: std::collections::VecDeque<(bool, bool)> =
        std::collections::VecDeque::new();

    loop {
        // 1. Pending commands. All are read before the slice: a press and a
        //    release arriving in the same frame must leave in order, and only
        //    the last state counts.
        loop {
            match consignes.try_recv() {
                Ok(Consigne::Vitesse(v)) => vitesse = v,
                Ok(Consigne::Entrees {
                    basses,
                    encodeur,
                    reveil,
                }) => {
                    if reveil && machine.reveiller_par_broche() {
                        // The reset put the cycle counter back to zero: the
                        // interface must flush its deadlines, which belonged
                        // to the old counter.
                        reveil_vu = true;
                        file_encodeur.clear();
                        for b in broches {
                            machine.relacher(b);
                        }
                        machine.relacher(Machine::ENCODEUR_1);
                        machine.relacher(Machine::ENCODEUR_2);
                        continue;
                    }
                    for (i, b) in broches.iter().enumerate() {
                        if basses[i] {
                            machine.appuyer(*b);
                        } else {
                            machine.relacher(*b);
                        }
                    }
                    file_encodeur.extend(encodeur);
                    // Same cap as on the interface side: beyond it, keyboard
                    // repeat produces faster than we consume.
                    while file_encodeur.len() > 64 {
                        file_encodeur.pop_front();
                    }
                }
                Ok(Consigne::Rendre) | Err(TryRecvError::Disconnected) => {
                    let _ = rendus.send(Rendu {
                        machine,
                        historique,
                        reprises,
                        suivi,
                        notes,
                    });
                    return;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        // 2. The debt follows real time, scaled by the requested speed. Same
        //    rule as the old interface loop: at most a quarter of a second of
        //    lag, beyond which catching up is abandoned.
        let dt = pendule.elapsed().as_secs_f64();
        pendule = Instant::now();
        let par_seconde = CYCLES_PAR_SECONDE as f64;
        if vitesse.is_finite() {
            cycles_dus += par_seconde * vitesse as f64 * dt;
            // The debt is bounded on both sides. Above, we give up on making
            // up more than a quarter of a second. Below, the debt goes
            // negative: a slice that does too much — the fast-forward jumps in
            // blocks and always overshoots a little — must give back what it
            // took, or the console gains time on every slice and drifts ahead
            // of real time. That is what made the speed and the sound uneven.
            // The floor keeps a single overshoot from freezing the console for
            // half a second.
            cycles_dus = cycles_dus.clamp(-par_seconde * 0.05, par_seconde * 0.25);
        } else {
            cycles_dus = f64::INFINITY;
        }

        // One encoder phase per slice, two hundred and fifty a second: each
        // holds long enough for the firmware to see it, and the queue drains
        // faster than a keyboard fills it.
        if let Some((voie1, voie2)) = file_encodeur.pop_front() {
            if voie1 {
                machine.relacher(Machine::ENCODEUR_1);
            } else {
                machine.appuyer(Machine::ENCODEUR_1);
            }
            if voie2 {
                machine.relacher(Machine::ENCODEUR_2);
            } else {
                machine.appuyer(Machine::ENCODEUR_2);
            }
        }

        // 3. One slice of work.
        let depart = machine.cpu.cycles;
        if machine.is_running && vitesse > 0.0 {
            let debut = Instant::now();
            while (machine.cpu.cycles.saturating_sub(depart) as f64) < cycles_dus && debut.elapsed() < TRANCHE
            {
                if !matches!(machine.run_frame(), StepResult::Ok(_)) {
                    break;
                }
                let note = suivre_la_note(&mut machine, &mut suivi, &mut derniere_recherche);
                if (note - suivi.note_courante).abs() > 0.5 {
                    let duree = machine.cpu.cycles.saturating_sub(suivi.note_depuis);
                    notes.push((suivi.note_courante, duree));
                    suivi.note_courante = note;
                    suivi.note_depuis = machine.cpu.cycles;
                }
            }
            // The note in progress is closed at the end of each slice, as the
            // interface loop used to. Without that, a long-held note is only
            // pushed when it changes: the buzzer got a long silence then a
            // block, which sounds like chopping.
            let reste = machine.cpu.cycles.saturating_sub(suivi.note_depuis);
            if reste > 0 {
                notes.push((suivi.note_courante, reste));
                suivi.note_depuis = machine.cpu.cycles;
            }
            let faits = machine.cpu.cycles.saturating_sub(depart) as f64;
            cycles_dus -= faits;
            historique.suivre(&machine);
            reprises.suivre(&machine);
            // Debt paid before the end of the slice: the core is ahead of real
            // time. Without this pause the loop went straight round again with
            // nothing to do, holding a whole core spinning — something the old
            // interface loop did not have to fear, the display's pace holding
            // it back. On a laptop that burnt core took from the interface the
            // thermal headroom it then lacked. Turbo mode, at infinite speed,
            // is not concerned: its debt is never paid off.
            if vitesse.is_finite() {
                // The remaining debt says how much real time must pass before
                // there is work again. Sleep that long, bounded so commands are
                // still picked up promptly, rather than a fixed duration.
                let par_seconde_reelle = par_seconde * vitesse as f64;
                if cycles_dus < 0.0 && par_seconde_reelle > 0.0 {
                    let attente = (-cycles_dus / par_seconde_reelle).min(0.008);
                    if attente > 0.0005 {
                        std::thread::sleep(Duration::from_secs_f64(attente));
                    }
                }
            }
        } else {
            // While paused the thread must not spin on a whole core.
            std::thread::sleep(TRANCHE);
        }

        // 4. The save follows the game to disk at the same rate as before, but
        //    the system call goes to the writer thread: every other second, the
        //    wait on the disk showed on screen once the console kept up.
        if machine.sauvegarde_a_ecrire() && derniere_ecriture.elapsed() >= Duration::from_secs(1) {
            derniere_ecriture = Instant::now();
            if let Some((chemin, contenu)) = machine.sauvegarde_a_confier() {
                if !scribe.confier(Tache::Octets { chemin, contenu }) {
                    // The writer thread is gone: write it ourselves rather
                    // than lose the save.
                    if let Err(e) = machine.ecrire_sauvegarde() {
                        message = Some(e);
                    }
                }
            }
        }

        // 5. Actual throughput, measured over half a second.
        let ecoule = debit_depart.1.elapsed().as_secs_f64();
        if ecoule >= 0.5 {
            debit = machine.cpu.cycles.saturating_sub(debit_depart.0) as f64 / ecoule;
            debit_depart = (machine.cpu.cycles, Instant::now());
        }

        // 6. Republish the mirror. The lock is held only for the copy of the
        //    screen memory, thirty-two kilobytes at most.
        let ecran_change = machine.periph.display.dirty;
        {
            let mut v = miroir.lock().unwrap_or_else(|e| e.into_inner());
            garnir(&mut v, &machine, broches);
            machine.periph.display.dirty = false;
            v.instantanes = historique.len();
            v.debit = debit;
            v.notes.append(&mut notes);
            if reveil_vu {
                v.reveil = true;
                reveil_vu = false;
            }
            if let Some(m) = message.take() {
                v.message = Some(m);
            }
        }
        // The interface is woken only when the console's screen has changed.
        // Waking it on every slice, every four milliseconds, made it redraw the
        // shell far more often than the console produces frames, and that
        // drawing took from the core what little thermal headroom a laptop has.
        // For the rest — pressed buttons, messages — the interface keeps its
        // own pace.
        if ecran_change {
            ctx.request_repaint();
        }
    }
}
