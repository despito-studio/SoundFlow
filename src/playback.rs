//! Native audio output. Network and decoding work never run on the UI thread.
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

const MAX_AUDIO_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct PlayerState {
    pub track: Option<Value>,
    pub playing: bool,
    pub loading: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f32,
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
        file: Result<File, String>,
    },
    Play,
    Pause,
    Toggle,
    Seek(f64),
    Volume(f32),
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
                    let file = prepare_audio(
                        &request.source,
                        &cache_dir,
                        &loader_generation,
                        request.generation,
                    );
                    let _ = loader_commands.send(Command::Ready {
                        generation: request.generation,
                        file,
                    });
                }
            })
            .expect("Start native audio cache worker");
        let actor_generation = generation.clone();
        std::thread::Builder::new()
            .name("soundflow-audio".into())
            .spawn(move || {
                audio_loop(input, events, loads, actor_generation);
            })
            .expect("Start native audio worker");
        (
            Self {
                commands,
                generation,
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

fn prepare_audio(
    source: &str,
    cache_dir: &Path,
    generation: &AtomicU64,
    expected: u64,
) -> Result<File, String> {
    if generation.load(Ordering::Acquire) != expected {
        return Err("Скасовано".into());
    }
    if !source.starts_with("https://") {
        if source.contains("://") {
            return Err("Непідтримуване аудіоджерело".into());
        }
        return File::open(source).map_err(|_| "Не вдалося відкрити локальне аудіо".into());
    }
    fs::create_dir_all(cache_dir).map_err(|_| "Не вдалося створити аудіокеш".to_string())?;
    let name = format!("{:x}.audio", Sha256::digest(source.as_bytes()));
    let complete = cache_dir.join(&name);
    if complete.is_file() {
        return File::open(complete).map_err(|_| "Аудіокеш недоступний".into());
    }
    let partial = cache_dir.join(format!("{name}.{expected}.partial"));
    let result = (|| {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|_| "Не вдалося підготувати аудіозапит".to_string())?;
        let mut response = client
            .get(source)
            .header("User-Agent", "Mozilla/5.0 SoundFlow/3.2")
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|_| "Аудіоджерело недоступне. Спробуй іншу версію треку.".to_string())?;
        if response.url().scheme() != "https"
            || response
                .content_length()
                .is_some_and(|size| size > MAX_AUDIO_BYTES)
        {
            return Err("Аудіоджерело недоступне або завелике".into());
        }
        let mut file =
            File::create(&partial).map_err(|_| "Не вдалося зберегти аудіокеш".to_string())?;
        let mut buffer = [0u8; 64 * 1024];
        let mut bytes = 0u64;
        loop {
            if generation.load(Ordering::Acquire) != expected {
                return Err("Скасовано".into());
            }
            let count = response
                .read(&mut buffer)
                .map_err(|_| "З’єднання з аудіоджерелом перервано".to_string())?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            if bytes > MAX_AUDIO_BYTES {
                return Err("Аудіофайл перевищує розмір кешу".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|_| "Не вдалося записати аудіокеш".to_string())?;
        }
        if bytes == 0 {
            return Err("Аудіоджерело порожнє".into());
        }
        file.sync_all()
            .map_err(|_| "Не вдалося завершити аудіокеш".to_string())?;
        drop(file);
        fs::rename(&partial, &complete).map_err(|_| "Не вдалося завершити аудіокеш".to_string())?;
        prune_audio_cache(cache_dir, &complete);
        File::open(&complete).map_err(|_| "Не вдалося відкрити аудіокеш".into())
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

fn audio_loop(
    input: Receiver<Command>,
    events: Sender<AudioEvent>,
    loads: Sender<LoadRequest>,
    generation: Arc<AtomicU64>,
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
    loop {
        let command = input.recv_timeout(Duration::from_millis(200));
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
                state.track = Some(track.clone());
                state.position = start;
                state.duration = track["duration"].as_f64().unwrap_or(0.0);
                state.playing = false;
                state.loading = true;
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
            Ok(Command::Ready {
                generation: id,
                file,
            }) if id == current_generation && generation.load(Ordering::Acquire) == id => {
                let loaded = file.and_then(|file| Decoder::try_from(file).map_err(|_| "Формат аудіо не підтримується. Обери M4A, MP3, FLAC, WAV або Ogg Vorbis.".to_string()));
                match loaded {
                    Ok(decoder) => {
                        let duration = decoder
                            .total_duration()
                            .map(|d| d.as_secs_f64())
                            .unwrap_or(state.duration);
                        if stream.is_none() {
                            match OutputStreamBuilder::open_default_stream() {
                                Ok(mut device) => {
                                    device.log_on_drop(false);
                                    stream = Some(device);
                                }
                                Err(_) => {
                                    state.loading = false;
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
                            output.set_volume(state.volume);
                            output.append(decoder);
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
                            sink = Some(output);
                        }
                    }
                    Err(error) => {
                        state.loading = false;
                        state.playing = false;
                        wanted_playing = false;
                        let _ = events.send(AudioEvent::Error(error));
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
                    let position = position.min(end);
                    if sink.try_seek(Duration::from_secs_f64(position)).is_ok() {
                        state.position = position;
                        last_position = position;
                        clock = Instant::now();
                    }
                }
            }
            Ok(Command::Volume(volume)) => {
                state.volume = volume;
                if let Some(sink) = &sink {
                    sink.set_volume(volume);
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
        if events.send(AudioEvent::State(state.clone())).is_err() {
            break;
        }
    }
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
    #[test]
    fn paused_muted_and_seek_time_are_not_listening() {
        assert_eq!(audible_delta(false, 1.0, 1.0, 1.0), 0.0);
        assert_eq!(audible_delta(true, 0.0, 1.0, 1.0), 0.0);
        assert_eq!(audible_delta(true, 1.0, 50.0, 0.2), 0.0);
        assert_eq!(audible_delta(true, 1.0, 0.0, 0.2), 0.0);
        assert!((audible_delta(true, 0.8, 0.2, 0.2) - 0.2).abs() < 0.001);
    }
    #[test]
    fn older_load_generation_is_cancelled_before_writing() {
        let folder = tempfile::tempdir().unwrap();
        let generation = AtomicU64::new(2);
        assert_eq!(
            prepare_audio(
                "https://127.0.0.1:1/cancelled",
                folder.path(),
                &generation,
                1
            )
            .unwrap_err(),
            "Скасовано"
        );
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 0);
    }
}
