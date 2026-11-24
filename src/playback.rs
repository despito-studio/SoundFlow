//! Native audio output. Network and decoding work never run on the UI thread.
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

const MAX_AUDIO_BYTES: u64 = 256 * 1024 * 1024;
// Keep only short clips (< ~11.8s stereo 44.1 kHz f32) decoded in RAM.
// Ordinary songs stream directly from the disk-backed decoder instead of
// retaining 16+ MiB of PCM in memory.
const MAX_PCM_SAMPLES: usize = 2 * 1024 * 1024 / size_of::<f32>();
/// Fallback initial sample capacity (256 KiB) when container duration is unknown.
const INITIAL_PCM_FALLBACK_SAMPLES: f64 = 65_536.0;
/// One initial attempt plus three retries for transient network failures.
pub const MAX_LOAD_ATTEMPTS: u32 = 4;
const FIRST_RETRY_DELAY: Duration = Duration::from_millis(400);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// A newer load replaced this one; nothing is reported to the user.
    Cancelled,
    /// Transport failure or a broken body read. Retried with backoff.
    Network,
    /// The source exists but cannot be used: 4xx/5xx, empty body, wrong scheme.
    Source,
    /// Bytes arrived but do not decode as audio.
    Decode,
    /// Local file or cache write failure.
    Storage,
}

#[derive(Clone, Debug)]
pub struct AudioFailure {
    pub kind: FailureKind,
    message: String,
}

impl AudioFailure {
    fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    fn cancelled() -> Self {
        Self::new(FailureKind::Cancelled, "Скасовано")
    }
    fn network(message: impl Into<String>) -> Self {
        Self::new(FailureKind::Network, message)
    }
    fn source(message: impl Into<String>) -> Self {
        Self::new(FailureKind::Source, message)
    }
    fn decode(message: impl Into<String>) -> Self {
        Self::new(FailureKind::Decode, message)
    }
    fn storage(message: impl Into<String>) -> Self {
        Self::new(FailureKind::Storage, message)
    }
    pub fn retryable(&self) -> bool {
        matches!(self.kind, FailureKind::Network)
    }
    pub fn is_cancelled(&self) -> bool {
        matches!(self.kind, FailureKind::Cancelled)
    }
    fn into_message(self) -> String {
        self.message
    }
}

/// Backoff between load attempts: 400 ms, 800 ms, 1.6 s, capped at 5 s.
pub fn retry_delay(attempt: u32) -> Duration {
    let factor = 1u32 << attempt.min(16);
    FIRST_RETRY_DELAY
        .saturating_mul(factor)
        .min(MAX_RETRY_DELAY)
}

/// Sleeps before the next attempt, abandoning the wait when a newer load starts.
fn wait_for_retry(attempt: u32, generation: &AtomicU64, expected: u64) -> bool {
    let deadline = Instant::now() + retry_delay(attempt);
    loop {
        if generation.load(Ordering::Acquire) != expected {
            return false;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return true;
        }
        std::thread::sleep(remaining.min(Duration::from_millis(50)));
    }
}

/// Stable cache name for a remote source. Local files bypass the cache.
fn audio_cache_path(cache_dir: &Path, source: &str) -> PathBuf {
    cache_dir.join(format!("{:x}.audio", Sha256::digest(source.as_bytes())))
}

/// A truncated or corrupt download is re-fetched once its cache file is dropped.
/// Returns true when the file was removed, which makes another attempt useful.
fn discard_undecodable(failure: &AudioFailure, source: &str, cache_dir: &Path) -> bool {
    if failure.kind != FailureKind::Decode || !source.starts_with("https://") {
        return false;
    }
    fs::remove_file(audio_cache_path(cache_dir, source)).is_ok()
}

struct PreparedAudio {
    source: Box<dyn Source<Item = f32> + Send>,
    duration: Option<Duration>,
    gain: Option<f32>,
}

