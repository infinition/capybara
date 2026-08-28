#![allow(dead_code)]

use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink};
use std::sync::{Arc, Mutex};

pub enum SoundEffect {
    ButtonClick,
    DialTick,
    Eat,
    Alert,
    Happy,
    Cure,
    MiniGameScore,
}

pub struct AudioEngine {
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    sink: Option<Arc<Mutex<Sink>>>,
    pub volume: f32,
    pub enabled: bool,
    /// Voie du buzzer de la console, alimentee en continu.
    buzzer: Option<Sink>,
    /// Phase courante du signal carre, gardee d'un morceau a l'autre.
    ///
    /// Sans elle, chaque tranche repartirait de zero et l'oreille entendrait un
    /// claquement a chaque image.
    phase: f32,
    /// Facteur applique a la hauteur tiree des voix du firmware.
    ///
    /// Il vaut un. La hauteur est calculee dans `Machine::hauteur_de_voix`, qui
    /// lit le champ de la voix pour ce qu'il est, un compte de rechargement, et
    /// non pour une frequence. Le reglage reste accessible pour reprendre
    /// l'accord si la base de temps devait etre ajustee.
    pub hauteur: f32,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (stream, stream_handle, sink) = match OutputStream::try_default() {
            Ok((str, handle)) => {
                let s = Sink::try_new(&handle).ok();
                (Some(str), Some(handle), s.map(|snk| Arc::new(Mutex::new(snk))))
            }
            Err(_) => (None, None, None),
        };

