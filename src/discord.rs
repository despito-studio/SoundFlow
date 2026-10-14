//! Discord Rich Presence integration via local IPC named pipe.
use serde_json::{json, Value};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const DEFAULT_CLIENT_ID: &str = "1177081335727267940";

#[derive(Clone, Debug)]
pub struct TrackActivity {
    pub title: String,
    pub artist: String,
    pub thumbnail: String,
    pub duration: f64,
    pub position: f64,
    pub playing: bool,
}

enum DiscordCommand {
    Update(Option<TrackActivity>),
    SetEnabled(bool),
    Shutdown,
}

#[derive(Clone)]
pub struct DiscordRpc {
    sender: Sender<DiscordCommand>,
    enabled: Arc<AtomicBool>,
}

impl DiscordRpc {
    pub fn new(enabled: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        let enabled_flag = Arc::new(AtomicBool::new(enabled));
        let flag_clone = enabled_flag.clone();

        thread::spawn(move || {
            worker_loop(rx, flag_clone);
        });

        Self {
            sender: tx,
            enabled: enabled_flag,
        }
    }

    pub fn update(&self, track: Option<&Value>, playing: bool, position: f64, duration: f64) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let activity = track.and_then(|t| {
            let title = t.get("title").and_then(Value::as_str)?.trim();
            if title.is_empty() {
                return None;
            }
            let artist = t.get("artist").and_then(Value::as_str).unwrap_or("SoundFlow").trim();
            let thumbnail = t.get("thumbnail").and_then(Value::as_str).unwrap_or("").trim();
            Some(TrackActivity {
                title: title.to_string(),
                artist: if artist.is_empty() { "SoundFlow".into() } else { artist.to_string() },
                thumbnail: thumbnail.to_string(),
                duration,
                position,
                playing,
            })
        });

        let _ = self.sender.send(DiscordCommand::Update(activity));
    }

    pub fn clear(&self) {
        let _ = self.sender.send(DiscordCommand::Update(None));
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        let _ = self.sender.send(DiscordCommand::SetEnabled(enabled));
    }

    pub fn shutdown(&self) {
        let _ = self.sender.send(DiscordCommand::Shutdown);
    }
}