fn decode_ahead(
    mut file: File,
    generation: &AtomicU64,
    expected: u64,
) -> Result<PreparedAudio, AudioFailure> {
    let backup = file
        .try_clone()
        .map_err(|e| AudioFailure::storage(format!("Не вдалося відкрити аудіо: {e}")))?;
    let mut decoder = Decoder::try_from(backup)
        .map_err(|e| AudioFailure::decode(format!("Не вдалося відкрити аудіо: {e}")))?;
    let channels = decoder.channels();
    let rate = decoder.sample_rate();
    let duration = decoder.total_duration();
    let estimate = duration.map(|d| d.as_secs_f64() * channels as f64 * rate as f64);
    if estimate.is_some_and(|n| n > MAX_PCM_SAMPLES as f64) {
        return Ok(PreparedAudio {
            source: Box::new(decoder),
            duration,
            gain: None,
        });
    }
    let mut samples = Vec::with_capacity(
        estimate
            .unwrap_or(INITIAL_PCM_FALLBACK_SAMPLES)
            .clamp(0.0, MAX_PCM_SAMPLES as f64) as usize,
    );
    let mut energy = 0.0f64;
    while let Some(sample) = decoder.next() {
        if samples.len() % 16384 == 0 && generation.load(Ordering::Acquire) != expected {
            return Err(AudioFailure::cancelled());
        }
        if samples.len() == MAX_PCM_SAMPLES {
            drop(decoder);
            drop(samples);
            file.seek(SeekFrom::Start(0))
                .map_err(|e| AudioFailure::storage(e.to_string()))?;
            let decoder =
                Decoder::try_from(file).map_err(|e| AudioFailure::decode(e.to_string()))?;
            return Ok(PreparedAudio {
                source: Box::new(decoder),
                duration,
                gain: None,
            });
        }
        if samples.len() == samples.capacity() {
            let wanted = (samples.capacity().max(1) * 2).min(MAX_PCM_SAMPLES);
            samples.reserve_exact(wanted - samples.len());
        }
        energy += (sample as f64) * (sample as f64);
        samples.push(sample);
    }
    if samples.is_empty() {
        return Err(AudioFailure::source("Аудіоджерело не містить звуку"));
    }
    let duration = Duration::from_secs_f64(samples.len() as f64 / channels as f64 / rate as f64);
    let rms = (energy / samples.len() as f64).sqrt().max(1e-5);
    let gain = (0.16 / rms).clamp(0.1, 1.0) as f32;
    Ok(PreparedAudio {
        source: Box::new(rodio::buffer::SamplesBuffer::new(channels, rate, samples)),
        duration: Some(duration),
        gain: Some(gain),
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    pub track: Option<Value>,
    pub playing: bool,
    pub loading: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f32,
    /// Short user-facing note while the loader retries a transient failure.
    pub status: String,
}
impl Default for PlayerState {
    fn default() -> Self {
        Self {
            track: None,
            playing: false,
            loading: false,
            position: 0.0,
            duration: 0.0,
            volume: 0.8,
            status: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ListeningReport {
    pub track: Value,
    pub user_id: String,
    pub session_id: String,
    pub started_at: f64,
    pub heard_seconds: f64,
    pub history_due: bool,
}

#[derive(Clone, Debug)]
pub enum AudioEvent {
    State(PlayerState),
    Finished,
    Error(String),
    Listening {
        track: Value,
        user_id: String,
        session_id: String,
        started_at: f64,
        heard_seconds: f64,
        history_due: bool,
    },
}

enum Command {
    Load {
        generation: u64,
        track: Value,
        source: String,
        start: f64,
        limit: Option<f64>,
    },
    Ready {
        generation: u64,
        audio: Result<PreparedAudio, AudioFailure>,
    },
    /// Emitted by the cache worker before it starts another attempt.
    Retrying {
        generation: u64,
        attempt: u32,
        total: u32,
    },
    Play,
    Pause,
    Toggle,
    Seek(f64),
    Volume(f32),
    Speed(f32),
    Normalize(bool),
    Identity(String),
    Stop,
    Drain(Sender<Option<ListeningReport>>),
    Shutdown,
}
struct LoadRequest {
    generation: u64,
    source: String,
}

#[derive(Clone)]
pub struct AudioPlayer {
    commands: Sender<Command>,
    generation: Arc<AtomicU64>,
    speed: Arc<AtomicU32>,
    normalize: Arc<AtomicBool>,
}
impl AudioPlayer {
    pub fn new() -> (Self, Receiver<AudioEvent>) {
        Self::new_in(std::env::temp_dir().join("SoundFlow-native-audio"))
    }
    pub fn new_in(cache_dir: PathBuf) -> (Self, Receiver<AudioEvent>) {
        let (commands, input) = mpsc::channel();
        let (events, output) = mpsc::channel();
        let (loads, pending) = mpsc::channel::<LoadRequest>();
        let generation = Arc::new(AtomicU64::new(0));
        let speed = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let normalize = Arc::new(AtomicBool::new(true));
        let speed_handle = speed.clone();
        let normalize_handle = normalize.clone();
        let loader_commands = commands.clone();
        let loader_generation = generation.clone();
        std::thread::Builder::new()
            .name("soundflow-audio-cache".into())
            .spawn(move || {
                while let Ok(mut request) = pending.recv() {
                    while let Ok(newer) = pending.try_recv() {
                        request = newer;
                    }
                    if loader_generation.load(Ordering::Acquire) != request.generation {
                        continue;
                    }
                    let mut attempt = 1;
                    let audio = loop {
                        let prepared = prepare_audio(
                            &request.source,
                            &cache_dir,
                            &loader_generation,
                            request.generation,
                        )
                        .and_then(|file| {
                            decode_ahead(file, &loader_generation, request.generation)
                        });
                        match prepared {
                            Ok(audio) => break Ok(audio),
                            Err(failure) => {
                                // A corrupt or truncated cache entry is dropped even on
                                // the final attempt, so the next play re-downloads it.
                                let cleared =
                                    discard_undecodable(&failure, &request.source, &cache_dir);
                                let retry = attempt < MAX_LOAD_ATTEMPTS
                                    && (failure.retryable() || cleared);
                                if !retry {
                                    break Err(failure);
                                }
                                let _ = loader_commands.send(Command::Retrying {
                                    generation: request.generation,
                                    attempt: attempt + 1,
                                    total: MAX_LOAD_ATTEMPTS,
                                });
                                if !wait_for_retry(
                                    attempt.saturating_sub(1),
                                    &loader_generation,
                                    request.generation,
                                ) {
                                    break Err(AudioFailure::cancelled());
                                }
                                attempt += 1;
                            }
                        }
                    };
                    let _ = loader_commands.send(Command::Ready {
                        generation: request.generation,
                        audio,
                    });
                }
            })
            .expect("Start native audio cache worker");
        let actor_generation = generation.clone();
        std::thread::Builder::new()
            .name("soundflow-audio".into())
            .spawn(move || {
                audio_loop(
                    input,
                    events,
                    loads,
                    actor_generation,
                    speed_handle.clone(),
                    normalize_handle.clone(),
                );
            })
            .expect("Start native audio worker");
        (
            Self {
                commands,
                generation,
                speed,
                normalize,
            },
            output,
        )
    }
    pub fn load(&self, track: Value, source: String, start: f64, limit: Option<f64>) {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let start = if start.is_finite() {
            start.max(0.0)
        } else {
            0.0
        };
        let limit = limit.filter(|value| value.is_finite() && *value > 0.0);
        let _ = self.commands.send(Command::Load {
            generation,
            track,
            source,
            start,
            limit,
        });
    }
    pub fn play(&self) {
        let _ = self.commands.send(Command::Play);
    }
    pub fn pause(&self) {
        let _ = self.commands.send(Command::Pause);
    }
    pub fn toggle(&self) {
        let _ = self.commands.send(Command::Toggle);
    }
    pub fn seek(&self, position: f64) {
        if position.is_finite() {
            let _ = self.commands.send(Command::Seek(position.max(0.0)));
        }
    }
    pub fn set_volume(&self, volume: f32) {
        if volume.is_finite() {
            let _ = self.commands.send(Command::Volume(volume.clamp(0.0, 1.0)));
        }
    }
    pub fn set_speed(&self, speed: f32) {
        let speed = speed.clamp(0.5, 2.0);
        self.speed.store(speed.to_bits(), Ordering::Relaxed);
        let _ = self.commands.send(Command::Speed(speed));
    }
    pub fn set_normalize(&self, enabled: bool) {
        self.normalize.store(enabled, Ordering::Relaxed);
        let _ = self.commands.send(Command::Normalize(enabled));
    }
    pub fn set_identity(&self, user_id: String) {
        let _ = self.commands.send(Command::Identity(user_id));
    }
    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        let _ = self.commands.send(Command::Stop);
    }
    pub fn drain(&self) -> Option<ListeningReport> {
        let (sender, receiver) = mpsc::channel();
        self.commands.send(Command::Drain(sender)).ok()?;
        receiver.recv_timeout(Duration::from_secs(2)).ok().flatten()
    }
    pub fn shutdown(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        let _ = self.commands.send(Command::Shutdown);
    }
}

// PCM gain is measured in the loader; long streams retain a bounded rolling envelope.
struct NormalizedSource<S> {
    inner: S,
    enabled: Arc<AtomicBool>,
    fixed_gain: Option<f32>,
    rms: f32,
    gain: f32,
}

impl<S> NormalizedSource<S> {
    fn new(inner: S, enabled: Arc<AtomicBool>, fixed_gain: Option<f32>) -> Self {
        Self {
            inner,
            enabled,
            fixed_gain,
            rms: 0.02,
            gain: 1.0,
        }
    }
}

impl<S: Source<Item = f32>> Iterator for NormalizedSource<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        if !self.enabled.load(Ordering::Relaxed) {
            return Some(sample);
        }
        if let Some(gain) = self.fixed_gain {
            return Some(sample * gain);
        }
        self.rms = self.rms * 0.9992 + sample * sample * 0.0008;
        let rms = self.rms.sqrt().max(1e-5);
        let target = (0.16 / rms).clamp(0.1, 1.0);
        self.gain += (target - self.gain) * 0.0008;
        Some((sample * self.gain).clamp(-1.0, 1.0))
    }
}

impl<S: Source<Item = f32>> Source for NormalizedSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }
    fn channels(&self) -> rodio::ChannelCount {
        self.inner.channels()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(position)?;
        self.rms = 0.02;
        self.gain = 1.0;
        Ok(())
    }
}