        Self {
            _stream: stream,
            stream_handle,
            sink,
            volume: 0.5,
            enabled: true,
            buzzer: None,
            phase: 0.0,
            hauteur: 1.0,
        }
    }

    /// Frequence d'echantillonnage des tranches de buzzer.
    const TAUX: u32 = 44_100;

    /// Pousse une suite de notes, chacune avec sa duree en cycles de console.
    ///
    /// Relever la note une fois par image d'interface ne suffit pas : une
    /// melodie dure cent cinquante millisecondes et change de note plusieurs
    /// fois dans cet intervalle. On la suit donc dans la boucle d'emulation, et
    /// on rend ici la suite complete, remise a l'echelle du temps reellement
    /// ecoule pour que le son ne s'interrompe pas entre deux images.
    pub fn buzzer_notes(&mut self, notes: &[(f32, u64)], secondes: f32) {
        if !self.enabled || self.volume <= 0.0 || notes.is_empty() || secondes <= 0.0 {
            return;
        }
        let total: u64 = notes.iter().map(|n| n.1).sum();
        if total == 0 {
            return;
        }
        if notes.iter().all(|n| n.0 <= 0.0) {
            self.silence_buzzer();
            return;
        }
        let Some(handle) = &self.stream_handle else {
            return;
        };
        if self.buzzer.is_none() {
            self.buzzer = Sink::try_new(handle).ok();
        }
        let Some(sink) = &self.buzzer else {
            return;
        };
        // La file n'est pas bornee ici. Jeter une tranche laisse un trou au
        // milieu d'une melodie, et l'ordre entendu n'est plus celui compose.
        // La cadence est deja tenue par ailleurs : on pousse autant de son que
        // de temps reel ecoule, la file ne peut donc pas s'allonger.

        let total_echantillons = (Self::TAUX as f32 * secondes) as usize;
        let mut echantillons = Vec::with_capacity(total_echantillons + notes.len());
        for &(frequence, cycles) in notes {
            let part = cycles as f64 / total as f64;
            let compte = (total_echantillons as f64 * part) as usize;
            if frequence <= 0.0 {
                echantillons.extend(std::iter::repeat(0.0).take(compte));
                self.phase = 0.0;
                continue;
            }
            let pas = frequence * self.hauteur.max(0.05) / Self::TAUX as f32;
            for _ in 0..compte {
                self.phase += pas;
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
                echantillons.push(if self.phase < 0.5 { 0.18 } else { -0.18 } * self.volume);
            }
        }
        if echantillons.is_empty() {
            return;
        }
        sink.append(SamplesBuffer::new(1, Self::TAUX, echantillons));
        sink.set_volume(1.0);
        sink.play();
    }

    /// Pousse une tranche de buzzer, a partir des voix que le firmware a
    /// calculees.
    ///
    /// Le buzzer de la console est un signal carre : additionner les voix
    /// actives rend le vrai son, sans avoir a modeliser le peripherique de
    /// sortie. La phase est gardee entre les tranches, et la file est bornee :
    /// une image en retard ne doit pas laisser un retard audible.
    pub fn buzzer(&mut self, voix: &[(f32, f32)], secondes: f32) {
        if !self.enabled || self.volume <= 0.0 {
            self.silence_buzzer();
            return;
        }
        if voix.is_empty() {
            self.silence_buzzer();
            return;
        }
        let Some(handle) = &self.stream_handle else {
            return;
        };
        if self.buzzer.is_none() {
            self.buzzer = Sink::try_new(handle).ok();
        }
        let Some(sink) = &self.buzzer else {
            return;
        };
        // Deux tranches d'avance suffisent : au dela le son trainerait derriere
        // l'image.
        if sink.len() > 2 {
            return;
        }

        let compte = (Self::TAUX as f32 * secondes).max(1.0) as usize;
        let mut echantillons = Vec::with_capacity(compte);
        // Une seule phase pour toutes les voix : le buzzer est unique, il ne
        // rend qu'un signal. On prend la voix la plus grave comme fondamentale
        // et on mele les autres a poids egal, ce que fait un haut parleur.
        let fondamentale =
            voix.iter().map(|v| v.0).fold(f32::MAX, f32::min) * self.hauteur.max(0.05);
        let ampleur = voix.iter().map(|v| v.1).fold(0.0_f32, f32::max);
        for _ in 0..compte {
            self.phase += fondamentale / Self::TAUX as f32;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            let carre = if self.phase < 0.5 { 1.0 } else { -1.0 };
            echantillons.push(carre * 0.18 * ampleur * self.volume);
        }
        sink.append(SamplesBuffer::new(1, Self::TAUX, echantillons));
        sink.set_volume(1.0);
        sink.play();
    }

    /// Laisse finir ce qui est en file, puis oublie la voie et sa phase.
    ///
    /// Le son a toujours une image ou deux de retard sur l'emulation : ce qui a
    /// ete pousse pendant la derniere image de la melodie n'est pas encore
    /// sorti quand le firmware se tait. Couper la voie a cet instant emportait
    /// la fin de chaque melodie, et ce qu'on entendait n'etait plus la suite
    /// que la console avait composee.
    pub fn silence_buzzer(&mut self) {
        if let Some(sink) = &self.buzzer {
            if !sink.empty() {
                return;
            }
            sink.stop();
        }
        self.buzzer = None;
        self.phase = 0.0;
    }

    pub fn play(&self, sfx: SoundEffect) {
        if !self.enabled || self.volume <= 0.0 {
            return;
        }

        if let Some(handle) = &self.stream_handle {
            let samples = Self::generate_samples(&sfx, self.volume);
            let buffer = SamplesBuffer::new(1, 44100, samples);
            if let Ok(sink) = Sink::try_new(handle) {
                sink.set_volume(self.volume);
                sink.append(buffer);
                sink.detach();
            }
        }
    }

    fn generate_samples(sfx: &SoundEffect, volume: f32) -> Vec<f32> {
        let sample_rate = 44100;
        let mut samples = Vec::new();

        match sfx {
            SoundEffect::ButtonClick => {
                let duration = 0.04;
                let freq = 1200.0;
                let count = (sample_rate as f32 * duration) as usize;
                for i in 0..count {
                    let t = i as f32 / sample_rate as f32;
                    let phase = t * freq * 2.0 * std::f32::consts::PI;
                    let val = if phase.sin() > 0.0 { 0.2 } else { -0.2 };
                    let envelope = 1.0 - (t / duration);
                    samples.push(val * envelope * volume);
                }
            }
            SoundEffect::DialTick => {
                let duration = 0.015;
                let freq = 2400.0;
                let count = (sample_rate as f32 * duration) as usize;
                for i in 0..count {
                    let t = i as f32 / sample_rate as f32;
                    let phase = t * freq * 2.0 * std::f32::consts::PI;
                    let val = if phase.sin() > 0.0 { 0.15 } else { -0.15 };
                    let envelope = 1.0 - (t / duration);
                    samples.push(val * envelope * volume);
                }
            }
            SoundEffect::Eat => {
                // Two crunch blips
                for freq in [800.0, 1100.0] {
                    let duration = 0.05;
                    let count = (sample_rate as f32 * duration) as usize;
                    for i in 0..count {
                        let t = i as f32 / sample_rate as f32;
                        let phase = t * freq * 2.0 * std::f32::consts::PI;
                        let val = if phase.sin() > 0.0 { 0.25 } else { -0.25 };
                        let envelope = 1.0 - (t / duration);
                        samples.push(val * envelope * volume);
                    }
                }
            }
            SoundEffect::Alert => {
                // Classic triple high pitch alert beep
                for _ in 0..3 {
                    let duration = 0.08;
                    let freq = 2048.0;
                    let count = (sample_rate as f32 * duration) as usize;
                    for i in 0..count {
                        let t = i as f32 / sample_rate as f32;
                        let phase = t * freq * 2.0 * std::f32::consts::PI;
                        let val = if phase.sin() > 0.0 { 0.3 } else { -0.3 };
                        samples.push(val * volume);
                    }
                    // silence between beeps
                    let pause = (sample_rate as f32 * 0.04) as usize;
                    samples.extend(std::iter::repeat(0.0).take(pause));
                }
            }
            SoundEffect::Happy => {
                // Ascending 4 notes melody
                for freq in [523.25, 659.25, 783.99, 1046.50] {
                    let duration = 0.07;
                    let count = (sample_rate as f32 * duration) as usize;
                    for i in 0..count {
                        let t = i as f32 / sample_rate as f32;
                        let phase = t * freq * 2.0 * std::f32::consts::PI;
                        let val = if phase.sin() > 0.0 { 0.25 } else { -0.25 };
                        let envelope = 1.0 - (t / duration) * 0.5;
                        samples.push(val * envelope * volume);
                    }
                }
            }
            SoundEffect::Cure => {
                for freq in [440.0, 554.37, 659.25, 880.0] {
                    let duration = 0.08;
                    let count = (sample_rate as f32 * duration) as usize;
                    for i in 0..count {
                        let t = i as f32 / sample_rate as f32;
                        let phase = t * freq * 2.0 * std::f32::consts::PI;
                        let val = (phase.sin() + 0.5 * (phase * 2.0).sin()) * 0.2;
                        samples.push(val * volume);
                    }
                }
            }
            SoundEffect::MiniGameScore => {
                let duration = 0.06;
                let freq = 1760.0;
                let count = (sample_rate as f32 * duration) as usize;
                for i in 0..count {
                    let t = i as f32 / sample_rate as f32;
                    let phase = t * freq * 2.0 * std::f32::consts::PI;
                    let val = if phase.sin() > 0.0 { 0.2 } else { -0.2 };
                    samples.push(val * volume);
                }
            }
        }

        samples
    }
}
