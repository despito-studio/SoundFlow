use crate::{
    desktop::Desktop,
    discord::DiscordRpc,
    playback::{AudioEvent, AudioPlayer, ListeningReport, PlayerState},
};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex, MutexGuard,
    },
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct SessionSnapshot {
    pub library: Value,
    pub settings: Value,
    pub cloud: Value,
    pub stats: Value,
    pub accounts: Value,
    pub home: Vec<Value>,
    pub feed: Value,
    pub creator: Value,
    pub profile_assets: Value,
    pub search: Vec<Value>,
    pub discovery: Vec<Value>,
    pub smart: Vec<Value>,
    pub downloads: Vec<Value>,
    pub queue: Vec<Value>,
    pub queue_index: usize,
    pub matches: Vec<Value>,
    pub pending_track: Option<Value>,
    pub current_user: String,
    pub view: String,
    pub selected_playlist: String,
    pub search_query: String,
    pub search_source: String,
    pub player: PlayerState,
    pub message: String,
    pub message_error: bool,
    pub pending: usize,
    pub last_method: String,
    pub last_result: Value,
    pub revision: u64,
}

impl Default for SessionSnapshot {
    fn default() -> Self {
        Self {
            library: json!({}),
            settings: json!({}),
            cloud: json!({}),
            stats: Value::Null,
            accounts: json!([]),
            home: Vec::new(),
            feed: json!({}),
            creator: json!({}),
            profile_assets: json!({}),
            search: Vec::new(),
            discovery: Vec::new(),
            smart: Vec::new(),
            downloads: Vec::new(),
            queue: Vec::new(),
            queue_index: 0,
            matches: Vec::new(),
            pending_track: None,
            current_user: "guest".into(),
            view: "home".into(),
            selected_playlist: String::new(),
            search_query: String::new(),
            search_source: "all".into(),
            player: PlayerState::default(),
            message: String::new(),
            message_error: false,
            pending: 0,
            last_method: String::new(),
            last_result: Value::Null,
            revision: 0,
        }
    }
}

