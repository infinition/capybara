//! LCD-only image/video capture. Encoding runs off the UI thread and retains
//! only the latest frame, so a slow encoder cannot grow an unbounded queue.
use std::io::{Read, Seek, SeekFrom, Write};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct CaptureFrame {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
}

impl CaptureFrame {
    pub fn from_rgb565(width: u32, height: u32, pixels: &[u16], enabled: bool) -> Result<Self, String> {
        let count = (width as usize).checked_mul(height as usize).ok_or("Invalid LCD dimensions")?;
        if width == 0 || height == 0 || count > 16_777_216 || pixels.len() != count {
            return Err("Invalid LCD dimensions or pixel count".into());
        }
        let mut rgb = Vec::with_capacity(count * 3);
        for &p in pixels {
            if enabled {
                rgb.extend_from_slice(&[
                    (((p >> 11) & 31) * 255 / 31) as u8,
                    (((p >> 5) & 63) * 255 / 63) as u8,
                    ((p & 31) * 255 / 31) as u8,
                ]);
            } else { rgb.extend_from_slice(&[0, 0, 0]); }
        }
        Ok(Self { width, height, rgb })
    }

    fn scaled(&self, scale: u32) -> Result<image::RgbImage, String> {
        if !(1..=16).contains(&scale) { return Err("Capture scale must be between 1 and 16".into()); }
        let width = self.width.checked_mul(scale).ok_or("Capture dimensions overflow")?;
        let height = self.height.checked_mul(scale).ok_or("Capture dimensions overflow")?;
        if width as u64 * height as u64 > 67_108_864 { return Err("Capture dimensions are too large".into()); }
        let source = image::RgbImage::from_raw(self.width, self.height, self.rgb.clone())
            .ok_or("Invalid LCD pixels")?;
        Ok(image::imageops::resize(&source, width, height, image::imageops::FilterType::Nearest))
    }
}

