//! Disk writes, kept out of the emulation loop.
//!
//! Two writes come round on their own while a console runs: the game save, once
//! a second, and a recovery point, once a minute. Both used to happen on the
//! interpreting thread.
//!
//! The first is short but frequent: create the directory, write a temporary
//! file, rename. On a machine where an antivirus inspects every file going by,
//! that is measured in milliseconds, once a second. The second is far worse: it
//! serialises a two-hundred-kilobyte snapshot to JSON, which makes several
//! megabytes of text, then writes it.
//!
//! While emulation ran behind real time these stalls were absorbed by the lag.
//! Now that the fast-forward puts it ahead, they show: the console stops for the
//! length of the write, then catches up. That is a hitch every second and a
//! longer one every minute.
//!
//! The writer is a thread that does nothing but wait for tasks and write them.
//! Whatever must be computed from the machine — encoding the save, copying the
//! snapshot — stays with the caller, which alone holds it; only serialising and
//! the system call come through here.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use super::etat::Instantane;

/// A write to perform.
pub enum Tache {
    /// Writes ready-made content, via a temporary file and a rename so that a
    /// power cut never leaves a half-written file.
    Octets { chemin: PathBuf, contenu: Vec<u8> },
    /// Serialises a snapshot, then writes it. Serialising is the expensive
    /// part, and precisely why this thread exists.
    Etat { chemin: PathBuf, etat: Box<Instantane> },
    /// Writes text as is, for the indexes.
    Texte { chemin: PathBuf, contenu: String },
    /// Deletes a file, without complaining if it is already gone.
    Effacer(PathBuf),
    /// Replies once everything before it is written.
    ///
    /// Used to fall in behind a direct write to the same file: without this
    /// meeting point, a save handed to the writer could land after the one that
    /// closing has just written, replacing it with an older version.
    Jalon(Sender<()>),
}

/// Handle on the writer thread.
///
/// Clonable: the recovery-point journal keeps one, the emulation loop another.
/// The thread stops when the last one disappears.
#[derive(Clone)]
pub struct Scribe {
    envoi: Sender<Tache>,
}

impl Scribe {
    pub fn demarrer() -> Self {
        let (envoi, reception) = std::sync::mpsc::channel::<Tache>();
        // If the thread cannot be born, sending will fail silently and the
        // caller falls back on the direct write: nothing is lost.
        let _ = std::thread::Builder::new()
            .name("ecritures".to_string())
            .spawn(move || boucle(reception));
        Self { envoi }
    }

    /// Hands over a task. Returns false if the thread is gone, in which case
    /// the caller must write it itself.
    pub fn confier(&self, tache: Tache) -> bool {
        self.envoi.send(tache).is_ok()
    }

    /// Waits for the queue to empty.
    ///
    /// Bounded at two seconds: better a save written twice than a close that
    /// never returns.
    pub fn attendre(&self) {
        let (reponse, attente) = std::sync::mpsc::channel();
        if self.envoi.send(Tache::Jalon(reponse)).is_ok() {
            let _ = attente.recv_timeout(std::time::Duration::from_secs(2));
        }
    }
}

fn boucle(reception: Receiver<Tache>) {
    while let Ok(tache) = reception.recv() {
        match tache {
            Tache::Octets { chemin, contenu } => ecrire_sur(&chemin, &contenu),
            Tache::Texte { chemin, contenu } => ecrire_sur(&chemin, contenu.as_bytes()),
            Tache::Etat { chemin, etat } => {
                if let Ok(texte) = serde_json::to_string(&*etat) {
                    ecrire_sur(&chemin, texte.as_bytes());
                }
            }
            Tache::Effacer(chemin) => {
                let _ = std::fs::remove_file(chemin);
            }
            Tache::Jalon(reponse) => {
                let _ = reponse.send(());
            }
        }
    }
}

/// Atomic write: the final file only ever appears whole.
fn ecrire_sur(chemin: &std::path::Path, contenu: &[u8]) {
    if let Some(parent) = chemin.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let provisoire = chemin.with_extension("tmp");
    if std::fs::write(&provisoire, contenu).is_ok() {
        let _ = std::fs::rename(&provisoire, chemin);
    }
}
