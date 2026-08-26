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
        }
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