fn prepare_audio(
    source: &str,
    cache_dir: &Path,
    generation: &AtomicU64,
    expected: u64,
) -> Result<File, AudioFailure> {
    if generation.load(Ordering::Acquire) != expected {
        return Err(AudioFailure::cancelled());
    }
    if !source.starts_with("https://") {
        if source.contains("://") {
            return Err(AudioFailure::source("Непідтримуване аудіоджерело"));
        }
        return File::open(source)
            .map_err(|_| AudioFailure::source("Не вдалося відкрити локальне аудіо"));
    }
    fs::create_dir_all(cache_dir)
        .map_err(|_| AudioFailure::storage("Не вдалося створити аудіокеш"))?;
    // Single source of truth for the cache name: the retry path drops the same
    // file when it cannot be decoded.
    let complete = audio_cache_path(cache_dir, source);
    if complete.is_file() {
        return File::open(&complete).map_err(|_| AudioFailure::storage("Аудіокеш недоступний"));
    }
    let partial = cache_dir.join(format!(
        "{}.{}.partial",
        complete
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("audio"),
        expected
    ));
    let result = (|| {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|_| AudioFailure::network("Не вдалося підготувати аудіозапит"))?;
        let mut response = client
            .get(source)
            .header("User-Agent", "Mozilla/5.0 SoundFlow/3.2")
            .send()
            .map_err(|error| {
                if error.is_timeout() || error.is_connect() || error.is_request() {
                    AudioFailure::network(
                        "Аудіоджерело не відповідає. Спробуй іншу версію треку.",
                    )
                } else {
                    AudioFailure::source(
                        "Аудіоджерело недоступне. Спробуй іншу версію треку.",
                    )
                }
            })?
            .error_for_status()
            .map_err(|_| {
                AudioFailure::source("Аудіоджерело недоступне. Спробуй іншу версію треку.")
            })?;
        if response.url().scheme() != "https"
            || response
                .content_length()
                .is_some_and(|size| size > MAX_AUDIO_BYTES)
        {
            return Err(AudioFailure::source("Аудіоджерело недоступне або завелике"));
        }
        let mut file = File::create(&partial)
            .map_err(|_| AudioFailure::storage("Не вдалося зберегти аудіокеш"))?;
        let mut buffer = [0u8; 64 * 1024];
        let mut bytes = 0u64;
        loop {
            if generation.load(Ordering::Acquire) != expected {
                return Err(AudioFailure::cancelled());
            }
            let count = response.read(&mut buffer).map_err(|_| {
                AudioFailure::network("З’єднання з аудіоджерелом перервано")
            })?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            if bytes > MAX_AUDIO_BYTES {
                return Err(AudioFailure::source("Аудіофайл перевищує розмір кешу"));
            }
            file.write_all(&buffer[..count])
                .map_err(|_| AudioFailure::storage("Не вдалося записати аудіокеш"))?;
        }
        if bytes == 0 {
            return Err(AudioFailure::source("Аудіоджерело порожнє"));
        }
        file.sync_all()
            .map_err(|_| AudioFailure::storage("Не вдалося завершити аудіокеш"))?;
        drop(file);
        fs::rename(&partial, &complete)
            .map_err(|_| AudioFailure::storage("Не вдалося завершити аудіокеш"))?;
        prune_audio_cache(cache_dir, &complete);
        File::open(&complete).map_err(|_| AudioFailure::storage("Не вдалося відкрити аудіокеш"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

fn prune_audio_cache(folder: &Path, keep: &Path) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|e| e == "audio"))
        .filter_map(|entry| {
            entry
                .metadata()
                .ok()
                .map(|m| (entry.path(), m.modified().ok(), m.len()))
        })
        .collect();
    files.sort_by_key(|(_, modified, _)| *modified);
    let mut bytes: u64 = files.iter().map(|(_, _, size)| size).sum();
    let mut count = files.len();
    for (path, _, size) in files {
        if count <= 8 && bytes <= 512 * 1024 * 1024 {
            break;
        }
        if path != keep && fs::remove_file(&path).is_ok() {
            count -= 1;
            bytes = bytes.saturating_sub(size);
        }
    }
}

struct Session {
    report: ListeningReport,
    last_sent: f64,
    history_sent: bool,
}
fn emit_listening(session: &mut Option<Session>, events: &Sender<AudioEvent>) {
    if let Some(session) = session {
        if session.report.heard_seconds >= 1.0 && session.report.heard_seconds > session.last_sent {
            let history_due = session.report.heard_seconds >= 10.0 && !session.history_sent;
            let report = &session.report;
            let _ = events.send(AudioEvent::Listening {
                track: report.track.clone(),
                user_id: report.user_id.clone(),
                session_id: report.session_id.clone(),
                started_at: report.started_at,
                heard_seconds: report.heard_seconds,
                history_due,
            });
            session.last_sent = report.heard_seconds;
            session.history_sent |= history_due;
        }
    }
}

pub fn track_duration(track: &Value) -> f64 {
    if let Some(v) = track.get("duration") {
        if let Some(n) = v.as_f64() {
            if n > 0.0 {
                return if n > 10_000.0 { n / 1000.0 } else { n };
            }
        }
        if let Some(s) = v.as_str() {
            if let Ok(n) = s.parse::<f64>() {
                if n > 0.0 {
                    return if n > 10_000.0 { n / 1000.0 } else { n };
                }
            }
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(m), Ok(sec)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                    return m * 60.0 + sec;
                }
            } else if parts.len() == 3 {
                if let (Ok(h), Ok(m), Ok(sec)) = (
                    parts[0].parse::<f64>(),
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                ) {
                    return h * 3600.0 + m * 60.0 + sec;
                }
            }
        }
    }
    if let Some(s) = track.get("duration_formatted").and_then(|v| v.as_str()) {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(m), Ok(sec)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                return m * 60.0 + sec;
            }
        }
    }
    0.0
}