fn worker_loop(rx: Receiver<DiscordCommand>, enabled_flag: Arc<AtomicBool>) {
    let mut pipe: Option<File> = None;
    let mut current_activity: Option<TrackActivity> = None;
    let mut last_connect_attempt = Instant::now() - Duration::from_secs(10);
    let mut last_sent_activity: Option<String> = None;

    loop {
        // Wait for next command or timeout to attempt reconnect if needed
        let timeout = if pipe.is_none() && current_activity.is_some() && enabled_flag.load(Ordering::Relaxed) {
            Duration::from_secs(3)
        } else {
            Duration::from_secs(15)
        };

        match rx.recv_timeout(timeout) {
            Ok(DiscordCommand::Shutdown) => {
                if let Some(mut p) = pipe.take() {
                    let _ = send_activity(&mut p, None);
                }
                break;
            }
            Ok(DiscordCommand::SetEnabled(enabled)) => {
                if !enabled {
                    if let Some(ref mut p) = pipe {
                        let _ = send_activity(p, None);
                    }
                    last_sent_activity = None;
                } else if current_activity.is_some() {
                    last_sent_activity = None; // force resend
                }
            }
            Ok(DiscordCommand::Update(activity)) => {
                current_activity = activity;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Periodic heartbeat or reconnect check
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if !enabled_flag.load(Ordering::Relaxed) {
            continue;
        }

        // Try connect if not connected and we have something to show
        if pipe.is_none() && current_activity.is_some() {
            if last_connect_attempt.elapsed() >= Duration::from_secs(3) {
                last_connect_attempt = Instant::now();
                pipe = try_connect_discord();
                if pipe.is_some() {
                    last_sent_activity = None; // force send after reconnect
                }
            }
        }

        // Send activity update if pipe is open
        if let Some(ref mut p) = pipe {
            let activity_key = current_activity.as_ref().map(|a| {
                format!("{}:{}:{}:{:.0}", a.title, a.artist, a.playing, a.position)
            });

            if activity_key != last_sent_activity {
                if let Err(_) = send_activity(p, current_activity.as_ref()) {
                    // Pipe failed, reset connection
                    pipe = None;
                    last_sent_activity = None;
                } else {
                    last_sent_activity = activity_key;
                }
            }
        }
    }
}

fn try_connect_discord() -> Option<File> {
    for i in 0..10 {
        let path = format!(r"\\.\pipe\discord-ipc-{i}");
        if let Ok(mut f) = OpenOptions::new().read(true).write(true).open(&path) {
            // Handshake (Opcode 0)
            let handshake = json!({
                "v": 1,
                "client_id": DEFAULT_CLIENT_ID
            }).to_string();

            if send_frame(&mut f, 0, handshake.as_bytes()).is_err() {
                continue;
            }

            // Read response
            if let Ok((op, _body)) = read_frame(&mut f) {
                if op == 1 {
                    return Some(f);
                }
            }
        }
    }
    None
}

fn send_activity(pipe: &mut File, activity: Option<&TrackActivity>) -> Result<(), std::io::Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let activity_json = if let Some(act) = activity {
        let mut obj = serde_json::Map::new();
        obj.insert("type".into(), json!(2)); // 2 = Listening to
        obj.insert("details".into(), json!(truncate_str(&act.title, 128)));

        let state_text = if act.playing {
            act.artist.clone()
        } else {
            format!("{} (Пауза)", act.artist)
        };
        obj.insert("state".into(), json!(truncate_str(&state_text, 128)));

        if act.playing && act.duration > 0.5 {
            let start_ts = (now as f64 - act.position.max(0.0)).max(0.0) as u64;
            let end_ts = (start_ts as f64 + act.duration) as u64;
            obj.insert(
                "timestamps".into(),
                json!({
                    "start": start_ts,
                    "end": end_ts
                }),
            );
        } else if act.playing {
            let start_ts = (now as f64 - act.position.max(0.0)).max(0.0) as u64;
            obj.insert(
                "timestamps".into(),
                json!({
                    "start": start_ts
                }),
            );
        }

        let mut assets = serde_json::Map::new();
        if act.thumbnail.starts_with("http://") || act.thumbnail.starts_with("https://") {
            assets.insert("large_image".into(), json!(act.thumbnail));
        } else {
            assets.insert("large_image".into(), json!("music"));
        }
        assets.insert("large_text".into(), json!("SoundFlow"));
        assets.insert(
            "small_image".into(),
            json!(if act.playing { "play" } else { "pause" }),
        );
        assets.insert(
            "small_text".into(),
            json!(if act.playing { "Відтворюється" } else { "На паузі" }),
        );
        obj.insert("assets".into(), Value::Object(assets));

        obj.insert(
            "buttons".into(),
            json!([
                {
                    "label": "Слухати в SoundFlow",
                    "url": "https://github.com/feodaI/SoundFlow"
                }
            ]),
        );

        Value::Object(obj)
    } else {
        Value::Null
    };

    let payload = json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": activity_json
        },
        "nonce": uuid::Uuid::new_v4().to_string()
    }).to_string();

    send_frame(pipe, 1, payload.as_bytes())?;

    // Drain reply
    let _ = read_frame(pipe);
    Ok(())
}

fn send_frame(pipe: &mut File, opcode: u32, payload: &[u8]) -> Result<(), std::io::Error> {
    let mut header = Vec::with_capacity(8 + payload.len());
    header.extend_from_slice(&opcode.to_le_bytes());
    header.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    header.extend_from_slice(payload);
    pipe.write_all(&header)?;
    pipe.flush()?;
    Ok(())
}

fn read_frame(pipe: &mut File) -> Result<(u32, Vec<u8>), std::io::Error> {
    let mut header = [0u8; 8];
    pipe.read_exact(&mut header)?;
    let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
    let length = u32::from_le_bytes(header[4..8].try_into().unwrap());
    if length > 65536 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Discord frame too large",
        ));
    }
    let mut body = vec![0u8; length as usize];
    pipe.read_exact(&mut body)?;
    Ok((opcode, body))
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let mut out: String = s.chars().take(max_chars).collect();
    while out.chars().count() < 2 {
        out.push(' ');
    }
    out
}