pub fn screenshot_png(path: &Path, frame: &CaptureFrame, scale: u32) -> Result<(), String> {
    frame.scaled(scale)?.save_with_format(path, image::ImageFormat::Png).map_err(|e| e.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureFormat { Gif, Mp4 }

fn ffmpeg_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).map(|p| p.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" })).collect())
        .unwrap_or_default();
    // Finder-launched apps do not inherit Homebrew's shell PATH.
    candidates.extend([PathBuf::from("/opt/homebrew/bin/ffmpeg"), PathBuf::from("/usr/local/bin/ffmpeg")]);
    candidates.into_iter().find(|path| path.is_file())
}

pub fn ffmpeg_available() -> bool { ffmpeg_path().is_some() }

struct State {
    frame: CaptureFrame,
    stop: bool,
    stopped_at: Option<Instant>,
    audio: VecDeque<AudioChunk>,
}
struct AudioChunk {
    start: u64,
    end: u64,
    notes: Vec<(f32, u64)>,
    volume: f32,
    pitch: f32,
    enabled: bool,
}
type Shared = Arc<(Mutex<State>, Condvar)>;
type Process = Arc<Mutex<Option<Child>>>;

pub struct Recorder {
    path: PathBuf,
    format: CaptureFormat,
    started_at: Instant,
    dimensions: (u32, u32),
    shared: Shared,
    process: Process,
    receiver: mpsc::Receiver<Result<(), String>>,
    completion: Option<Result<(), String>>,
    worker: Option<JoinHandle<()>>,
}

impl Recorder {
    pub fn start(path: PathBuf, format: CaptureFormat, initial_frame: CaptureFrame) -> Result<Self, String> {
        if path.as_os_str().is_empty() { return Err("Choose a recording file".into()); }
        let ffmpeg = if format == CaptureFormat::Mp4 {
            Some(ffmpeg_path().ok_or("MP4 recording requires ffmpeg (install it with Homebrew or use GIF)")?)
        } else { None };
        initial_frame.scaled(4)?;
        let dimensions = (initial_frame.width, initial_frame.height);
        let shared = Arc::new((Mutex::new(State { frame: initial_frame, stop: false, stopped_at: None, audio: VecDeque::new() }), Condvar::new()));
        let process: Process = Arc::new(Mutex::new(None));
        let (sender, receiver) = mpsc::channel();
        let work_shared = Arc::clone(&shared);
        let work_process = Arc::clone(&process);
        let work_path = path.clone();
        let started_at = Instant::now();
        let worker = thread::Builder::new().name("lcd-recorder".into()).spawn(move || {
            let _cleanup = ProcessCleanup(Arc::clone(&work_process));
            let result = match format {
                CaptureFormat::Gif => record_gif(&work_path, &work_shared),
                CaptureFormat::Mp4 => record_mp4(&work_path, ffmpeg.as_deref().unwrap(), &work_shared, &work_process),
            };
            if let Ok(mut state) = work_shared.0.lock() {
                state.stop = true;
                state.stopped_at.get_or_insert_with(Instant::now);
            }
            let _ = sender.send(result);
        }).map_err(|e| e.to_string())?;
        Ok(Self { path, format, started_at, dimensions, shared, process, receiver, completion: None, worker: Some(worker) })
    }

    /// Replaces one bounded latest-frame slot; no disk IO or encoding here.
    pub fn update_frame(&self, frame: &CaptureFrame) -> Result<(), String> {
        if (frame.width, frame.height) != self.dimensions { return Err("LCD dimensions changed during recording".into()); }
        if let Ok(mut state) = self.shared.0.lock() {
            if !state.stop { state.frame = frame.clone(); }
        }
        Ok(())
    }

    /// Records the emulator's own notes, without microphone/system audio or
    /// dependence on the availability of a host audio output device.
    pub fn update_audio_notes(&self, notes: &[(f32, u64)], seconds: f32, volume: f32, pitch: f32, enabled: bool) -> Result<(), String> {
        if self.format == CaptureFormat::Gif { return Ok(()); }
        if !seconds.is_finite() || seconds < 0.0 || !volume.is_finite() || !pitch.is_finite() {
            return Err("Invalid recording audio parameters".into());
        }
        let end = (self.started_at.elapsed().as_secs_f64() * 44_100.0) as u64;
        let count = (seconds.min(2.0) as f64 * 44_100.0) as u64;
        let start = end.saturating_sub(count);
        let mut state = self.shared.0.lock().map_err(|_| "Recording state unavailable")?;
        if state.stop { return Ok(()); }
        state.audio.push_back(AudioChunk { start, end, notes: notes.iter().take(4096).copied().collect(),
            volume: volume.clamp(0.0, 1.0), pitch: pitch.max(0.05), enabled });
        // Bounded independently of recording length. If the encoder stalls,
        // dropped old intervals become silence rather than delayed sound.
        while state.audio.len() > 256 || state.audio.front().is_some_and(|chunk| end.saturating_sub(chunk.start) > 88_200) {
            state.audio.pop_front();
        }
        Ok(())
    }

    pub fn stop(&self) {
        if let Ok(mut state) = self.shared.0.lock() {
            state.stop = true;
            state.stopped_at.get_or_insert_with(Instant::now);
        }
        self.shared.1.notify_all();
    }

    /// Returns a cached result once finalization finishes. Never blocks the UI.
    pub fn poll_completion(&mut self) -> Option<Result<(), String>> {
        if self.completion.is_none() {
            match self.receiver.try_recv() {
                Ok(result) => self.completion = Some(result),
                Err(mpsc::TryRecvError::Empty) => {},
                Err(mpsc::TryRecvError::Disconnected) => self.completion = Some(Err("Recording worker stopped unexpectedly".into())),
            }
        }
        if self.worker.as_ref().is_some_and(|worker| worker.is_finished()) {
            let _ = self.worker.take().unwrap().join();
        }
        self.completion.clone()
    }

    /// For application exit only; Stop in the UI should use stop()+poll_completion().
    pub fn finish(&mut self) -> Result<(), String> {
        self.stop();
        if self.completion.is_none() {
            self.completion = Some(match self.receiver.recv_timeout(Duration::from_secs(45)) {
                Ok(result) => result,
                Err(_) => {
                    kill_process(&self.process);
                    Err("Recording finalization timed out".into())
                }
            });
        }
        if self.worker.as_ref().is_some_and(|worker| worker.is_finished()) {
            let _ = self.worker.take().unwrap().join();
        }
        self.completion.clone().unwrap()
    }

    pub fn is_recording(&self) -> bool { self.completion.is_none() && !self.shared.0.lock().map(|s| s.stop).unwrap_or(true) }
    pub fn is_stopping(&self) -> bool { self.completion.is_none() && self.shared.0.lock().map(|s| s.stop).unwrap_or(true) }
    pub fn elapsed(&self) -> Duration {
        self.shared.0.lock().ok().and_then(|s| s.stopped_at).unwrap_or_else(Instant::now).saturating_duration_since(self.started_at)
    }
    pub fn path(&self) -> &Path { &self.path }
}

impl Drop for Recorder {
    fn drop(&mut self) { self.stop(); }
}

fn frame_loop(shared: &Shared, fps: u32, mut encode: impl FnMut(&CaptureFrame) -> Result<(), String>) -> Result<(), String> {
    let started = Instant::now();
    let mut tick = 0u64;
    loop {
        let frame = {
            let state = shared.0.lock().map_err(|_| "Recording state unavailable")?;
            if state.stop && tick > 0 { break; }
            state.frame.clone()
        };
        encode(&frame)?;
        tick += 1;
        let next = started + Duration::from_secs_f64(tick as f64 / fps as f64);
        let mut state = shared.0.lock().map_err(|_| "Recording state unavailable")?;
        while !state.stop && Instant::now() < next {
            state = shared.1.wait_timeout(state, next.saturating_duration_since(Instant::now()))
                .map_err(|_| "Recording state unavailable")?.0;
        }
        if state.stop { break; }
    }
    Ok(())
}

fn record_gif(path: &Path, shared: &Shared) -> Result<(), String> {
    let mut writer = std::io::BufWriter::new(std::fs::File::create(path).map_err(|e| e.to_string())?);
    let mut encoder = image::codecs::gif::GifEncoder::new_with_speed(&mut writer, 30);
    encoder.set_repeat(image::codecs::gif::Repeat::Infinite).map_err(|e| e.to_string())?;
    let result = frame_loop(shared, 10, |frame| {
        let rgba = image::DynamicImage::ImageRgb8(frame.scaled(4)?).into_rgba8();
        encoder.encode_frame(image::Frame::from_parts(rgba, 0, 0, image::Delay::from_numer_denom_ms(100, 1)))
            .map_err(|e| e.to_string())
    });
    drop(encoder); // writes the GIF trailer before flushing
    let flushed = writer.flush().map_err(|e| e.to_string());
    result.and(flushed)
}

fn kill_process(process: &Process) {
    if let Ok(mut slot) = process.lock() {
        if let Some(child) = slot.as_mut() { let _ = child.kill(); let _ = child.wait(); }
        *slot = None;
    }
}

struct ProcessCleanup(Process);
impl Drop for ProcessCleanup {
    fn drop(&mut self) { kill_process(&self.0); }
}

struct RecordingTemp(PathBuf);
impl RecordingTemp {
    fn new() -> Result<Self, String> {
        for _ in 0..10 {
            let path = std::env::temp_dir().join(format!("capybara-recording-{}-{}", std::process::id(), rand::random::<u64>()));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Could not create recording workspace".into())
    }
}
impl Drop for RecordingTemp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

struct AudioTrack {
    file: std::fs::File,
    position: u64,
    phase: f64,
}
impl AudioTrack {
    fn new(path: &Path) -> Result<Self, String> {
        let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        file.write_all(&[0; 44]).map_err(|e| e.to_string())?;
        Ok(Self { file, position: 0, phase: 0.0 })
    }
    fn emit_until(&mut self, end: u64, frequency: f32, volume: f32) -> Result<(), String> {
        let frequency = if frequency.is_finite() { frequency.clamp(0.0, 22_000.0) as f64 } else { 0.0 };
        let mut buffer = [0u8; 8192];
        while self.position < end {
            let count = (end - self.position).min(4096) as usize;
            for i in 0..count {
                let sample = if frequency > 0.0 && volume > 0.0 {
                    self.phase = (self.phase + frequency / 44_100.0).fract();
                    ((if self.phase < 0.5 { 0.18 } else { -0.18 }) * volume as f64 * 32767.0) as i16
                } else { self.phase = 0.0; 0 };
                buffer[i * 2..i * 2 + 2].copy_from_slice(&sample.to_le_bytes());
            }
            self.file.write_all(&buffer[..count * 2]).map_err(|e| e.to_string())?;
            self.position += count as u64;
        }
        Ok(())
    }
    fn drain(&mut self, shared: &Shared) -> Result<(), String> {
        let chunks: Vec<_> = shared.0.lock().map_err(|_| "Recording state unavailable")?.audio.drain(..).collect();
        for chunk in chunks {
            if chunk.end <= self.position { continue; }
            self.emit_until(chunk.start, 0.0, 0.0)?;
            let total: u128 = chunk.notes.iter().map(|(_, cycles)| *cycles as u128).sum();
            if !chunk.enabled || total == 0 {
                self.emit_until(chunk.end, 0.0, 0.0)?;
                continue;
            }
            let mut cycles = 0u128;
            for (frequency, count) in chunk.notes {
                cycles += count as u128;
                let end = chunk.start + ((chunk.end - chunk.start) as u128 * cycles / total) as u64;
                self.emit_until(end, frequency * chunk.pitch, chunk.volume)?;
            }
        }
        Ok(())
    }
    fn finish(&mut self, samples: u64) -> Result<(), String> {
        // Include silence for the final fractional video frame. Earlier audio
        // timestamps can overrun by only one frame; truncate to exact duration.
        self.emit_until(samples, 0.0, 0.0)?;
        let size = samples.checked_mul(2).and_then(|n| u32::try_from(n).ok())
            .ok_or("Recording exceeds WAV size limit")?;
        self.file.set_len(size as u64 + 44).map_err(|e| e.to_string())?;
        let mut header = Vec::with_capacity(44);
        header.extend_from_slice(b"RIFF"); header.extend_from_slice(&(size + 36).to_le_bytes());
        header.extend_from_slice(b"WAVEfmt "); header.extend_from_slice(&16u32.to_le_bytes());
        header.extend_from_slice(&1u16.to_le_bytes()); header.extend_from_slice(&1u16.to_le_bytes());
        header.extend_from_slice(&44100u32.to_le_bytes()); header.extend_from_slice(&88200u32.to_le_bytes());
        header.extend_from_slice(&2u16.to_le_bytes()); header.extend_from_slice(&16u16.to_le_bytes());
        header.extend_from_slice(b"data"); header.extend_from_slice(&size.to_le_bytes());
        self.file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        self.file.write_all(&header).and_then(|_| self.file.flush()).map_err(|e| e.to_string())
    }
}

fn record_mp4(path: &Path, ffmpeg: &Path, shared: &Shared, process: &Process) -> Result<(), String> {
    let temporary = RecordingTemp::new()?;
    let video_path = temporary.0.join("video.mp4");
    let audio_path = temporary.0.join("audio.wav");
    let mut audio = AudioTrack::new(&audio_path)?;
    let size = {
        let state = shared.0.lock().map_err(|_| "Recording state unavailable")?;
        format!("{}x{}", state.frame.width * 4, state.frame.height * 4)
    };
    let mut child = Command::new(ffmpeg).args([
        "-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pixel_format", "rgb24",
        "-video_size", &size, "-framerate", "30", "-i", "pipe:0", "-an", "-c:v", "libx264",
        "-preset", "ultrafast", "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-f", "mp4",
    ]).arg(&video_path).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let mut input = child.stdin.take().ok_or("ffmpeg stdin unavailable")?;
    let mut stderr = child.stderr.take().ok_or("ffmpeg stderr unavailable")?;
    *process.lock().map_err(|_| "Recording process unavailable")? = Some(child);
    let error_reader = thread::spawn(move || {
        let mut tail = Vec::new();
        let mut buffer = [0; 4096];
        while let Ok(n) = stderr.read(&mut buffer) {
            if n == 0 { break; }
            tail.extend_from_slice(&buffer[..n]);
            if tail.len() > 8192 { tail.drain(..tail.len() - 8192); }
        }
        String::from_utf8_lossy(&tail).into_owned()
    });
    let mut video_frames = 0u64;
    let encoded = frame_loop(shared, 30, |frame| {
        audio.drain(shared)?;
        input.write_all(frame.scaled(4)?.as_raw()).map_err(|e| e.to_string())?;
        video_frames += 1;
        Ok(())
    });
    drop(input); // EOF asks ffmpeg to write the MP4 index/trailer
    wait_for_ffmpeg(process, error_reader, Duration::from_secs(10))?;
    encoded?;
    audio.drain(shared)?;
    audio.finish((video_frames * 44_100 + 29) / 30)?;
    drop(audio);
    let mut child = Command::new(ffmpeg).args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&video_path).arg("-i").arg(&audio_path)
        .args(["-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy", "-c:a", "aac", "-b:a", "128k", "-shortest", "-movflags", "+faststart", "-f", "mp4"])
        .arg(path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let mut stderr = child.stderr.take().ok_or("ffmpeg stderr unavailable")?;
    *process.lock().map_err(|_| "Recording process unavailable")? = Some(child);
    let reader = thread::spawn(move || {
        let mut tail = Vec::new();
        let mut buffer = [0; 4096];
        while let Ok(n) = stderr.read(&mut buffer) {
            if n == 0 { break; }
            tail.extend_from_slice(&buffer[..n]);
            if tail.len() > 8192 { tail.drain(..tail.len() - 8192); }
        }
        String::from_utf8_lossy(&tail).into_owned()
    });
    wait_for_ffmpeg(process, reader, Duration::from_secs(30))
}

fn wait_for_ffmpeg(process: &Process, error_reader: JoinHandle<String>, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    let status = loop {
        let next = process.lock().map_err(|_| "Recording process unavailable")?.as_mut().map(|child| child.try_wait());
        match next {
            Some(Ok(Some(status))) => break Ok(status),
            Some(Ok(None)) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Some(Ok(None)) => { kill_process(process); break Err("ffmpeg finalization timed out".to_owned()); }
            Some(Err(error)) => { kill_process(process); break Err(error.to_string()); }
            None => break Err("ffmpeg was stopped".to_owned()),
        }
    };
    let errors = error_reader.join().unwrap_or_else(|_| "ffmpeg error reader stopped".into());
    *process.lock().map_err(|_| "Recording process unavailable")? = None;
    let status = status?;
    if !status.success() { return Err(format!("ffmpeg failed: {}", errors.trim())); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::AnimationDecoder;
    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!("capybara-capture-{}-{}", std::process::id(), rand::random::<u64>()));
        std::fs::create_dir(&path).unwrap(); path
    }
    fn test_frame() -> CaptureFrame { CaptureFrame::from_rgb565(2, 2, &[0xF800, 0x07E0, 0x001F, 0xFFFF], true).unwrap() }

    #[test]
    fn png_preserves_pixel_colors_and_nearest_neighbor_scale() {
        let dir = directory(); let path = dir.join("screen.png");
        screenshot_png(&path, &test_frame(), 4).unwrap();
        let picture = image::open(path).unwrap().into_rgb8();
        assert_eq!(picture.dimensions(), (8, 8));
        for y in 0..8 { for x in 0..8 {
            let expected = match (x / 4, y / 4) { (0, 0) => [255, 0, 0], (1, 0) => [0, 255, 0], (0, 1) => [0, 0, 255], _ => [255; 3] };
            assert_eq!(picture.get_pixel(x, y).0, expected);
        } }
        assert!(CaptureFrame::from_rgb565(2, 2, &[0], true).is_err());
        assert!(CaptureFrame::from_rgb565(2, 2, &[0xFFFF; 4], false).unwrap().rgb.iter().all(|b| *b == 0));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn static_gif_keeps_elapsed_time_and_finalizes_on_stop() {
        let dir = directory(); let path = dir.join("screen.gif");
        let mut recorder = Recorder::start(path.clone(), CaptureFormat::Gif, test_frame()).unwrap();
        thread::sleep(Duration::from_millis(260));
        recorder.stop();
        recorder.finish().unwrap();
        let frames = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()))
            .unwrap().into_frames().collect_frames().unwrap();
        assert!(frames.len() >= 2);
        assert!(frames.iter().all(|f| f.buffer().dimensions() == (8, 8)));
        let milliseconds: u32 = frames.iter().map(|f| { let (n, d) = f.delay().numer_denom_ms(); n / d }).sum();
        assert!((200..=1000).contains(&milliseconds), "GIF duration was {milliseconds} ms");
        assert!(recorder.poll_completion().unwrap().is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn real_ffmpeg_mp4_is_finalized_and_decodable() {
        let Some(ffmpeg) = ffmpeg_path() else { eprintln!("ffmpeg unavailable; MP4 integration test skipped"); return; };
        let dir = directory(); let path = dir.join("screen.mp4");
        let mut recorder = Recorder::start(path.clone(), CaptureFormat::Mp4, test_frame()).unwrap();
        for _ in 0..8 {
            thread::sleep(Duration::from_millis(30));
            recorder.update_audio_notes(&[(1000.0, 96_000)], 0.03, 0.5, 1.0, true).unwrap();
        }
        recorder.finish().unwrap();
        let output = Command::new(&ffmpeg).args(["-v", "error", "-i"]).arg(&path)
            .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"]).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout.len(), 8 * 8 * 3);
        assert!(output.stdout[0] > 200 && output.stdout[1] < 40 && output.stdout[2] < 40);
        let audio = Command::new(&ffmpeg).args(["-v", "error", "-i"]).arg(&path)
            .args(["-map", "0:a:0", "-f", "s16le", "-acodec", "pcm_s16le", "pipe:1"]).output().unwrap();
        assert!(audio.status.success(), "{}", String::from_utf8_lossy(&audio.stderr));
        assert!(audio.stdout.chunks_exact(2).any(|b| i16::from_le_bytes([b[0], b[1]]).unsigned_abs() > 100));
        let probe = ffmpeg.parent().unwrap().join(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" });
        if probe.is_file() {
            let output = Command::new(probe).args(["-v", "error", "-show_streams", "-of", "json"]).arg(&path).output().unwrap();
            assert!(output.status.success());
            let details: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let streams = details["streams"].as_array().unwrap();
            let video = streams.iter().find(|s| s["codec_type"] == "video").unwrap();
            let audio = streams.iter().find(|s| s["codec_type"] == "audio").unwrap();
            let duration = |stream: &serde_json::Value| stream["duration"].as_str().unwrap().parse::<f64>().unwrap();
            assert!((duration(video) - duration(audio)).abs() < 0.05);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn audio_streams_to_disk_and_muted_segments_are_silent() {
        let dir = directory(); let path = dir.join("audio.wav");
        let shared = Arc::new((Mutex::new(State { frame: test_frame(), stop: false, stopped_at: None, audio: VecDeque::from([
            AudioChunk { start: 0, end: 4410, notes: vec![(440.0, 10)], volume: 0.5, pitch: 1.0, enabled: true },
            AudioChunk { start: 4410, end: 8820, notes: vec![(440.0, 10)], volume: 0.5, pitch: 1.0, enabled: false },
        ]) }), Condvar::new()));
        let mut track = AudioTrack::new(&path).unwrap();
        track.drain(&shared).unwrap();
        track.finish(8820).unwrap();
        drop(track);
        let data = std::fs::read(&path).unwrap();
        assert_eq!(&data[..4], b"RIFF");
        assert_eq!(data.len(), 44 + 8820 * 2);
        assert!(data[44..44 + 4410 * 2].iter().any(|b| *b != 0));
        assert!(data[44 + 4410 * 2..].iter().all(|b| *b == 0));
        assert!(shared.0.lock().unwrap().audio.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