fn audio_loop(
    input: Receiver<Command>,
    events: Sender<AudioEvent>,
    loads: Sender<LoadRequest>,
    generation: Arc<AtomicU64>,
    speed_store: Arc<AtomicU32>,
    normalize_store: Arc<AtomicBool>,
) {
    let mut state = PlayerState::default();
    let mut stream: Option<OutputStream> = None;
    let mut sink: Option<Sink> = None;
    let mut session: Option<Session> = None;
    let mut user_id = "guest".to_string();
    let mut wanted_playing = false;
    let mut current_generation = 0;
    let mut start = 0.0;
    let mut limit: Option<f64> = None;
    let mut clock = Instant::now();
    let mut last_position = 0.0;
    let mut heartbeat = Instant::now();
    let mut speed = f32::from_bits(speed_store.load(Ordering::Relaxed));
    let mut last_emitted: Option<PlayerState> = None;
    loop {
        let poll_wait = if state.playing || state.loading || wanted_playing {
            Duration::from_millis(200)
        } else {
            Duration::from_millis(1000)
        };
        let command = input.recv_timeout(poll_wait);
        let elapsed = clock.elapsed().as_secs_f64();
        clock = Instant::now();
        if let Some(output) = sink.as_ref() {
            let position = output.get_pos().as_secs_f64();
            if let Some(session) = session.as_mut() {
                session.report.heard_seconds += audible_delta(
                    state.playing && !output.empty(),
                    state.volume,
                    position - last_position,
                    elapsed,
                );
                session.report.history_due = session.report.heard_seconds >= 10.0;
            }
            state.position = position;
            last_position = position;
            if wanted_playing
                && (output.empty() || limit.is_some_and(|seconds| position >= start + seconds))
            {
                output.pause();
                state.playing = false;
                wanted_playing = false;
                emit_listening(&mut session, &events);
                let _ = events.send(AudioEvent::Finished);
            }
        }
        match command {
            Ok(Command::Load {
                generation: id,
                track,
                source,
                start: position,
                limit: preview,
            }) => {
                emit_listening(&mut session, &events);
                sink.take();
                current_generation = id;
                start = position;
                limit = preview
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .map(|v| v.min(30.0));
                let parsed_duration = track_duration(&track);
                state.track = Some(track.clone());
                state.position = start;
                state.duration = if parsed_duration > 0.0 {
                    parsed_duration
                } else {
                    0.0
                };
                state.playing = false;
                state.loading = true;
                state.status.clear();
                wanted_playing = true;
                session = Some(Session {
                    report: ListeningReport {
                        track,
                        user_id: user_id.clone(),
                        session_id: uuid::Uuid::new_v4().to_string(),
                        started_at: crate::library::now(),
                        heard_seconds: 0.0,
                        history_due: false,
                    },
                    last_sent: 0.0,
                    history_sent: false,
                });
                let _ = loads.send(LoadRequest {
                    generation: id,
                    source,
                });
            }
            Ok(Command::Retrying {
                generation: id,
                attempt,
                total,
            }) if id == current_generation && generation.load(Ordering::Acquire) == id => {
                state.loading = true;
                state.status = format!("Відновлюємо з’єднання. Спроба {attempt} з {total}");
            }
            Ok(Command::Retrying { .. }) => {}
            Ok(Command::Ready {
                generation: id,
                audio,
            }) if id == current_generation && generation.load(Ordering::Acquire) == id => {
                match audio {
                    Ok(prepared) => {
                        let mut duration = prepared
                            .duration
                            .map(|d| d.as_secs_f64())
                            .filter(|&d| d > 0.5)
                            .unwrap_or(state.duration);
                        if duration <= 0.5 {
                            duration = state.track.as_ref().map(track_duration).unwrap_or(0.0);
                        }
                        if stream.is_none() {
                            match OutputStreamBuilder::open_default_stream() {
                                Ok(mut device) => {
                                    device.log_on_drop(false);
                                    stream = Some(device);
                                }
                                Err(_) => {
                                    state.loading = false;
                                    state.status.clear();
                                    wanted_playing = false;
                                    let _ = events.send(AudioEvent::Error(
                                        "Немає доступного пристрою відтворення аудіо".into(),
                                    ));
                                }
                            }
                        }
                        if let Some(device) = stream.as_ref() {
                            let output = Sink::connect_new(device.mixer());
                            output.pause();
                            output.set_volume(volume_gain(state.volume));
                            output.set_speed(speed);
                            output.append(NormalizedSource::new(
                                prepared.source,
                                normalize_store.clone(),
                                prepared.gain,
                            ));
                            state.duration = duration;
                            if duration > 0.0 {
                                if let Some(track) = state.track.as_mut() {
                                    track["duration"] = serde_json::json!(duration);
                                }
                                if let Some(session) = session.as_mut() {
                                    session.report.track["duration"] = serde_json::json!(duration);
                                }
                            }
                            start = start.min((duration - 0.1).max(0.0));
                            if start > 0.0 {
                                if output.try_seek(Duration::from_secs_f64(start)).is_err() {
                                    start = 0.0;
                                }
                            }
                            state.position = start;
                            last_position = start;
                            clock = Instant::now();
                            heartbeat = Instant::now();
                            if wanted_playing {
                                output.play();
                            }
                            state.playing = wanted_playing;
                            state.loading = false;
                            state.status.clear();
                            sink = Some(output);
                        }
                    }
                    Err(failure) => {
                        state.loading = false;
                        state.playing = false;
                        state.status.clear();
                        wanted_playing = false;
                        // A superseded load is not an error the listener asked about.
                        if !failure.is_cancelled() {
                            let _ = events.send(AudioEvent::Error(failure.into_message()));
                        }
                    }
                }
            }
            Ok(Command::Ready { .. }) => {}
            Ok(Command::Play) => {
                wanted_playing = true;
                if let Some(sink) = &sink {
                    sink.play();
                    state.playing = true;
                }
            }
            Ok(Command::Pause) => {
                wanted_playing = false;
                state.playing = false;
                if let Some(sink) = &sink {
                    sink.pause();
                }
                emit_listening(&mut session, &events);
            }
            Ok(Command::Toggle) => {
                wanted_playing = !wanted_playing;
                if let Some(sink) = &sink {
                    if wanted_playing {
                        sink.play();
                    } else {
                        sink.pause();
                    }
                    state.playing = wanted_playing;
                }
                if !wanted_playing {
                    emit_listening(&mut session, &events);
                }
            }
            Ok(Command::Seek(position)) => {
                if let Some(sink) = &sink {
                    let end = limit
                        .map(|seconds| start + seconds)
                        .unwrap_or(state.duration)
                        .max(0.0);
                    let target_pos = if end > 0.0 {
                        position.clamp(0.0, end)
                    } else {
                        position.max(0.0)
                    };
                    match sink.try_seek(Duration::from_secs_f64(target_pos)) {
                        Ok(()) => {
                            state.position = target_pos;
                            last_position = target_pos;
                            clock = Instant::now();
                            let _ = events.send(AudioEvent::State(state.clone()));
                        }
                        Err(error) => {
                            let _ = events.send(AudioEvent::Error(format!(
                                "Не вдалося перемотати трек: {error}"
                            )));
                        }
                    }
                } else if state.loading && state.track.is_some() {
                    start = if state.duration > 0.0 {
                        position.clamp(0.0, (state.duration - 0.01).max(0.0))
                    } else {
                        position.max(0.0)
                    };
                    state.position = start;
                    last_position = start;
                    let _ = events.send(AudioEvent::State(state.clone()));
                }
            }
            Ok(Command::Speed(value)) => {
                speed = value.clamp(0.5, 2.0);
                speed_store.store(speed.to_bits(), Ordering::Relaxed);
                if let Some(sink) = &sink {
                    sink.set_speed(speed);
                }
            }
            Ok(Command::Normalize(enabled)) => {
                normalize_store.store(enabled, Ordering::Relaxed);
            }
            Ok(Command::Volume(volume)) => {
                state.volume = volume;
                if let Some(sink) = &sink {
                    sink.set_volume(volume_gain(volume));
                }
            }
            Ok(Command::Identity(user)) => {
                user_id = user;
            }
            Ok(Command::Drain(reply)) => {
                wanted_playing = false;
                state.playing = false;
                if let Some(sink) = &sink {
                    sink.pause();
                }
                let report = session.as_mut().map(|session| {
                    session.last_sent = session.report.heard_seconds;
                    session.history_sent |= session.report.history_due;
                    session.report.clone()
                });
                let _ = reply.send(report);
            }
            Ok(Command::Stop) => {
                emit_listening(&mut session, &events);
                sink.take();
                session = None;
                state.track = None;
                state.playing = false;
                state.loading = false;
                state.status.clear();
                state.position = 0.0;
                state.duration = 0.0;
                wanted_playing = false;
            }
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                emit_listening(&mut session, &events);
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if heartbeat.elapsed() >= Duration::from_secs(10) {
            emit_listening(&mut session, &events);
            heartbeat = Instant::now();
        }
        if last_emitted.as_ref() != Some(&state) {
            last_emitted = Some(state.clone());
            if events.send(AudioEvent::State(state.clone())).is_err() {
                break;
            }
        }
    }
}

// UI percentages describe perceived loudness; the sink accepts linear amplitude.
fn volume_gain(volume: f32) -> f32 {
    volume.clamp(0.0, 1.0).powi(2)
}

fn audible_delta(playing: bool, volume: f32, delta: f64, elapsed: f64) -> f64 {
    if playing && volume > 0.0001 && delta > 0.0 && delta <= elapsed * 1.5 + 0.15 {
        delta.min(elapsed * 1.2 + 0.02)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A real PCM WAV exercises both preparation paths without an audio device.
    fn wav(seconds: u32) -> File {
        let mut file = tempfile::tempfile().unwrap();
        let bytes = seconds * 44100 * 2 * 2;
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + bytes).to_le_bytes()).unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16u32.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap();
        file.write_all(&2u16.to_le_bytes()).unwrap();
        file.write_all(&44100u32.to_le_bytes()).unwrap();
        file.write_all(&176400u32.to_le_bytes()).unwrap();
        file.write_all(&4u16.to_le_bytes()).unwrap();
        file.write_all(&16u16.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&bytes.to_le_bytes()).unwrap();
        file.set_len(44 + bytes as u64).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file
    }

    #[test]
    fn ordinary_songs_remain_file_backed_and_seekable() {
        let mut audio = decode_ahead(wav(180), &AtomicU64::new(1), 1).unwrap();
        assert!(audio.gain.is_none(), "long songs must not retain a full PCM buffer");
        assert_eq!(audio.source.channels(), 2);
        assert_eq!(audio.source.sample_rate(), 44100);
        audio.source.try_seek(Duration::from_secs(42)).unwrap();
        assert!(audio.source.next().is_some());
    }

    #[test]
    fn short_clips_keep_precomputed_normalization() {
        let audio = decode_ahead(wav(1), &AtomicU64::new(1), 1).unwrap();
        assert!(audio.gain.is_some());
        assert_eq!(audio.duration, Some(Duration::from_secs(1)));
    }

    #[test]
    fn retry_delay_grows_then_caps() {
        assert_eq!(retry_delay(0), Duration::from_millis(400));
        assert_eq!(retry_delay(1), Duration::from_millis(800));
        assert_eq!(retry_delay(2), Duration::from_millis(1600));
        assert_eq!(retry_delay(3), Duration::from_millis(3200));
        assert_eq!(retry_delay(4), MAX_RETRY_DELAY);
        assert_eq!(retry_delay(u32::MAX), MAX_RETRY_DELAY);
    }

    #[test]
    fn only_transport_failures_are_retried() {
        assert!(AudioFailure::network("timeout").retryable());
        assert!(!AudioFailure::source("404").retryable());
        assert!(!AudioFailure::decode("bad container").retryable());
        assert!(!AudioFailure::storage("no space").retryable());
        assert!(!AudioFailure::cancelled().retryable());
        assert!(AudioFailure::cancelled().is_cancelled());
    }

    #[test]
    fn retry_wait_aborts_on_new_generation() {
        let generation = AtomicU64::new(1);
        assert!(!wait_for_retry(3, &generation, 0));
        assert!(wait_for_retry(0, &generation, 1));
    }

    #[test]
    fn attempt_budget_covers_three_retries() {
        assert_eq!(MAX_LOAD_ATTEMPTS, 4);
    }
}