impl SessionSnapshot {
    pub fn playlists(&self) -> Vec<Value> {
        self.library["playlists"]
            .as_object()
            .map(|items| items.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn tracks(&self) -> Vec<Value> {
        match self.view.as_str() {
            "search" => self.search.clone(),
            "flow" | "discover" => self.discovery.clone(),
            "smart" => self.smart.clone(),
            "liked" | "likes" => array(&self.library["liked_tracks"]),
            "history" => array(&self.library["history"]),
            "queue" => self.queue.clone(),
            "home" => {
                if self.home.is_empty() {
                    array(&self.library["history"])
                } else {
                    self.home.clone()
                }
            }
            "playlist" if !self.selected_playlist.is_empty() => {
                array(&self.library["playlists"][&self.selected_playlist]["tracks"])
            }
            _ => {
                let mut tracks = array(&self.library["liked_tracks"]);
                for playlist in self.playlists() {
                    tracks.extend(array(&playlist["tracks"]));
                }
                let mut seen = HashSet::new();
                tracks.retain(|track| seen.insert(track_key(track)));
                tracks
            }
        }
    }

    pub fn is_liked(&self, track: &Value) -> bool {
        self.library["liked_tracks"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| track_key(item) == track_key(track)))
    }
}

#[derive(Clone, Copy)]
enum Relevance {
    Always,
    Search(u64),
    Discovery(u64),
    Playback(u64),
}

struct RpcJob {
    method: String,
    args: Vec<Value>,
    profile: u64,
    relevance: Relevance,
    account: bool,
}

struct PlayRequest {
    track: Value,
    generation: u64,
    profile: u64,
    user: String,
}

struct Shared {
    state: Mutex<SessionSnapshot>,
    stopped: AtomicBool,
    account_pending: AtomicBool,
    profile_generation: AtomicU64,
    playback_generation: AtomicU64,
    search_generation: AtomicU64,
    discovery_generation: AtomicU64,
    resolving: AtomicBool,
    playback_gate: Mutex<()>,
    expected_track: Mutex<Option<String>>,
    previous_volume: Mutex<f32>,
    audio_settings: Mutex<Option<(f32, f32, bool, bool)>>,
    snapshot_sequence: AtomicU64,
    applied_snapshot: AtomicU64,
    desired_playing: AtomicBool,
    pending_settings: Mutex<serde_json::Map<String, Value>>,
    autoplay_pending: AtomicBool,
    oauth_pending: AtomicBool,
}

#[derive(Clone)]
pub struct NativeSession {
    pub desktop: Arc<Desktop>,
    pub audio: AudioPlayer,
    shared: Arc<Shared>,
    local: SyncSender<RpcJob>,
    remote: SyncSender<RpcJob>,
    playback: SyncSender<PlayRequest>,
    reports: SyncSender<ListeningReport>,
    discord: DiscordRpc,
}

impl NativeSession {
    pub fn new(data_dir: PathBuf, resources: PathBuf, offline: bool) -> Result<Self, String> {
        let desktop = Arc::new(Desktop::open(data_dir.clone(), resources, offline)?);
        let (audio, audio_events) = AudioPlayer::new_in(data_dir.join("audio-cache"));
        let (local, local_rx) = mpsc::sync_channel(64);
        let (remote, remote_rx) = mpsc::sync_channel(32);
        let (playback, playback_rx) = mpsc::sync_channel(32);
        let (reports, report_rx) = mpsc::sync_channel(64);
        let session = Self {
            desktop,
            audio,
            shared: Arc::new(Shared {
                state: Mutex::new(SessionSnapshot::default()),
                stopped: AtomicBool::new(false),
                account_pending: AtomicBool::new(false),
                profile_generation: AtomicU64::new(0),
                playback_generation: AtomicU64::new(0),
                search_generation: AtomicU64::new(0),
                discovery_generation: AtomicU64::new(0),
                resolving: AtomicBool::new(false),
                playback_gate: Mutex::new(()),
                expected_track: Mutex::new(None),
                previous_volume: Mutex::new(0.8),
                audio_settings: Mutex::new(None),
                snapshot_sequence: AtomicU64::new(0),
                applied_snapshot: AtomicU64::new(0),
                desired_playing: AtomicBool::new(false),
                pending_settings: Mutex::new(serde_json::Map::new()),
                autoplay_pending: AtomicBool::new(false),
                oauth_pending: AtomicBool::new(false),
            }),
            local,
            remote,
            playback,
            reports,
            discord: DiscordRpc::new(true),
        };
        for (name, receiver) in [
            ("soundflow-local", local_rx),
            ("soundflow-provider", remote_rx),
        ] {
            let worker = session.clone();
            std::thread::Builder::new()
                .name(name.into())
                .spawn(move || worker.rpc_loop(receiver))
                .map_err(|error| error.to_string())?;
        }
        let worker = session.clone();
        std::thread::Builder::new()
            .name("soundflow-resolve".into())
            .spawn(move || worker.playback_loop(playback_rx))
            .map_err(|error| error.to_string())?;
        let worker = session.clone();
        std::thread::Builder::new()
            .name("soundflow-events".into())
            .spawn(move || worker.audio_loop(audio_events))
            .map_err(|error| error.to_string())?;
        let worker = session.clone();
        std::thread::Builder::new()
            .name("soundflow-listening".into())
            .spawn(move || {
                while !worker.shared.stopped.load(Ordering::Acquire) {
                    match report_rx.recv_timeout(Duration::from_millis(250)) {
                        Ok(report) => {
                            if worker.desktop.record_audio(&report).is_ok() && report.history_due {
                                worker.refresh_snapshot();
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        let worker = session.clone();
        std::thread::Builder::new()
            .name("soundflow-downloads".into())
            .spawn(move || {
                while !worker.shared.stopped.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_secs(2));
                    let poll = {
                        let state = worker.state();
                        state.pending < 4
                            && (state.view == "downloads"
                                || state.downloads.iter().any(|job| {
                                    matches!(
                                        job["status"].as_str(),
                                        Some(
                                            "queued"
                                                | "downloading"
                                                | "processing"
                                                | "cancelling"
                                                | "running"
                                                | "resolving"
                                        )
                                    )
                                }))
                    };
                    if poll {
                        worker.call("get_downloads", vec![]);
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        session.call("_refresh", vec![]);
        Ok(session)
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        self.state().clone()
    }

    pub fn snapshot_value(&self) -> Value {
        let state = self.state();
        json!({
            "library": state.library, "settings": state.settings, "cloud": state.cloud,
            "stats": state.stats, "accounts": state.accounts, "home": state.home,
            "feed": state.feed, "creator": state.creator,
            "profile_assets": state.profile_assets,
            "search": state.search, "discovery": state.discovery, "smart": state.smart,
            "downloads": state.downloads, "queue": state.queue, "queue_index": state.queue_index,
            "matches": state.matches, "pending_track": state.pending_track,
            "current_user": state.current_user, "view": state.view,
            "selected_playlist": state.selected_playlist, "message": state.message,
            "message_error": state.message_error, "pending": state.pending,
            "search_query": state.search_query, "search_source": state.search_source,
            "last_method": state.last_method, "last_result": state.last_result,
            "revision": state.revision,
            "player": {
                "track": state.player.track, "playing": state.player.playing,
                "loading": state.player.loading, "position": state.player.position,
                "duration": state.player.duration, "volume": state.player.volume
            }
        })
    }

    pub fn navigate(&self, view: &str) {
        {
            let mut state = self.state();
            state.view = view.into();
            state.revision += 1;
        }
        match view {
            "home" | "discover" | "flow" if self.state().feed["loaded"] != true => {
                self.call("get_home_feed", vec![]);
            }
            "downloads" => self.call("get_downloads", vec![]),
            "stats" => self.call("get_listening_stats", vec![json!("30d"), json!("all")]),
            "connections" | "accounts" => self.call("list_accounts", vec![]),
            _ => {}
        }
    }

    pub fn select_playlist(&self, id: &str) {
        let mut state = self.state();
        state.selected_playlist = id.into();
        state.view = "playlist".into();
        state.revision += 1;
    }

    pub fn search(&self, query: &str, source: &str) {
        let query: String = query.trim().chars().take(500).collect();
        let source = if ["all", "youtube", "soundcloud", "spotify", "local"].contains(&source) {
            source
        } else {
            "all"
        };
        let generation = self.shared.search_generation.fetch_add(1, Ordering::AcqRel) + 1;
        {
            let mut state = self.state();
            state.view = "search".into();
            state.search_query = query.clone();
            state.search_source = source.into();
            state.search.clear();
            state.message.clear();
            state.revision += 1;
        }
        if query.is_empty() {
            return;
        }
        self.call("add_search_query", vec![json!(query)]);
        self.enqueue(
            "search",
            vec![json!(query), json!(source), json!(40)],
            Relevance::Search(generation),
        );
    }

    pub fn play(&self, track: Value, mut queue: Vec<Value>) {
        if self.shared.account_pending.load(Ordering::Acquire) {
            self.message("Зачекай, поки завершиться зміна профілю", true);
            return;
        }
        if track["id"].as_str().is_none_or(str::is_empty) || !track.is_object() {
            self.message("Трек більше недоступний. Онови список", true);
            return;
        }
        if queue.is_empty() {
            queue.push(track.clone());
        }
        let index = queue
            .iter()
            .position(|item| track_key(item) == track_key(&track));
        let index = match index {
            Some(index) => index,
            None => {
                queue.push(track.clone());
                queue.len() - 1
            }
        };
        {
            let mut state = self.state();
            state.queue = queue;
            state.queue_index = index;
            state.pending_track = None;
            state.matches.clear();
        }
        self.request_track(track);
    }

    pub fn player_action(&self, action: &str) {
        match action {
            "toggle" | "play-pause" => {
                let snapshot = self.snapshot();
                if snapshot.player.track.is_none() {
                    if let Some(track) = snapshot
                        .queue
                        .get(snapshot.queue_index)
                        .cloned()
                        .or_else(|| snapshot.tracks().first().cloned())
                    {
                        self.play(track, snapshot.tracks());
                    } else {
                        self.navigate("search");
                    }
                } else {
                    let wanted = !self.shared.desired_playing.load(Ordering::Acquire);
                    self.shared.desired_playing.store(wanted, Ordering::Release);
                    if wanted {
                        self.audio.play();
                    } else {
                        self.audio.pause();
                    }
                }
            }
            "play" => {
                self.shared.desired_playing.store(true, Ordering::Release);
                self.audio.play();
            }
            "pause" => {
                self.shared.desired_playing.store(false, Ordering::Release);
                self.audio.pause();
            }
            "next" => self.advance(1, false),
            "previous" => {
                if self.state().player.position > 3.0 {
                    self.seek(0.0);
                } else {
                    self.advance(-1, false);
                }
            }
            "stop" => self.stop(),
            "shuffle" => {
                let value = !self.state().settings["shuffle"].as_bool().unwrap_or(false);
                self.update_settings(json!({"shuffle": value}));
            }
            "repeat" => {
                let mode = match self.state().settings["repeat"].as_str() {
                    Some("all") => "one",
                    Some("one") => "off",
                    _ => "all",
                };
                self.update_settings(json!({"repeat": mode}));
            }
            "mute" => {
                let current = self.state().player.volume;
                let next = if current > 0.0 {
                    *lock(&self.shared.previous_volume) = current;
                    0.0
                } else {
                    *lock(&self.shared.previous_volume)
                };
                self.set_volume(next);
                self.update_settings(json!({"volume": next}));
            }
            _ => {}
        }
    }

    pub fn seek(&self, position: f64) {
        let _gate = lock(&self.shared.playback_gate);
        if position.is_finite() {
            let duration = self.state().player.duration;
            let position = if duration > 0.0 {
                position.clamp(0.0, duration)
            } else {
                position.max(0.0)
            };
            if self.shared.resolving.load(Ordering::Acquire) {
                self.state().player.position = position;
            } else {
                self.audio.seek(position);
            }
        }
    }

    pub fn set_volume(&self, volume: f32) {
        if volume.is_finite() && !self.shared.account_pending.load(Ordering::Acquire) {
            let volume = volume.clamp(0.0, 1.0);
            lock(&self.shared.pending_settings).insert("volume".into(), json!(volume));
            {
                let mut state = self.state();
                state.player.volume = volume;
                state.settings["volume"] = json!(volume);
            }
            self.audio.set_volume(volume);
            if let Some(applied) = lock(&self.shared.audio_settings).as_mut() {
                applied.0 = volume;
            }
        }
    }

    pub fn update_settings(&self, patch: Value) {
        let Some(fields) = patch.as_object() else {
            return;
        };
        if self.shared.stopped.load(Ordering::Acquire) {
            return;
        }
        if self.shared.account_pending.load(Ordering::Acquire) {
            self.message("Зачекай, поки завершиться зміна профілю", true);
            return;
        }
        lock(&self.shared.pending_settings).extend(fields.clone());
        {
            let mut state = self.state();
            if !state.settings.is_object() {
                state.settings = json!({});
            }
            state
                .settings
                .as_object_mut()
                .unwrap()
                .extend(fields.clone());
            state.revision += 1;
        }
        self.apply_audio_settings();
        if !self.enqueue("update_settings", vec![patch.clone()], Relevance::Always) {
            self.finish_settings(&patch);
        }
    }

    pub fn call(&self, method: &str, args: Vec<Value>) {
        if method == "start_oauth" {
            self.start_oauth(args.first().and_then(Value::as_str).unwrap_or(""));
            return;
        }
        if method == "search" {
            self.search(
                args.first().and_then(Value::as_str).unwrap_or(""),
                args.get(1).and_then(Value::as_str).unwrap_or("all"),
            );
            return;
        }
        let relevance = if method == "get_discover_tracks" {
            Relevance::Discovery(
                self.shared
                    .discovery_generation
                    .fetch_add(1, Ordering::AcqRel)
                    + 1,
            )
        } else {
            Relevance::Always
        };
        let _ = self.enqueue(method, args, relevance);
    }

    fn start_oauth(&self, provider: &str) {
        if provider != "google" {
            self.message("Вхід через браузер для цього сервісу ще недоступний", true);
            return;
        }
        if self.shared.stopped.load(Ordering::Acquire) {
            return;
        }
        if self.shared.account_pending.load(Ordering::Acquire) {
            self.message("Зачекай, поки завершиться зміна профілю", false);
            return;
        }
        if self.shared.oauth_pending.swap(true, Ordering::AcqRel) {
            self.message("Заверши відкритий вхід у браузері", false);
            return;
        }
        let profile = self.shared.profile_generation.load(Ordering::Acquire);
        self.state().pending += 1;
        self.message("Відкриваємо вхід через Google у браузері…", false);
        let worker = self.clone();
        let result = std::thread::Builder::new()
            .name("soundflow-google-auth".into())
            .spawn(move || {
                let result = worker.google_oauth(profile);
                worker.shared.oauth_pending.store(false, Ordering::Release);
                {
                    let mut state = worker.state();
                    state.pending = state.pending.saturating_sub(1);
                    state.revision += 1;
                }
                if worker.shared.stopped.load(Ordering::Acquire)
                    || profile != worker.shared.profile_generation.load(Ordering::Acquire)
                {
                    return;
                }
                match result {
                    Ok(session) => worker.call(
                        "google_sign_in",
                        vec![
                            json!(session.email),
                            json!(session.name),
                            json!(session.avatar_url),
                        ],
                    ),
                    Err(error) => worker.message(error, true),
                }
            });
        if let Err(error) = result {
            self.shared.oauth_pending.store(false, Ordering::Release);
            let mut state = self.state();
            state.pending = state.pending.saturating_sub(1);
            drop(state);
            self.message(format!("Не вдалося почати вхід: {error}"), true);
        }
    }

    fn google_oauth(&self, profile: u64) -> Result<crate::supabase::GoogleUserSession, String> {
        let client = crate::supabase::SupabaseClient::new(&self.desktop.data_dir);
        let (url, receiver) = client.start_oauth_flow()?;
        open::that_detached(url)
            .map_err(|error| format!("Не вдалося відкрити браузер: {error}"))?;
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        loop {
            if self.shared.stopped.load(Ordering::Acquire)
                || profile != self.shared.profile_generation.load(Ordering::Acquire)
            {
                return Err("Вхід скасовано після зміни профілю".into());
            }
            if std::time::Instant::now() >= deadline {
                return Err("Час очікування входу в браузері минув. Спробуй ще раз".into());
            }
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(result) => {
                    let session = result?;
                    if self.shared.stopped.load(Ordering::Acquire)
                        || profile != self.shared.profile_generation.load(Ordering::Acquire)
                    {
                        return Err("Вхід скасовано після зміни профілю".into());
                    }
                    if let Some(token) = session.provider_token.as_deref() {
                        let _ = crate::supabase_auth::sign_in_with_id_token(
                            &client.config,
                            &self.desktop.data_dir,
                            token,
                        );
                    }
                    return Ok(session);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Вхід через Google не завершено".into());
                }
            }
        }
    }

    pub fn action(&self, action: &str, payload: Value) {
        match action {
            "dismiss-message" => self.clear_message(),
            "navigate" => self.navigate(payload.as_str().unwrap_or("home")),
            "select-playlist" => self.select_playlist(payload.as_str().unwrap_or("")),
            "play-track" => {
                let queue = self.state().tracks();
                if payload.is_object() {
                    self.play(payload, queue);
                } else if let Some(track) = self.find_track(payload.as_str().unwrap_or("")) {
                    self.play(track, queue);
                }
            }
            "like" | "toggle-like" => {
                let track = if payload.is_object() {
                    Some(payload)
                } else {
                    self.find_track(payload.as_str().unwrap_or(""))
                };
                if let Some(track) = track {
                    self.call("toggle_like", vec![track]);
                }
            }
            "download" | "download-track" => {
                let track = if payload.is_object() {
                    Some(payload)
                } else {
                    self.find_track(payload.as_str().unwrap_or(""))
                };
                if let Some(track) = track {
                    self.call("download_track", vec![track]);
                }
            }
            "choose_match" => self.choose_match(payload),
            "dismiss_match" => {
                let mut state = self.state();
                state.pending_track = None;
                state.matches.clear();
            }
            "queue_move" => self.move_queue(
                payload["from"].as_u64().unwrap_or(u64::MAX) as usize,
                payload["to"].as_u64().unwrap_or(u64::MAX) as usize,
            ),
            "queue_play" => {
                if self.shared.account_pending.load(Ordering::Acquire) {
                    self.message("Зачекай, поки завершиться зміна профілю", true);
                    return;
                }
                let Some(index) = payload["index"].as_u64().map(|index| index as usize) else {
                    return;
                };
                let toggle = {
                    let state = self.state();
                    state.queue_index == index
                        && state
                            .queue
                            .get(index)
                            .zip(state.player.track.as_ref())
                            .is_some_and(|(queued, current)| {
                                track_key(queued) == track_key(current)
                            })
                };
                if toggle {
                    self.player_action("toggle");
                } else {
                    let track = select_queued_track(&mut self.state(), index);
                    if let Some(track) = track {
                        self.request_track(track);
                    }
                }
            }
            "queue_remove" => {
                self.remove_queue(payload["index"].as_u64().unwrap_or(u64::MAX) as usize)
            }
            "queue_clear" => {
                let mut state = self.state();
                let playing = state.player.track.clone();
                state.queue = playing.into_iter().collect();
                state.queue_index = 0;
                state.revision += 1;
            }
            "queue_add_next" | "add_next" => {
                let track = if payload.is_object() {
                    Some(payload)
                } else {
                    self.find_track(payload.as_str().unwrap_or(""))
                };
                if let Some(track) = track {
                    let mut state = self.state();
                    let index = if state.queue.is_empty() {
                        0
                    } else {
                        (state.queue_index + 1).min(state.queue.len())
                    };
                    state.queue.insert(index, track);
                    state.revision += 1;
                }
            }
            "refresh" => self.call("_refresh", vec![]),
            "scan-local" => self.call("scan_local_music", vec![Value::Null]),
            "set-settings" | "update-settings" => self.update_settings(payload),
            "toggle" | "play" | "pause" | "next" | "previous" | "shuffle" | "repeat" | "mute"
            | "stop" => self.player_action(action),
            _ => {
                let args = if let Some(args) = payload.as_array() {
                    args.clone()
                } else if payload.is_null() {
                    vec![]
                } else {
                    vec![payload]
                };
                self.call(action, args);
            }
        }
    }

    pub fn clear_message(&self) {
        let mut state = self.state();
        state.message.clear();
        state.message_error = false;
        state.revision += 1;
    }

    pub fn shutdown(&self) {
        if self.shared.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        self.shared
            .playback_generation
            .fetch_add(1, Ordering::AcqRel);
        if let Some(report) = self.audio.drain() {
            let desktop = self.desktop.clone();
            let (done, wait) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = desktop.record_audio(&report);
                let _ = done.send(());
            });
            let _ = wait.recv_timeout(Duration::from_millis(500));
        }
        self.audio.shutdown();
        self.discord.shutdown();
        self.desktop.shutdown();
    }

    fn state(&self) -> MutexGuard<'_, SessionSnapshot> {
        lock(&self.shared.state)
    }

    fn message(&self, message: impl Into<String>, error: bool) {
        let mut state = self.state();
        state.message = message.into();
        state.message_error = error;
        state.revision += 1;
    }

    fn stop(&self) {
        let _gate = lock(&self.shared.playback_gate);
        self.shared
            .playback_generation
            .fetch_add(1, Ordering::AcqRel);
        self.shared.resolving.store(false, Ordering::Release);
        self.shared.desired_playing.store(false, Ordering::Release);
        *lock(&self.shared.expected_track) = None;
        self.audio.stop();
        let mut state = self.state();
        state.player.track = None;
        state.player.loading = false;
        state.player.playing = false;
        state.player.position = 0.0;
        state.revision += 1;
    }

    fn request_track(&self, track: Value) {
        if self.shared.stopped.load(Ordering::Acquire) {
            return;
        }
        if self.shared.account_pending.load(Ordering::Acquire) {
            return;
        }
        let _gate = lock(&self.shared.playback_gate);
        let generation = self
            .shared
            .playback_generation
            .fetch_add(1, Ordering::AcqRel)
            + 1;
        self.shared.resolving.store(true, Ordering::Release);
        self.shared.desired_playing.store(true, Ordering::Release);
        *lock(&self.shared.expected_track) = Some(track_key(&track));
        self.audio.stop();
        let user = {
            let mut state = self.state();
            state.player.track = Some(track.clone());
            state.player.playing = false;
            state.player.loading = true;
            state.player.position = 0.0;
            state.player.duration = crate::playback::track_duration(&track);
            state.message.clear();
            state.message_error = false;
            state.revision += 1;
            state.current_user.clone()
        };
        let request = PlayRequest {
            track,
            generation,
            profile: self.shared.profile_generation.load(Ordering::Acquire),
            user,
        };
        if self.playback.try_send(request).is_err() {
            self.shared.resolving.store(false, Ordering::Release);
            self.state().player.loading = false;
            self.message("Зачекай, поки завершиться поточний запит", true);
        }
    }

    fn advance(&self, direction: isize, finished: bool) {
        let radio = {
            let state = self.state();
            let at_end = !state.queue.is_empty() && state.queue_index + 1 >= state.queue.len();
            if direction > 0
                && at_end
                && !state.settings["shuffle"].as_bool().unwrap_or(false)
                && state.settings["repeat"].as_str().unwrap_or("off") == "off"
                && state.settings["infinite_autoplay"]
                    .as_bool()
                    .unwrap_or(true)
            {
                Some(json!({
                    "current_track": state.queue.get(state.queue_index),
                    "playlist_name": state.library["playlists"][&state.selected_playlist]["name"].as_str().unwrap_or(""),
                    "tracks": state.queue.iter().rev().take(30).cloned().collect::<Vec<_>>(),
                    "history": array(&state.library["history"]).into_iter().take(20).collect::<Vec<_>>(),
                    "exclude_ids": state.queue.iter().filter_map(|track| track["id"].as_str()).collect::<Vec<_>>(),
                    "seed": state.queue.len()
                }))
            } else {
                None
            }
        };
        if let Some(context) = radio {
            if !self.shared.autoplay_pending.swap(true, Ordering::AcqRel) {
                let generation = self.shared.playback_generation.load(Ordering::Acquire);
                if !self.enqueue(
                    "_autoplay",
                    vec![context, json!(15)],
                    Relevance::Playback(generation),
                ) {
                    self.shared.autoplay_pending.store(false, Ordering::Release);
                }
            }
            return;
        }
        let next = {
            let mut state = self.state();
            let shuffle = state.settings["shuffle"].as_bool().unwrap_or(false);
            let repeat = state.settings["repeat"].as_str().unwrap_or("off");
            let Some(index) = next_index(
                state.queue.len(),
                state.queue_index,
                direction,
                finished,
                shuffle,
                repeat,
            ) else {
                return;
            };
            state.queue_index = index;
            state.revision += 1;
            state.queue.get(index).cloned()
        };
        if let Some(track) = next {
            self.request_track(track);
        }
    }

    fn move_queue(&self, from: usize, to: usize) {
        let mut state = self.state();
        if from >= state.queue.len() || to >= state.queue.len() || from == to {
            return;
        }
        let current = state.queue_index;
        let track = state.queue.remove(from);
        state.queue.insert(to, track);
        state.queue_index = moved_index(current, from, to);
        state.revision += 1;
    }

    fn remove_queue(&self, index: usize) {
        let next = {
            let mut state = self.state();
            if index >= state.queue.len() {
                return;
            }
            state.queue.remove(index);
            let removed_current = index == state.queue_index;
            if index < state.queue_index {
                state.queue_index -= 1;
            }
            state.queue_index = state.queue_index.min(state.queue.len().saturating_sub(1));
            state.revision += 1;
            if removed_current {
                Some(state.queue.get(state.queue_index).cloned())
            } else {
                None
            }
        };
        match next {
            Some(Some(track)) => self.request_track(track),
            Some(None) => self.stop(),
            None => {}
        }
    }

    fn find_track(&self, id: &str) -> Option<Value> {
        let state = self.state();
        let mut candidates = state.tracks();
        candidates.extend(state.queue.iter().cloned());
        candidates.extend(state.matches.iter().cloned());
        candidates.extend(state.discovery.iter().cloned());
        if let Some(track) = state.player.track.as_ref() {
            candidates.push(track.clone());
        }
        candidates
            .into_iter()
            .find(|track| track["id"].as_str() == Some(id))
    }

    fn choose_match(&self, candidate: Value) {
        let chosen = {
            let mut state = self.state();
            let Some(selected) = confirmed_spotify_match(&state, &candidate) else {
                return;
            };
            let index = state.queue_index;
            if let Some(track) = state.queue.get_mut(index) {
                *track = selected.clone();
            }
            state.pending_track = None;
            state.matches.clear();
            selected
        };
        self.request_track(chosen);
    }

    fn enqueue(&self, method: &str, args: Vec<Value>, relevance: Relevance) -> bool {
        if self.shared.stopped.load(Ordering::Acquire) {
            return false;
        }
        let account = account_method(method);
        if account {
            if self.shared.account_pending.swap(true, Ordering::AcqRel) {
                self.message("Зачекай, поки завершиться зміна профілю", true);
                return false;
            }
            self.shared
                .profile_generation
                .fetch_add(1, Ordering::AcqRel);
            lock(&self.shared.pending_settings).clear();
            self.stop();
        } else if self.shared.account_pending.load(Ordering::Acquire) && profile_write(method) {
            self.message("Зачекай, поки завершиться зміна профілю", true);
            return false;
        }
        let job = RpcJob {
            method: method.into(),
            args,
            profile: self.shared.profile_generation.load(Ordering::Acquire),
            relevance,
            account,
        };
        self.state().pending += 1;
        let channel = if remote_method(method) {
            &self.remote
        } else {
            &self.local
        };
        if channel.try_send(job).is_err() {
            let mut state = self.state();
            state.pending = state.pending.saturating_sub(1);
            drop(state);
            if account {
                self.shared.account_pending.store(false, Ordering::Release);
            }
            self.message("Зачекай, поки завершиться поточний запит", true);
            return false;
        }
        true
    }

    fn relevant(&self, job: &RpcJob) -> bool {
        job.profile == self.shared.profile_generation.load(Ordering::Acquire)
            && match job.relevance {
                Relevance::Always => true,
                Relevance::Search(generation) => {
                    generation == self.shared.search_generation.load(Ordering::Acquire)
                }
                Relevance::Discovery(generation) => {
                    generation == self.shared.discovery_generation.load(Ordering::Acquire)
                }
                Relevance::Playback(generation) => {
                    generation == self.shared.playback_generation.load(Ordering::Acquire)
                }
            }
    }

    fn rpc_loop(&self, receiver: Receiver<RpcJob>) {
        while !self.shared.stopped.load(Ordering::Acquire) {
            let job = match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(job) => job,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if !self.relevant(&job) {
                self.complete_job(&job);
                continue;
            }
            let result = if job.method == "_refresh" {
                Ok(Value::Null)
            } else if job.method == "_autoplay" {
                self.desktop.call("get_infinite_recommendations", &job.args)
            } else {
                self.desktop.call(&job.method, &job.args)
            };
            if self.relevant(&job) {
                if job.method == "update_settings" {
                    if let Some(patch) = job.args.first() {
                        self.finish_settings(patch);
                    }
                }
                self.refresh_snapshot();
                self.apply_result(&job, result);
            }
            self.complete_job(&job);
        }
    }

    fn complete_job(&self, job: &RpcJob) {
        let mut state = self.state();
        state.pending = state.pending.saturating_sub(1);
        state.revision += 1;
        drop(state);
        if job.account {
            self.shared.account_pending.store(false, Ordering::Release);
        }
        if job.method == "_autoplay" {
            self.shared.autoplay_pending.store(false, Ordering::Release);
        }
    }

    fn refresh_snapshot(&self) {
        let snapshot = {
            let core = lock(&self.desktop.core);
            (
                core.library.snapshot(),
                Value::Object(core.library.data.settings.clone()),
                core.account_status(),
                core.user().to_owned(),
                core.accounts.list_accounts(),
                self.shared.snapshot_sequence.fetch_add(1, Ordering::AcqRel) + 1,
            )
        };
        let pending_settings = lock(&self.shared.pending_settings).clone();
        let changed = {
            let mut state = self.state();
            if snapshot.5 < self.shared.applied_snapshot.load(Ordering::Acquire) {
                return;
            }
            self.shared
                .applied_snapshot
                .store(snapshot.5, Ordering::Release);
            let changed = state.current_user != snapshot.3;
            state.library = snapshot.0;
            state.settings = snapshot.1;
            if let Some(settings) = state.settings.as_object_mut() {
                settings.extend(pending_settings);
            }
            state.cloud = snapshot.2;
            state.current_user = snapshot.3;
            state.accounts = snapshot.4;
            if changed {
                state.search.clear();
                state.home.clear();
                state.feed = json!({});
                state.creator = json!({});
                state.profile_assets = json!({});
                state.smart.clear();
                state.discovery.clear();
                state.queue.clear();
                state.queue_index = 0;
                state.matches.clear();
                state.pending_track = None;
                state.selected_playlist.clear();
                state.stats = Value::Null;
            }
            state.revision += 1;
            changed
        };
        if changed {
            self.stop();
        }
        self.apply_audio_settings();
    }

    fn finish_settings(&self, patch: &Value) {
        if let Some(fields) = patch.as_object() {
            let mut pending = lock(&self.shared.pending_settings);
            for (key, value) in fields {
                if pending.get(key) == Some(value) {
                    pending.remove(key);
                }
            }
        }
    }

    fn apply_audio_settings(&self) {
        let settings = self.state().settings.clone();
        let wanted = (
            settings["volume"].as_f64().unwrap_or(0.8) as f32,
            settings["playback_speed"].as_f64().unwrap_or(1.0) as f32,
            settings["volume_norm_enabled"].as_bool().unwrap_or(true),
            settings["discord_rpc_enabled"].as_bool().unwrap_or(true),
        );
        let mut applied = lock(&self.shared.audio_settings);
        if applied.is_none_or(|old| old.0 != wanted.0) {
            self.audio.set_volume(wanted.0);
        }
        if applied.is_none_or(|old| old.1 != wanted.1) {
            self.audio.set_speed(wanted.1);
        }
        if applied.is_none_or(|old| old.2 != wanted.2) {
            self.audio.set_normalize(wanted.2);
        }
        if applied.is_none_or(|old| old.3 != wanted.3) {
            self.discord.set_enabled(wanted.3);
        }
        *applied = Some(wanted);
    }

    fn apply_result(&self, job: &RpcJob, result: Result<Value, String>) {
        if job.method == "_autoplay" {
            self.apply_autoplay(result);
            return;
        }
        let mut state = self.state();
        state.last_method = job.method.clone();
        match result {
            Err(error) => {
                state.message = error.clone();
                state.message_error = true;
                state.last_result = json!({"ok": false, "error": error});
            }
            Ok(value) => {
                state.last_result = value.clone();
                match job.method.as_str() {
                    "search" => state.search = array(&value),
                    "get_home_feed" => state.feed = value.clone(),
                    "import_external_profile" => {
                        state.view = "library".into();
                        if value["profile"] == true {
                            state.message = format!(
                                "Імпортовано плейлістів: {} · треків: {}",
                                value["playlists"].as_u64().unwrap_or(0),
                                value["count"].as_u64().unwrap_or(0)
                            );
                            state.message_error = false;
                        }
                    }
                    "get_creator_profile" => state.creator = value.clone(),
                    "get_profile_assets" => state.profile_assets = value.clone(),
                    "get_discover_tracks" => {
                        state.discovery = array(&value);
                        state.home = state.discovery.clone();
                    }
                    "smart_playlist" => state.smart = array(&value),
                    "get_downloads" => {
                        state.downloads = if value.is_array() {
                            array(&value)
                        } else {
                            array(&value["jobs"])
                        }
                    }
                    "download_track" | "cancel_download" => {
                        if value["job"].is_object() {
                            let job = &value["job"];
                            if let Some(existing) = state
                                .downloads
                                .iter_mut()
                                .find(|item| item["id"] == job["id"])
                            {
                                *existing = job.clone();
                            } else {
                                state.downloads.insert(0, job.clone());
                            }
                        }
                    }
                    "get_listening_stats" => state.stats = value.clone(),
                    "list_accounts" => state.accounts = value.clone(),
                    "get_cloud_status" => state.cloud = value.clone(),
                    "match_spotify_track" => state.matches = array(&value),
                    "create_playlist" => {
                        let id = value
                            .as_str()
                            .or_else(|| value["id"].as_str())
                            .unwrap_or("");
                        if !id.is_empty() {
                            state.selected_playlist = id.into();
                            state.view = "playlist".into();
                        }
                    }
                    "delete_playlist" => {
                        state.selected_playlist.clear();
                        state.view = "library".into();
                    }
                    _ => {}
                }
                if value["ok"] == false || value["success"] == false {
                    if value["cancelled"] != true
                        && !(job.method == "get_listening_stats" && value["locked"] == true)
                    {
                        state.message = value["error"]
                            .as_str()
                            .unwrap_or("Не вдалося завершити дію")
                            .into();
                        state.message_error = true;
                    }
                } else if !read_method(&job.method)
                    && !matches!(
                        job.method.as_str(),
                        "update_settings"
                            | "toggle_like"
                            | "remove_search_query"
                            | "clear_search_history"
                            | "import_external_profile"
                    )
                    && value["cancelled"] != true
                {
                    state.message = value["message"].as_str().unwrap_or("Готово").into();
                    state.message_error = false;
                }
            }
        }
        state.revision += 1;
    }

    fn apply_autoplay(&self, result: Result<Value, String>) {
        match result {
            Ok(value) => {
                let next = {
                    let mut state = self.state();
                    let mut seen = state.queue.iter().map(track_key).collect::<HashSet<_>>();
                    let fresh = array(&value)
                        .into_iter()
                        .filter(|track| seen.insert(track_key(track)))
                        .collect::<Vec<_>>();
                    let index = state.queue.len();
                    state.queue.extend(fresh);
                    let next = state.queue.get(index).cloned();
                    if next.is_some() {
                        state.queue_index = index;
                    }
                    state.revision += 1;
                    next
                };
                if let Some(track) = next {
                    self.request_track(track);
                } else {
                    self.message("Не знайшли нових треків для продовження черги", false);
                }
            }
            Err(error) => self.message(error, true),
        }
    }

    fn playback_loop(&self, receiver: Receiver<PlayRequest>) {
        while !self.shared.stopped.load(Ordering::Acquire) {
            let mut request = match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(request) => request,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            while let Ok(newer) = receiver.try_recv() {
                request = newer;
            }
            if !self.play_request_current(&request) {
                continue;
            }
            if request.track["source"] == "spotify"
                || (request.track["matched_from"].is_object()
                    && request.track["match_confirmed"] != true)
            {
                let original = if request.track["source"] == "spotify" {
                    request.track.clone()
                } else {
                    request.track["matched_from"].clone()
                };
                {
                    let _gate = lock(&self.shared.playback_gate);
                    if !self.play_request_current(&request) {
                        continue;
                    }
                    let mut state = self.state();
                    state.pending_track = Some(original.clone());
                    state.player.loading = false;
                    state.message = "Обери доступну версію для відтворення".into();
                    state.message_error = false;
                    self.shared.resolving.store(false, Ordering::Release);
                }
                let result = self
                    .desktop
                    .call("match_spotify_track", &[original, json!("all"), json!(6)]);
                let _gate = lock(&self.shared.playback_gate);
                if self.play_request_current(&request) {
                    match result {
                        Ok(value) => {
                            self.state().matches = array(&value);
                        }
                        Err(error) => self.message(error, true),
                    }
                }
                continue;
            }
            let source = self.desktop.call("get_stream", &[request.track.clone()]);
            let _gate = lock(&self.shared.playback_gate);
            if !self.play_request_current(&request) {
                continue;
            }
            match source {
                Ok(source) => {
                    if let Some(source) = source.as_str() {
                        self.audio.set_identity(request.user);
                        let position = self.state().player.position;
                        self.audio
                            .load(request.track, source.into(), position, None);
                        if !self.shared.desired_playing.load(Ordering::Acquire) {
                            self.audio.pause();
                        }
                    } else {
                        self.state().player.loading = false;
                        self.message("Недійсне джерело аудіо", true);
                    }
                }
                Err(error) => {
                    self.state().player.loading = false;
                    self.message(error, true);
                }
            }
            self.shared.resolving.store(false, Ordering::Release);
        }
    }

    fn play_request_current(&self, request: &PlayRequest) -> bool {
        !self.shared.stopped.load(Ordering::Acquire)
            && request.generation == self.shared.playback_generation.load(Ordering::Acquire)
            && request.profile == self.shared.profile_generation.load(Ordering::Acquire)
    }

    fn audio_loop(&self, receiver: Receiver<AudioEvent>) {
        while !self.shared.stopped.load(Ordering::Acquire) {
            let event = match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(event) => event,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            match event {
                AudioEvent::State(mut player) => {
                    if self.shared.resolving.load(Ordering::Acquire) {
                        continue;
                    }
                    let expected = lock(&self.shared.expected_track).clone();
                    if player.track.as_ref().map(track_key) != expected {
                        continue;
                    }
                    if player.duration <= 0.5 {
                        player.duration = player
                            .track
                            .as_ref()
                            .map(crate::playback::track_duration)
                            .unwrap_or(0.0);
                    }
                    self.discord.update(
                        player.track.as_ref(),
                        player.playing,
                        player.position,
                        player.duration,
                    );
                    let mut state = self.state();
                    player.volume = state.settings["volume"]
                        .as_f64()
                        .unwrap_or(player.volume as f64) as f32;
                    state.player = player;
                    state.revision += 1;
                }
                AudioEvent::Finished => {
                    if !self.shared.resolving.load(Ordering::Acquire)
                        && !self.state().player.loading
                    {
                        self.advance(1, true);
                    }
                }
                AudioEvent::Error(message) => {
                    if !self.shared.resolving.load(Ordering::Acquire) {
                        self.state().player.loading = false;
                        self.message(message, true);
                    }
                }
                AudioEvent::Listening {
                    track,
                    user_id,
                    session_id,
                    started_at,
                    heard_seconds,
                    history_due,
                } => {
                    let _ = self.reports.try_send(ListeningReport {
                        track,
                        user_id,
                        session_id,
                        started_at,
                        heard_seconds,
                        history_due,
                    });
                }
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

fn array(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}

fn confirmed_spotify_match(state: &SessionSnapshot, candidate: &Value) -> Option<Value> {
    let original = state.pending_track.as_ref()?;
    let mut selected = state
        .matches
        .iter()
        .find(|item| item["id"] == candidate["id"])?
        .clone();
    if selected["matched_from"]["id"] != original["id"]
        || !matches!(selected["source"].as_str(), Some("youtube" | "soundcloud"))
    {
        return None;
    }
    selected["match_confirmed"] = json!(true);
    Some(selected)
}

fn track_key(track: &Value) -> String {
    format!(
        "{}:{}",
        track["source"].as_str().unwrap_or(""),
        track["id"].as_str().unwrap_or("")
    )
}

fn moved_index(current: usize, from: usize, to: usize) -> usize {
    if current == from {
        to
    } else if from < current && current <= to {
        current - 1
    } else if to <= current && current < from {
        current + 1
    } else {
        current
    }
}

fn select_queued_track(state: &mut SessionSnapshot, index: usize) -> Option<Value> {
    let track = state.queue.get(index)?.clone();
    state.queue_index = index;
    state.pending_track = None;
    state.matches.clear();
    state.revision += 1;
    Some(track)
}

fn next_index(
    length: usize,
    current: usize,
    direction: isize,
    finished: bool,
    shuffle: bool,
    repeat: &str,
) -> Option<usize> {
    if length == 0 {
        return None;
    }
    let current = current.min(length - 1);
    if finished && repeat == "one" {
        return Some(current);
    }
    if shuffle && length > 1 {
        let random =
            u64::from_le_bytes(uuid::Uuid::new_v4().as_bytes()[..8].try_into().unwrap()) as usize;
        return Some((current + 1 + random % (length - 1)) % length);
    }
    if finished && direction > 0 && current + 1 >= length && repeat != "all" {
        return None;
    }
    Some((current as isize + direction).rem_euclid(length as isize) as usize)
}

fn account_method(method: &str) -> bool {
    matches!(
        method,
        "configure_cloud"
            | "restore_session"
            | "sign_in"
            | "sign_up"
            | "sign_out"
            | "switch_account"
            | "google_sign_in"
            | "github_sign_in"
    )
}

fn profile_write(method: &str) -> bool {
    !read_method(method)
        || matches!(
            method,
            "add_search_query"
                | "scan_local_music"
                | "sync_connected_accounts"
                | "import_external_profile"
        )
}

fn remote_method(method: &str) -> bool {
    matches!(
        method,
        "_autoplay"
            | "search"
            | "get_discover_tracks"
            | "get_home_feed"
            | "get_creator_profile"
            | "get_profile_assets"
            | "get_similar_tracks"
            | "get_infinite_recommendations"
            | "match_spotify_track"
            | "resolve_direct_url"
            | "download_track"
            | "get_downloads"
            | "get_download_job"
            | "cancel_download"
            | "detect_connected_accounts"
            | "sync_connected_accounts"
            | "import_external_profile"
            | "provider_ping"
    )
}

fn read_method(method: &str) -> bool {
    method.starts_with("get_")
        || method.starts_with("list_")
        || matches!(
            method,
            "_refresh"
                | "search"
                | "search_library"
                | "smart_playlist"
                | "is_liked"
                | "provider_ping"
                | "add_search_query"
        )
}
