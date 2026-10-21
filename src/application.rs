//! Native UI adapters, asynchronous commands, and playback orchestration.
use crate::{
    desktop::Desktop,
    playback::{AudioEvent, AudioPlayer, ListeningReport},
    AppWindow, AccountRow, CommandRow, DailyRow, DownloadRow, PlaylistRow, RankRow, StatMetric,
    TrackRow,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Default)]
struct UiData {
    library: Value,
    cloud: Value,
    settings: Value,
    registry: HashMap<String, Value>,
    search: Vec<Value>,
    smart: Vec<Value>,
    home: Vec<Value>,
    flow: Vec<Value>,
    flow_index: usize,
    preview: bool,
    preview_start: f64,
    selected_playlist: String,
    queue: Vec<Value>,
    queue_index: usize,
    shuffle: bool,
    repeat: u8,
    pending_track: Option<Value>,
    add_to_new_playlist: bool,
    user: String,
    downloads: Vec<Value>,
    stats: Value,
    matches: Vec<Value>,
    matches_owner: String,
    pending_confirm: Option<(String, Vec<Value>)>,
    snapshot_version: u64,
    stats_period: String,
    stats_source: String,
    radio_buffer: Vec<Value>,
    radio_fetching: bool,
    radio_waiting: bool,
    radio_seed: usize,
    radio_playlist_name: String,
}
struct Shared {
    data: Mutex<UiData>,
    cache: PathBuf,
    generation: AtomicU64,
    pending: AtomicUsize,
    artwork_busy: AtomicBool,
    snapshot_sequence: AtomicU64,
    close_to_tray: Arc<AtomicBool>,
    media_hotkeys: Arc<AtomicBool>,
    discord_rpc: Arc<AtomicBool>,
    playback_gate: Mutex<()>,
    offline: Mutex<std::collections::HashSet<String>>,
    audio_applied: Mutex<(f32, bool)>,
    account_pending: AtomicUsize,
}
struct Command {
    method: String,
    args: Vec<Value>,
    scope: String,
    quiet: bool,
    generation: u64,
}
#[derive(Clone)]
pub struct Application {
    pub desktop: Arc<Desktop>,
    pub audio: AudioPlayer,
    pub discord: crate::discord::DiscordRpc,
    shared: Arc<Shared>,
    sender: SyncSender<Command>,
    receiver: Arc<Mutex<Option<Receiver<Command>>>>,
    audio_events: Arc<Mutex<Option<Receiver<AudioEvent>>>>,
}
pub struct UiBinding {
    _poll: slint::Timer,
}
impl Application {
    pub fn new(data_dir: PathBuf, resource_dir: PathBuf, offline: bool) -> Result<Self, String> {
        let desktop = Arc::new(Desktop::open(data_dir.clone(), resource_dir, offline)?);
        let (audio, events) = AudioPlayer::new_in(data_dir.join("audio-cache"));
        let discord = crate::discord::DiscordRpc::new(true);
        let cache = data_dir.join("artwork");
        fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
        let (sender, receiver) = mpsc::sync_channel(64);
        Ok(Self {
            desktop,
            audio,
            discord,
            shared: Arc::new(Shared {
                data: Mutex::new(UiData {
                    user: "guest".into(),
                    stats_period: "30d".into(),
                    stats_source: "all".into(),
                    ..Default::default()
                }),
                cache,
                generation: AtomicU64::new(0),
                pending: AtomicUsize::new(0),
                artwork_busy: AtomicBool::new(false),
                snapshot_sequence: AtomicU64::new(0),
                close_to_tray: Arc::new(AtomicBool::new(true)),
                media_hotkeys: Arc::new(AtomicBool::new(true)),
                discord_rpc: Arc::new(AtomicBool::new(true)),
                playback_gate: Mutex::new(()),
                offline: Mutex::new(std::collections::HashSet::new()),
                audio_applied: Mutex::new((1.0, true)),
                account_pending: AtomicUsize::new(0),
            }),
            sender,
            receiver: Arc::new(Mutex::new(Some(receiver))),
            audio_events: Arc::new(Mutex::new(Some(events))),
        })
    }
    fn submit(&self, ui: &AppWindow, method: &str, args: Vec<Value>, quiet: bool) {
        if self.shared.account_pending.load(Ordering::SeqCst) > 0 && profile_write(method) {
            ui.set_message(crate::tr!(ui, "Зачекайте на завершення зміни профілю", "Wait for the profile change to finish"));
            return;
        }
        if account_method(method) {
            self.shared.account_pending.fetch_add(1, Ordering::SeqCst);
        }
        if !quiet {
            self.shared.pending.fetch_add(1, Ordering::Relaxed);
            ui.set_task_loading(true);
        }
        let command = Command {
            method: method.into(),
            args,
            scope: ui.get_view().to_string(),
            quiet,
            generation: self.shared.generation.load(Ordering::SeqCst),
        };
        if self.sender.try_send(command).is_err() {
            if account_method(method) {
                self.shared.account_pending.fetch_sub(1, Ordering::SeqCst);
            }
            if !quiet {
                self.shared.pending.fetch_sub(1, Ordering::Relaxed);
                ui.set_task_loading(self.shared.pending.load(Ordering::Relaxed) > 0);
            }
            ui.set_message(crate::tr!(ui, "Зачекайте на завершення поточного запиту", "Wait for the current request to finish"));
        }
    }
    fn play_track(&self, ui: &AppWindow, id: &str, preview: bool) {
        self.play_track_in(ui, id, preview, false);
    }
    fn play_track_in(&self, ui: &AppWindow, id: &str, preview: bool, retain_queue: bool) {
        if self.shared.account_pending.load(Ordering::SeqCst) > 0 {
            ui.set_message(crate::tr!(ui, "Зачекайте на завершення зміни профілю", "Wait for the profile change to finish"));
            return;
        }
        let mut data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
        let Some(track) = data
            .registry
            .get(id)
            .cloned()
            .or_else(|| data.queue.iter().find(|t| text(t, "id") == id).cloned())
        else {
            ui.set_message(crate::tr!(ui, "Трек більше недоступний. Оновіть список.", "This track is no longer available. Refresh the list."));
            return;
        };
        data.registry.insert(id.to_string(), track.clone());
        if track["source"] == "spotify"
            || (track["matched_from"].is_object() && track["match_confirmed"] != true)
        {
            let original = if track["source"] == "spotify" {
                track.clone()
            } else {
                track["matched_from"].clone()
            };
            data.pending_track = Some(original.clone());
            drop(data);
            reset_modal(&ui);
            ui.set_modal_kind("match".into());
            ui.set_modal_title(crate::tr!(ui, "Оберіть джерело відтворення", "Choose a playback source"));
            ui.set_modal_description(crate::tr!(ui, "Spotify надає метадані. Відтворення й завантаження потребують вашого вибору версії з YouTube або SoundCloud.", "Spotify provides metadata. Playback and downloads need a version from YouTube or SoundCloud."));
            ui.set_modal_open(true);
            self.submit(
                ui,
                "match_spotify_track",
                vec![original, json!("all"), json!(6)],
                false,
            );
            return;
        }
        if !preview && !retain_queue {
            data.queue = ui
                .get_tracks()
                .iter()
                .filter_map(|row| data.registry.get(row.id.as_str()).cloned())
                .collect();
            data.queue_index = data
                .queue
                .iter()
                .position(|item| item["id"] == id)
                .unwrap_or(0);
            if data.queue.is_empty() {
                data.queue.push(track.clone());
            }
            data.radio_buffer.clear();
            data.radio_fetching = false;
            data.radio_waiting = false;
            data.radio_seed = 0;
            data.radio_playlist_name = if !data.selected_playlist.is_empty() {
                text(&data.library["playlists"][&data.selected_playlist], "name")
            } else if ui.get_view() == "likes" || ui.get_view() == "liked" {
                "Улюблені треки".into()
            } else {
                "".into()
            };
        }
        let prefetch_needed = !preview
            && data.settings["infinite_autoplay"].as_bool().unwrap_or(true)
            && data.queue_index + 2 >= data.queue.len()
            && data.radio_buffer.is_empty()
            && !data.radio_fetching;
        let prefetch_context = if prefetch_needed {
            data.radio_fetching = true;
            data.radio_waiting = false;
            Some(json!({
                "current_track": track.clone(),
                "playlist_name": data.radio_playlist_name,
                "tracks": data.queue.iter().rev().take(30).cloned().collect::<Vec<_>>(),
                "history": values(&data.library["history"]).iter().take(20).cloned().collect::<Vec<_>>(),
                "exclude_ids": data.queue.iter().map(|t| text(t, "id")).collect::<Vec<_>>(),
                "seed": data.radio_seed,
            }))
        } else {
            None
        };
        data.preview = preview;
        let duration = track["duration"].as_f64().unwrap_or(0.0);
        let start = if preview {
            track["highlight_start"].as_f64().unwrap_or_else(|| {
                if duration > 30.0 {
                    (duration - 30.0) / 2.0
                } else {
                    0.0
                }
            })
        } else {
            0.0
        };
        data.preview_start = start;
        let owner = data.user.clone();
        drop(data);
        if let Some(ctx) = prefetch_context {
            self.submit(ui, "get_infinite_recommendations", vec![ctx, json!(15)], true);
        }
        let gate = self
            .shared
            .playback_gate
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        self.audio.stop();
        drop(gate);
        ui.set_loading(true);
        self.submit(
            ui,
            "_play",
            vec![track, json!(start), json!(preview), json!(owner)],
            false,
        );
    }
    fn advance_infinite_radio(&self, ui: &AppWindow) {
        let (next_id, prefetch_context) = {
            let mut data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
            if !data.radio_buffer.is_empty() {
                data.radio_waiting = false;
                let next_track = data.radio_buffer.remove(0);
                let next_id = text(&next_track, "id");
                data.registry.insert(next_id.clone(), next_track.clone());
                data.queue.push(next_track);
                data.queue_index = data.queue.len() - 1;
                let need_more = data.radio_buffer.len() < 4 && !data.radio_fetching;
                if need_more {
                    data.radio_fetching = true;
                }
                let last_track = data.queue.last().cloned().unwrap_or(Value::Null);
                let prefetch = if need_more {
                    Some(json!({
                        "current_track": last_track,
                        "playlist_name": data.radio_playlist_name,
                        "tracks": data.queue.iter().rev().take(30).cloned().collect::<Vec<_>>(),
                        "history": values(&data.library["history"]).iter().take(20).cloned().collect::<Vec<_>>(),
                        "exclude_ids": data.queue.iter().map(|t| text(t, "id")).collect::<Vec<_>>(),
                        "seed": data.radio_seed,
                    }))
                } else {
                    None
                };
                (Some(next_id), prefetch)
            } else {
                data.radio_waiting = true;
                let prefetch = if !data.radio_fetching {
                    data.radio_fetching = true;
                    let last_track = data.queue.last().cloned().unwrap_or(Value::Null);
                    Some(json!({
                        "current_track": last_track,
                        "playlist_name": data.radio_playlist_name,
                        "tracks": data.queue.iter().rev().take(30).cloned().collect::<Vec<_>>(),
                        "history": values(&data.library["history"]).iter().take(20).cloned().collect::<Vec<_>>(),
                        "exclude_ids": data.queue.iter().map(|t| text(t, "id")).collect::<Vec<_>>(),
                        "seed": data.radio_seed,
                    }))
                } else {
                    None
                };
                (None, prefetch)
            }
        };
        if let Some(ctx) = prefetch_context {
            self.submit(ui, "get_infinite_recommendations", vec![ctx, json!(15)], true);
        }
        if let Some(id) = next_id {
            refresh_models(ui, &self.shared);
            self.play_track_in(ui, &id, false, true);
        }
    }
    fn next(&self, ui: &AppWindow, direction: isize) {
        let (at_end, infinite_autoplay) = {
            let data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
            (
                !data.preview && !data.shuffle && data.queue_index + 1 >= data.queue.len(),
                data.settings["infinite_autoplay"].as_bool().unwrap_or(true),
            )
        };
        if direction == 1 && at_end && infinite_autoplay {
            self.advance_infinite_radio(ui);
            return;
        }
        let (id, preview) = {
            let mut data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
            if data.preview {
                if data.flow.is_empty() {
                    return;
                }
                data.flow_index = (data.flow_index as isize + direction)
                    .rem_euclid(data.flow.len() as isize)
                    as usize;
                (text(&data.flow[data.flow_index], "id"), true)
            } else {
                if data.queue.is_empty() {
                    return;
                }
                data.queue_index = if data.shuffle {
                    usize::from(uuid::Uuid::new_v4().as_bytes()[0]) % data.queue.len()
                } else {
                    (data.queue_index as isize + direction).rem_euclid(data.queue.len() as isize)
                        as usize
                };
                (text(&data.queue[data.queue_index], "id"), false)
            }
        };
        refresh_models(ui, &self.shared);
        self.play_track_in(ui, &id, preview, true);
    }
    pub fn bind(&self, ui: &AppWindow) -> UiBinding {
        self.start_workers(ui);
        ui.set_app_version(env!("CARGO_PKG_VERSION").into());
        ui.set_commands(rows(command_catalog(ui.get_language().as_str())));
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_navigate(move |view| {
            if let Some(ui) = weak.upgrade() {
                if ui.get_view() == "flow" && view != "flow" {
                    this.shared.generation.fetch_add(1, Ordering::SeqCst);
                    this.audio.stop();
                    this.shared
                        .data
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .preview = false;
                }
                ui.set_view(view.clone());
                refresh_models(&ui, &this.shared);
                match view.as_str() {
                    "flow" => {
                        if this
                            .shared
                            .data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .flow
                            .is_empty()
                        {
                            this.submit(
                                &ui,
                                "get_discover_tracks",
                                vec![json!(ui.get_flow_genre().as_str())],
                                false,
                            );
                        }
                    }
                    "downloads" => this.submit(&ui, "get_downloads", vec![], false),
                    "stats" => {
                        let args = {
                            let data = this.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                            vec![json!(data.stats_period), json!(data.stats_source)]
                        };
                        this.submit(&ui, "get_listening_stats", args, false);
                    }
                    "search" => {
                        if let Ok(history) = this.desktop.call("get_search_history", &[]) {
                            if let Some(arr) = history.as_array() {
                                let slint_history: Vec<slint::SharedString> = arr.iter().filter_map(|v| v.as_str()).map(Into::into).collect();
                                ui.set_search_history(slint::ModelRc::new(slint::VecModel::from(slint_history)));
                            }
                        }
                        this.submit(&ui, "_refresh", vec![], true);
                    }
                    _ => this.submit(&ui, "_refresh", vec![], true),
                }
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_search(move |query, source| {
            if let Some(ui) = weak.upgrade() {
                ui.set_view("search".into());
                ui.set_search_query(query.clone());
                ui.set_search_source(source.clone());
                let clean = query.trim().to_string();
                if !clean.is_empty() {
                    let _ = this.desktop.call("add_search_query", &[json!(clean)]);
                    if let Ok(history) = this.desktop.call("get_search_history", &[]) {
                        if let Some(arr) = history.as_array() {
                            let slint_history: Vec<slint::SharedString> = arr.iter().filter_map(|v| v.as_str()).map(Into::into).collect();
                            ui.set_search_history(slint::ModelRc::new(slint::VecModel::from(slint_history)));
                        }
                    }
                }
                this.submit(
                    &ui,
                    "search",
                    vec![json!(query.as_str()), json!(source.as_str()), json!(24)],
                    false,
                );
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_select_playlist(move |id| {
            if let Some(ui) = weak.upgrade() {
                this.shared
                    .data
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .selected_playlist = id.to_string();
                ui.set_view("playlist".into());
                refresh_models(&ui, &this.shared);
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_play_track(move |id| {
            if let Some(ui) = weak.upgrade() {
                this.play_track(&ui, id.as_str(), false);
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_like_track(move |id| {
            if let Some(ui) = weak.upgrade() {
                if let Some(track) = this.chosen_track(&ui, id.as_str()) {
                    this.submit(&ui, "toggle_like", vec![track], false);
                }
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_download_track(move |id| {
            if let Some(ui) = weak.upgrade() {
                if let Some(track) = this.track(id.as_str()) {
                    if track["source"] == "spotify"
                        || (track["matched_from"].is_object() && track["match_confirmed"] != true)
                    {
                        let original = if track["source"] == "spotify" {
                            track.clone()
                        } else {
                            track["matched_from"].clone()
                        };
                        this.shared
                            .data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .pending_track = Some(original.clone());
                        reset_modal(&ui);
                        ui.set_modal_kind("match".into());
                        ui.set_modal_title(crate::tr!(ui, "Оберіть версію для завантаження", "Choose a version to download"));
                        ui.set_modal_open(true);
                        this.submit(
                            &ui,
                            "match_spotify_track",
                            vec![original, json!("all"), json!(6)],
                            false,
                        );
                    } else {
                        this.submit(&ui, "download_track", vec![track], false);
                    }
                }
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_add_track(move |id| {
            if let Some(ui) = weak.upgrade() {
                let track = this.chosen_track(&ui, id.as_str());
                if track.is_none() {
                    return;
                }
                this.shared
                    .data
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .pending_track = track;
                reset_modal(&ui);
                ui.set_modal_kind("add-to-playlist".into());
                ui.set_modal_title(crate::tr!(ui, "Додати до плейлиста", "Add to playlist"));
                ui.set_modal_open(true);
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_player_action(move |action| {
            if let Some(ui) = weak.upgrade() {
                match action.as_str() {
                    "toggle" => this.audio.toggle(),
                    "next" => this.next(&ui, 1),
                    "previous" => this.next(&ui, -1),
                    "shuffle" => {
                        let mut data = this.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                        data.shuffle = !data.shuffle;
                        ui.set_shuffle(data.shuffle);
                        let is_en = data.settings["language"] == "en";
                        this.submit(
                            &ui,
                            "update_settings",
                            vec![json!({"shuffle":data.shuffle})],
                            true,
                        );
                        ui.set_message(
                            if data.shuffle {
                                if is_en { "Shuffle enabled" } else { "Перемішування увімкнено" }
                            } else {
                                if is_en { "Shuffle disabled" } else { "Перемішування вимкнено" }
                            }
                            .into(),
                        );
                    }
                    "repeat" => {
                        let mut data = this.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                        data.repeat = (data.repeat + 1) % 3;
                        let is_en = data.settings["language"] == "en";
                        ui.set_repeat_mode(["off", "all", "one"][data.repeat as usize].into());
                        this.submit(
                            &ui,
                            "update_settings",
                            vec![json!({"repeat":(["off","all","one"][data.repeat as usize])})],
                            true,
                        );
                        ui.set_message(
                            if is_en {
                                ["Repeat off", "Repeat all", "Repeat one"][data.repeat as usize]
                            } else {
                                ["Повтор вимкнено", "Повтор черги", "Повтор треку"][data.repeat as usize]
                            }
                            .into(),
                        );
                    }
                    "mute" => this
                        .audio
                        .set_volume(if ui.get_volume() > 0.0 { 0.0 } else { 0.8 }),
                    "queue" => {
                        let queue = this
                            .shared
                            .data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .queue
                            .clone();
                        this.shared
                            .data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .matches = queue.clone();
                        ui.set_match_tracks(rows(
                            queue
                                .iter()
                                .map(|track| track_row(track, &this.shared))
                                .collect(),
                        ));
                        reset_modal(&ui);
                        ui.set_modal_kind("queue".into());
                        ui.set_modal_title(crate::tr!(ui, "Черга відтворення", "Playback queue"));
                        ui.set_modal_open(true);
                    }
                    _ => {}
                }
            }
        });
        let audio = self.audio.clone();
        ui.on_seek(move |position| audio.seek(f64::from(position)));
        let audio = self.audio.clone();
        let sender = self.sender.clone();
        let debounce = std::rc::Rc::new(slint::Timer::default());
        ui.on_set_volume(move |volume| {
            audio.set_volume(volume.clamp(0.0, 1.0));
            let sender = sender.clone();
            debounce.start(
                slint::TimerMode::SingleShot,
                Duration::from_secs(1),
                move || {
                    let _ = sender.try_send(Command {
                        method: "update_settings".into(),
                        args: vec![json!({"volume":volume})],
                        scope: "settings".into(),
                        quiet: true,
                        generation: 0,
                    });
                },
            );
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_flow_action(move |action| {
            if let Some(ui) = weak.upgrade() {
                match action.as_str() {
                    "skip" => this.next(&ui, 1),
                    "play" => {
                        if ui.get_flow_playing() {
                            this.audio.toggle();
                        } else {
                            let id = ui.get_flow_track().id;
                            this.play_track(&ui, id.as_str(), true);
                        }
                    }
                    "like" => {
                        let id = ui.get_flow_track().id;
                        if let Some(track) = this.track(id.as_str()) {
                            this.submit(&ui, "_flow_like", vec![track], false);
                        }
                        this.next(&ui, 1);
                    }
                    "save" => {
                        let id = ui.get_flow_track().id;
                        let track = this.track(id.as_str());
                        this.shared
                            .data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .pending_track = track;
                        reset_modal(&ui);
                        ui.set_modal_kind("add-to-playlist".into());
                        ui.set_modal_title(crate::tr!(ui, "Зберегти відкриття", "Save discovery"));
                        ui.set_modal_open(true);
                    }
                    "refresh" => this.submit(
                        &ui,
                        "get_discover_tracks",
                        vec![json!(ui.get_flow_genre().as_str())],
                        false,
                    ),
                    value if value.starts_with("genre:") => {
                        ui.set_flow_genre(value[6..].into());
                        this.submit(&ui, "get_discover_tracks", vec![json!(&value[6..])], false);
                    }
                    _ => {}
                }
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_action(move |method, payload| {
            if let Some(ui) = weak.upgrade() {
                this.action(&ui, method.as_str(), payload.as_str());
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_modal_submit(move |value| {
            if let Some(ui) = weak.upgrade() {
                this.modal_submit(&ui, value.as_str());
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_modal_action(move |method, value| {
            if let Some(ui) = weak.upgrade() {
                match method.as_str() {
                    "close" => {
                        // "Більше не питати" зберігається лише коли його справді обрали.
                        if ui.get_modal_kind() == "import-prompt" && ui.get_import_prompt_disabled() {
                            this.shared
                                .data
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .settings["import_prompt_disabled"] = json!(true);
                            this.submit(
                                &ui,
                                "update_settings",
                                vec![json!({"import_prompt_disabled": true})],
                                true,
                            );
                        }
                        close_modal(&ui);
                    }
                    "select-match" | "play-match" => {
                        if this.confirm_match(&ui, value.as_str()).is_none() {
                            return;
                        }
                        close_modal(&ui);
                        this.play_track(&ui, value.as_str(), false);
                    }
                    "play-queue" => {
                        close_modal(&ui);
                        let index = {
                            let data = this.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                            data.queue
                                .iter()
                                .position(|track| track["id"] == value.as_str())
                        };
                        if let Some(index) = index {
                            this.shared
                                .data
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .queue_index = index;
                            this.play_track_in(&ui, value.as_str(), false, true);
                        }
                    }
                    "download-match" => {
                        if let Some(track) = this.confirm_match(&ui, value.as_str()) {
                            close_modal(&ui);
                            this.submit(&ui, "download_track", vec![track], false);
                        }
                    }
                    "add-to-playlist" | "select-playlist" => {
                        let track = this
                            .shared
                            .data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .pending_track
                            .clone();
                        if let Some(track) = track {
                            this.submit(
                                &ui,
                                "add_to_playlist",
                                vec![json!(value.as_str()), track],
                                false,
                            );
                            close_modal(&ui);
                        }
                    }
                    _ => this.action(&ui, method.as_str(), value.as_str()),
                }
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_sign_in(move |email, password| {
            if let Some(ui) = weak.upgrade() {
                this.stop_for_account();
                this.submit(
                    &ui,
                    "sign_in",
                    vec![json!({"email":email.as_str(),"password":password.as_str()})],
                    false,
                );
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_sign_up(move |name, email, password| if let Some(ui) = weak.upgrade() { this.stop_for_account(); this.submit(&ui, "sign_up", vec![json!({"name":name.as_str(),"email":email.as_str(),"password":password.as_str()})], false); });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_configure_cloud(move |url| {
            if let Some(ui) = weak.upgrade() {
                this.stop_for_account();
                this.submit(
                    &ui,
                    "configure_cloud",
                    vec![json!({"base_url":url.as_str()})],
                    false,
                );
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_sync_library(move |mode| {
            if let Some(ui) = weak.upgrade() {
                if mode != "push" {
                    this.stop_for_account();
                }
                this.submit(&ui, "sync_library", vec![json!(mode.as_str())], false);
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_sign_out(move || {
            if let Some(ui) = weak.upgrade() {
                this.stop_for_account();
                crate::supabase_auth::clear(&crate::platform::data_dir());
                this.submit(&ui, "sign_out", vec![], false);
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_google_sign_in(move || {
            if let Some(ui) = weak.upgrade() {
                let is_en = ui.get_language() == "en";
                ui.set_message(if is_en {
                    "Opening Google Sign-In in your browser...".into()
                } else {
                    "Відкриваємо вхід через Google у браузері...".into()
                });

                let data_dir = crate::platform::data_dir();
                let supabase_client = crate::supabase::SupabaseClient::new(&data_dir);
                let auth_config = supabase_client.config.clone();
                let auth_dir = data_dir.clone();
                let app = this.clone();
                let weak_ui = ui.as_weak();

                match supabase_client.start_oauth_flow() {
                    Ok((auth_url, rx)) => {
                        let _ = open::that_detached(&auth_url);

                        std::thread::spawn(move || {
                            match rx.recv_timeout(Duration::from_secs(120)) {
                                Ok(Ok(session)) => {
                                    // Google id_token міняємо на сесію Supabase Auth: далі
                                    // запити до PostgREST ідуть з Bearer-токеном і RLS
                                    // перевіряє auth.uid().
                                    if let Some(id_token) = session.provider_token.as_deref() {
                                        let _ = crate::supabase_auth::sign_in_with_id_token(
                                            &auth_config,
                                            &auth_dir,
                                            id_token,
                                        );
                                    }
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = weak_ui.upgrade() {
                                            crate::platform::focus_main_window();
                                            ui.set_message(if ui.get_language() == "en" {
                                                "Google authorized! Connecting music accounts...".into()
                                            } else {
                                                "Авторизація через Google успішна! Шукаємо музичні акаунти...".into()
                                            });
                                            app.submit(
                                                &ui,
                                                "google_sign_in",
                                                vec![json!(session.email), json!(session.name), json!(session.avatar_url)],
                                                false,
                                            );
                                        }
                                    });
                                }
                                Ok(Err(err)) => {
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = weak_ui.upgrade() {
                                            ui.set_message_error(true);
                                            ui.set_message(format!("Помилка Google авторизації: {err}").into());
                                        }
                                    });
                                }
                                Err(_) => {
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = weak_ui.upgrade() {
                                            ui.set_message_error(true);
                                            ui.set_message(if ui.get_language() == "en" {
                                                "Google sign-in timed out.".into()
                                            } else {
                                                "Час очікування авторизації в браузері минув.".into()
                                            });
                                        }
                                    });
                                }
                            }
                        });
                    }
                    Err(err) => {
                        ui.set_message_error(true);
                        ui.set_message(format!("Помилка запуску OAuth: {err}").into());
                    }
                }
            }
        });
        let weak = ui.as_weak();
        ui.on_github_sign_in(move || {
            if let Some(ui) = weak.upgrade() {
                reset_modal(&ui);
                ui.set_modal_kind("github-account:".into());
                ui.set_modal_title(if ui.get_language() == "en" {
                    "Sign in with GitHub".into()
                } else {
                    "Вхід через GitHub".into()
                });
                ui.set_modal_description(if ui.get_language() == "en" {
                    "Enter your GitHub username or email to link your profile:".into()
                } else {
                    "Введи свій GitHub username або email для входу в SoundFlow:".into()
                });
                ui.set_modal_value("".into());
                ui.set_modal_open(true);
            }
        });
        let weak = ui.as_weak();
        ui.on_connect_account(move |provider, current| {
            if let Some(ui) = weak.upgrade() {
                reset_modal(&ui);
                ui.set_modal_kind(format!("connect:{}", provider).into());
                let is_en = ui.get_language() == "en";
                let p_title = match provider.as_str() {
                    "youtube" => "YouTube",
                    "soundcloud" => "SoundCloud",
                    "spotify" => "Spotify",
                    _ => provider.as_str(),
                };
                ui.set_modal_title(if is_en {
                    format!("Connect {p_title} Account").into()
                } else {
                    format!("Підключити акаунт {p_title}").into()
                });
                let p_desc = if is_en {
                    match provider.as_str() {
                        "youtube" => "Enter YouTube channel name, @handle, or playlist URL:",
                        "soundcloud" => "Enter SoundCloud username or profile/playlist URL:",
                        "spotify" => "Enter Spotify playlist, album, or track URL:",
                        _ => "Enter account identifier:",
                    }
                } else {
                    match provider.as_str() {
                        "youtube" => "Введіть назву каналу, @тег або посилання на плейлист YouTube:",
                        "soundcloud" => "Введіть ім'я профілю або посилання на плейлист/профіль SoundCloud:",
                        "spotify" => "Введіть посилання на плейлист, альбом або пісню Spotify:",
                        _ => "Введіть ідентифікатор акаунта:",
                    }
                };
                ui.set_modal_description(p_desc.into());
                ui.set_modal_value(current);
                ui.set_modal_open(true);
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_import_account(move |provider| {
            if let Some(ui) = weak.upgrade() {
                ui.set_modal_busy(true);
                let is_en = ui.get_language() == "en";
                ui.set_accounts_import_status(if is_en {
                    format!("Importing tracks from {provider}…").into()
                } else {
                    format!("Імпортуємо треки з {provider}…").into()
                });
                this.submit(
                    &ui,
                    "sync_connected_accounts",
                    vec![json!(provider.as_str())],
                    false,
                );
            }
        });
        let this = self.clone();
        let weak = ui.as_weak();
        ui.on_import_all_accounts(move |providers| {
            if let Some(ui) = weak.upgrade() {
                ui.set_modal_busy(true);
                let is_en = ui.get_language() == "en";
                ui.set_accounts_import_status(if is_en {
                    "Importing tracks from all discovered accounts…".into()
                } else {
                    "Імпортуємо треки з усіх знайдених акаунтів…".into()
                });
                let args = if providers.is_empty() {
                    vec![]
                } else {
                    vec![json!(providers.as_str())]
                };
                this.submit(&ui, "sync_connected_accounts", args, false);
            }
        });
        if let Ok(history) = self.desktop.call("get_search_history", &[]) {
            if let Some(arr) = history.as_array() {
                let slint_history: Vec<slint::SharedString> = arr.iter().filter_map(|v| v.as_str()).map(Into::into).collect();
                ui.set_search_history(slint::ModelRc::new(slint::VecModel::from(slint_history)));
            }
        }
        self.submit(ui, "_refresh", vec![], true);
        self.submit(ui, "restore_session", vec![], true);
        self.submit(ui, "get_subscription_status", vec![], true);
        self.submit(ui, "get_downloads", vec![], true);
        // Тихо перевіряємо, чи на GitHub уже є новіший реліз.
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            if let Some((tag, url)) = crate::update::check() {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_update_version(tag.into());
                        ui.set_update_url(url.into());
                    }
                });
            }
        });
        let poll = slint::Timer::default();
        let this = self.clone();
        let weak = ui.as_weak();
        poll.start(
            slint::TimerMode::Repeated,
            Duration::from_secs(2),
            move || {
                if let Some(ui) = weak.upgrade() {
                    if ui.get_view() == "downloads" {
                        this.submit(&ui, "get_downloads", vec![], true);
                    }
                }
            },
        );
        UiBinding { _poll: poll }
    }
    fn track(&self, id: &str) -> Option<Value> {
        self.shared
            .data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .registry
            .get(id)
            .cloned()
    }
    fn confirm_match(&self, ui: &AppWindow, id: &str) -> Option<Value> {
        let result = confirm_selected_match(
            &mut self
                .shared
                .data
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
            id,
        );
        match result {
            Ok(track) => Some(track),
            Err(error) => {
                ui.set_modal_error(error.clone().into());
                ui.set_message(error.into());
                None
            }
        }
    }
    fn chosen_track(&self, ui: &AppWindow, id: &str) -> Option<Value> {
        if ui.get_modal_open() && ui.get_modal_kind() == "match" {
            self.confirm_match(ui, id)
        } else {
            self.track(id)
        }
    }
    fn stop_for_account(&self) {
        let _gate = self
            .shared
            .playback_gate
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        self.audio.pause();
    }
    /// Виконує команду з палітри Ctrl+K.
    fn run_command(&self, ui: &AppWindow, id: &str) {
        ui.set_command_open(false);
        let (kind, value) = id.split_once(':').unwrap_or((id, ""));
        match (kind, value) {
            ("nav", view) => ui.invoke_navigate(view.into()),
            ("action", "create-playlist") => self.action(ui, "create-playlist", ""),
            ("action", "toggle-theme") => {
                let next = if ui.get_dark() { "light" } else { "dark" };
                self.action(ui, "set-theme", next);
            }
            ("action", "cycle-accent") => {
                const ORDER: [&str; 6] = ["mono", "blue", "violet", "rose", "amber", "green"];
                let current = ui.get_accent().to_string();
                let index = ORDER.iter().position(|name| *name == current).unwrap_or(0);
                let next = ORDER[(index + 1) % ORDER.len()];
                self.action(ui, "set-accent", next);
            }
            ("action", "connected") => self.action(ui, "open-connected-accounts", ""),
            ("action", "about") => self.action(ui, "about", ""),
            ("sync", mode) => self.submit(ui, "sync_library", vec![json!(mode)], false),
            _ => {}
        }
    }
    fn action(&self, ui: &AppWindow, method: &str, payload: &str) {
        match method {
            "command-query" => {
                let catalog = command_catalog(ui.get_language().as_str());
                let query = payload.trim().to_lowercase();
                let filtered: Vec<CommandRow> = if query.is_empty() {
                    catalog
                } else {
                    catalog
                        .into_iter()
                        .filter(|item| {
                            item.title.to_lowercase().contains(&query)
                                || item.hint.to_lowercase().contains(&query)
                        })
                        .collect()
                };
                ui.set_commands(rows(filtered));
            }
            "run-command" => self.run_command(ui, payload),
            "smart-playlist" => {
                let kind = match payload {
                    "top" | "forgotten" => payload,
                    _ => "recent",
                };
                ui.set_view("smart".into());
                ui.set_smart_kind(kind.into());
                self.submit(ui, "smart_playlist", vec![json!(kind)], false);
            }
            "create-playlist" | "create_playlist" => {
                self.shared
                    .data
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .add_to_new_playlist =
                    ui.get_modal_kind() == "add-to-playlist" && ui.get_modal_open();
                reset_modal(&ui);
                ui.set_modal_kind("create-playlist".into());
                ui.set_modal_title(crate::tr!(ui, "Новий плейлист", "New playlist"));
                ui.set_modal_value("".into());
                ui.set_modal_open(true);
            }
            "rename-playlist" => {
                let data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                let id = if payload.is_empty() {
                    data.selected_playlist.as_str()
                } else {
                    payload
                };
                if id.is_empty() {
                    ui.set_message(crate::tr!(ui, "Спершу відкрийте плейлист", "Open a playlist first"));
                    return;
                }
                reset_modal(&ui);
                ui.set_modal_kind(format!("rename:{id}").into());
                ui.set_modal_title(crate::tr!(ui, "Назва плейлиста", "Playlist name"));
                ui.set_modal_value(
                    data.library["playlists"][id]["name"]
                        .as_str()
                        .unwrap_or("")
                        .into(),
                );
                ui.set_modal_open(true);
            }
            "delete-playlist" | "delete_playlist" => {
                let mut data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                let id = if payload.is_empty() {
                    data.selected_playlist.clone()
                } else {
                    payload.into()
                };
                if id.is_empty() {
                    ui.set_message(crate::tr!(ui, "Спершу відкрийте плейлист", "Open a playlist first"));
                    return;
                }
                data.pending_confirm = Some(("delete_playlist".into(), vec![json!(id)]));
                reset_modal(&ui);
                ui.set_modal_kind("confirm".into());
                ui.set_modal_title(crate::tr!(ui, "Видалити плейлист?", "Delete playlist?"));
                ui.set_modal_description(crate::tr!(
                    ui,
                    "Цю дію неможливо скасувати. Самі аудіофайли залишаться на диску.",
                    "This cannot be undone. The audio files stay on disk."
                ));
                ui.set_modal_open(true);
            }
            "remove-track" => {
                let id = self
                    .shared
                    .data
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .selected_playlist
                    .clone();
                if !id.is_empty() {
                    self.submit(
                        ui,
                        "remove_from_playlist",
                        vec![json!(id), json!(payload)],
                        false,
                    );
                }
            }
            "download-playlist" => {
                let tracks = {
                    let data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                    if data.selected_playlist.is_empty() {
                        Vec::new()
                    } else {
                        values(&data.library["playlists"][&data.selected_playlist]["tracks"])
                    }
                };
                if tracks.is_empty() {
                    ui.set_message(crate::tr!(ui, "Спершу відкрийте плейлист", "Open a playlist first"));
                } else {
                    let mut queued = 0usize;
                    for track in tracks.iter().take(200) {
                        self.submit(ui, "download_track", vec![track.clone()], true);
                        queued += 1;
                    }
                    ui.set_message(crate::tr!(
                        ui,
                        &format!("У чергу завантажень додано {queued} треків"),
                        &format!("Queued {queued} tracks for download")
                    ));
                }
            }
            "play-playlist" => {
                let first = {
                    let data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                    data.library["playlists"][&data.selected_playlist]["tracks"][0]["id"]
                        .as_str()
                        .map(str::to_owned)
                };
                if let Some(id) = first {
                    self.play_track(ui, &id, false);
                }
            }
            "download-url" => self.submit(ui, "resolve_direct_url", vec![json!(payload)], false),
            "open-downloads" => {
                self.submit(ui, "open_downloads_directory", vec![], false);
            }
            "cancel-download" | "cancel_download" | "reveal-download" | "reveal_download" => self
                .submit(
                    ui,
                    if method.starts_with("cancel") {
                        "cancel_download"
                    } else {
                        "reveal_download"
                    },
                    vec![json!(payload)],
                    false,
                ),
            "clear_history" | "clear-history" => {
                self.stop_for_account();
                self.submit(ui, "clear_history", vec![], false);
            }
            "set-icon" => {
                match crate::shortcut::set_desktop_icon(payload) {
                    Ok(count) => {
                        ui.set_desktop_icon(payload.into());
                        self.submit(
                            ui,
                            "update_settings",
                            vec![json!({"desktop_icon": payload})],
                            false,
                        );
                        ui.set_message(crate::tr!(
                            ui,
                            &format!("Іконку оновлено ({count} ярлики)"),
                            &format!("Icon updated ({count} shortcuts)")
                        ));
                    }
                    Err(error) => ui.set_message(error.into()),
                }
            }
            "theme" | "set-theme" => {
                let scheme = match payload {
                    "dark" | "rose" | "violet" | "ocean" => payload,
                    _ => "light",
                };
                ui.set_scheme(scheme.into());
                self.submit(ui, "update_settings", vec![json!({"theme":scheme})], false)
            }
            "set-speed" => {
                let speed = payload.parse::<f32>().unwrap_or(1.0).clamp(0.5, 2.0);
                if let Ok(mut applied) = self.shared.audio_applied.lock() {
                    applied.0 = speed;
                }
                self.audio.set_speed(speed);
                ui.set_speed(speed);
                self.submit(
                    ui,
                    "update_settings",
                    vec![json!({"playback_speed": speed})],
                    true,
                );
                ui.set_message(crate::tr!(
                    ui,
                    &format!("Швидкість відтворення: {speed}×"),
                    &format!("Playback speed: {speed}×")
                ));
            }
            "set-volume-norm" => {
                let enabled = payload == "true";
                if let Ok(mut applied) = self.shared.audio_applied.lock() {
                    applied.1 = enabled;
                }
                self.audio.set_normalize(enabled);
                ui.set_volume_norm(enabled);
                self.submit(
                    ui,
                    "update_settings",
                    vec![json!({"volume_norm_enabled": enabled})],
                    true,
                );
            }
            "set-accent" => {
                let accent = match payload {
                    "blue" | "violet" | "rose" | "amber" | "green" => payload,
                    _ => "mono",
                };
                ui.set_accent(accent.into());
                self.submit(ui, "update_settings", vec![json!({"accent":accent})], true)
            }
            "language" | "set-language" => {
                self.submit(ui, "update_settings", vec![json!({"language":payload})], false)
            }
            "reduced-motion" | "set-reduced-motion" => self.submit(
                ui,
                "update_settings",
                vec![json!({"reduced_motion":payload == "true"})],
                false,
            ),
            "set-close-to-tray" => self.submit(
                ui,
                "update_settings",
                vec![json!({"tray_enabled":payload=="true"})],
                false,
            ),
            "set-media-hotkeys" => self.submit(
                ui,
                "update_settings",
                vec![json!({"hotkeys_enabled":payload=="true"})],
                false,
            ),
            "set-infinite-autoplay" => self.submit(
                ui,
                "update_settings",
                vec![json!({"infinite_autoplay":payload=="true"})],
                false,
            ),
            "set-discord-rpc" => {
                let enabled = payload == "true";
                self.discord.set_enabled(enabled);
                self.shared.discord_rpc.store(enabled, Ordering::Relaxed);
                self.submit(
                    ui,
                    "update_settings",
                    vec![json!({"discord_rpc_enabled": enabled})],
                    false,
                );
            }
            "update-profile" => {
                self.submit(ui, "update_profile", vec![json!({"name":payload})], false)
            }
            "export" | "export-library" | "export_library_file" => {
                self.submit(ui, "export_library_file", vec![], false)
            }
            "import" | "import-library" | "import_library_file" => {
                self.stop_for_account();
                self.submit(ui, "import_library_file", vec![json!(true)], false)
            }
            "scan" | "scan-local" | "scan_local_music" => {
                self.submit(ui, "scan_local_music", vec![], false)
            }
            "legacy-import" | "import_legacy_library" => {
                self.stop_for_account();
                self.submit(ui, "import_legacy_library", vec![], false)
            }
            "import-provider" => {
                reset_modal(&ui);
                ui.set_modal_kind(format!("import:{payload}").into());
                ui.set_modal_title(crate::tr!(ui, "Імпорт музики", "Import music"));
                ui.set_modal_description(crate::tr!(
                    ui,
                    "Вставте публічне посилання на профіль або плейлист",
                    "Paste a public profile or playlist link"
                ));
                ui.set_modal_value("".into());
                ui.set_modal_open(true);
            }
            "import-history" | "import-listening" => {
                let provider = if payload.is_empty() {
                    self.shared
                        .data
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .stats_source
                        .clone()
                } else {
                    payload.into()
                };
                if provider == "all" || provider == "local" {
                    ui.set_message(crate::tr!(
                        ui,
                        "Оберіть Spotify, SoundCloud або YouTube для імпорту історії",
                        "Choose Spotify, SoundCloud or YouTube to import the history"
                    ));
                } else {
                    self.submit(
                        ui,
                        "import_listening_history_file",
                        vec![json!(provider)],
                        false,
                    );
                }
            }
            "checkout" | "create_subscription_checkout" => {
                self.submit(ui, "create_subscription_checkout", vec![], false)
            }
            "refresh-subscription" => self.submit(ui, "get_subscription_status", vec![], false),
            "open-subscription-modal" => {
                reset_modal(ui);
                ui.set_modal_kind("subscription-plans".into());
                if ui.get_subscription_active() {
                    ui.set_modal_title(crate::tr!(ui, "Керування підпискою SoundFlow Plus ⭐", "SoundFlow Plus subscription ⭐"));
                    ui.set_modal_description(crate::tr!(ui, "Ваша підписка активна. Ви можете змінити тарифний план або скасувати підписку:", "Your subscription is active. Change the plan or cancel it:"));
                } else {
                    ui.set_modal_title(crate::tr!(ui, "Підписка SoundFlow Plus ⭐", "SoundFlow Plus ⭐"));
                    ui.set_modal_description(crate::tr!(ui, "Отримай повний доступ до детальної статистики прослуховування, ритму та улюблених виконавців:", "Get full access to detailed listening stats, rhythm and favourite artists:"));
                }
                ui.set_modal_open(true);
            }
            "open-promo-modal" => {
                reset_modal(ui);
                ui.set_modal_kind("promo-code".into());
                ui.set_modal_title(crate::tr!(ui, "Активація промокоду SoundFlow Plus", "Activate a SoundFlow Plus promo code"));
                ui.set_modal_description(crate::tr!(ui, "Введи промокод (наприклад: PLUS, VIP, SOUNDFLOW, FREE) або отримай Plus безкоштовно:", "Enter a promo code (e.g. PLUS, VIP, SOUNDFLOW, FREE) or unlock Plus for free:"));
                ui.set_modal_value("".into());
                ui.set_modal_open(true);
            }
            "open-accounts-modal" => {
                reset_modal(ui);
                ui.set_modal_kind("accounts-list".into());
                ui.set_modal_title(crate::tr!(ui, "Керування акаунтами SoundFlow", "SoundFlow account management"));
                ui.set_modal_description(crate::tr!(ui, "Обери акаунт для перемикання або додай новий:", "Choose an account to switch or add a new one:"));
                ui.set_modal_open(true);
            }
            "open-create-account-modal" => {
                reset_modal(ui);
                ui.set_modal_kind("create-account:".into());
                ui.set_modal_title(crate::tr!(ui, "Створити новий акаунт", "Create a new account"));
                ui.set_modal_description(crate::tr!(ui, "Введи ім'я або email для нового локального акаунта:", "Enter a name or email for the new local account:"));
                ui.set_modal_value("".into());
                ui.set_modal_open(true);
            }
            "buy-plan" => {
                self.submit(ui, "buy_subscription", vec![json!(payload)], false);
            }
            "grant-subscription" | "apply-promo" => {
                self.submit(ui, "grant_subscription", vec![json!(payload)], false);
            }
            "complete-onboarding" => {
                ui.set_show_onboarding(false);
                self.submit(ui, "update_settings", vec![json!({"onboarding_completed": true})], false);
            }
            "confirm-cancel-subscription" | "cancel-subscription" | "cancel_subscription" => {
                self.submit(ui, "cancel_subscription", vec![], false);
            }
            "switch-account-id" => {
                self.stop_for_account();
                self.submit(ui, "switch_account", vec![json!(payload)], false);
            }
            "open-url" | "open_external_url" => {
                self.submit(ui, "open_external_url", vec![json!(payload)], false)
            }
            "stats-period" | "stats-source" => {
                let args = {
                    let mut data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                    if method == "stats-period" {
                        data.stats_period = payload.into();
                    } else {
                        data.stats_source = payload.into();
                    }
                    vec![json!(data.stats_period), json!(data.stats_source)]
                };
                self.submit(ui, "get_listening_stats", args, false);
            }
            "paste-clipboard" => {
                if let Some(text) = crate::platform::clipboard_text() {
                    let text = text.replace(['\r', '\n'], " ").trim().to_string();
                    if !text.is_empty() {
                        if ui.get_modal_open() {
                            let current = ui.get_modal_value().to_string();
                            ui.set_modal_value(format!("{current}{text}").into());
                        } else if ui.get_command_open() {
                            let next = format!("{}{}", ui.get_command_query(), text);
                            ui.set_command_query(next.clone().into());
                            self.action(ui, "command-query", &next);
                        } else {
                            let next = format!("{}{}", ui.get_search_query(), text);
                            ui.set_search_query(next.into());
                        }
                    }
                }
            }
            "dismiss-message" => ui.set_message("".into()),
            "remove-search-query" => {
                let _ = self.desktop.call("remove_search_query", &[json!(payload)]);
                if let Ok(history) = self.desktop.call("get_search_history", &[]) {
                    if let Some(arr) = history.as_array() {
                        let slint_history: Vec<slint::SharedString> = arr.iter().filter_map(|v| v.as_str()).map(Into::into).collect();
                        ui.set_search_history(slint::ModelRc::new(slint::VecModel::from(slint_history)));
                    }
                }
            }
            "clear-search-history" => {
                let _ = self.desktop.call("clear_search_history", &[]);
                ui.set_search_history(slint::ModelRc::new(slint::VecModel::from(vec![])));
            }
            "refresh" => self.submit(ui, "_refresh", vec![], false),
            "open-release" => {
                if !payload.is_empty() {
                    let _ = open::that_detached(payload);
                }
            }
            "about" => {
                reset_modal(ui);
                ui.set_modal_kind("about".into());
                ui.set_modal_title("SoundFlow".into());
                ui.set_modal_description(crate::tr!(ui, "Нативний музичний застосунок. Rust · Slint · локальне аудіо. Статистика Plus потребує налаштованого сервера й активної підписки.", "Native music application. Rust · Slint · local audio. Plus statistics need a configured server and an active subscription."));
                ui.set_modal_open(true);
            }
            "open-connected-accounts" => {
                let (email, name) = {
                    let data = self.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                    let email = data.cloud["user"]["email"]
                        .as_str()
                        .or_else(|| data.library["profile"]["email"].as_str())
                        .unwrap_or("")
                        .to_owned();
                    let name = data.cloud["user"]["name"]
                        .as_str()
                        .or_else(|| data.library["profile"]["name"].as_str())
                        .unwrap_or("")
                        .to_owned();
                    (email, name)
                };
                self.submit(
                    ui,
                    "detect_connected_accounts",
                    vec![json!(email), json!(name)],
                    false,
                );
            }
            value if value.starts_with("import:") => {
                reset_modal(ui);
                ui.set_modal_kind(value.into());
                ui.set_modal_title(crate::tr!(ui, "Імпорт музики", "Import music"));
                ui.set_modal_value("".into());
                ui.set_modal_open(true);
            }
            _ => ui.set_message(crate::tr!(ui, "Ця дія недоступна", "This action is not available")),
        }
    }
    fn modal_submit(&self, ui: &AppWindow, value: &str) {
        let kind = ui.get_modal_kind();
        ui.set_modal_busy(true);
        ui.set_modal_error("".into());
        if kind == "create-playlist" {
            self.submit(ui, "create_playlist", vec![json!(value)], false);
        } else if let Some(id) = kind.strip_prefix("rename:") {
            self.submit(ui, "rename_playlist", vec![json!(id), json!(value)], false);
        } else if let Some(provider) = kind.strip_prefix("import:") {
            self.submit(
                ui,
                "import_external_profile",
                vec![json!(provider), json!(value)],
                false,
            );
        } else if let Some(provider) = kind.strip_prefix("connect:") {
            self.submit(
                ui,
                "set_connected_account",
                vec![json!(provider), json!(value)],
                false,
            );
        } else if let Some(_) = kind.strip_prefix("create-account:") {
            let name = value.trim();
            let email = format!("{}@soundflow.local", name.to_lowercase().replace(' ', "_"));
            self.submit(
                ui,
                "sign_up",
                vec![json!({"name": name, "email": email, "password": "password"})],
                false,
            );
        } else if kind == "promo-code" {
            self.submit(ui, "grant_subscription", vec![json!(value)], false);
        } else if let Some(_) = kind.strip_prefix("google-email:") {
            let email = value.trim();
            let name = email.split('@').next().unwrap_or("user");
            self.submit(
                ui,
                "google_sign_in",
                vec![json!(email), json!(name), json!("")],
                false,
            );
            self.submit(
                ui,
                "update_settings",
                vec![json!({"onboarding_completed": true})],
                false,
            );
            ui.set_show_onboarding(false);
        } else if let Some(_) = kind.strip_prefix("github-account:") {
            let handle = value.trim();
            let name = handle.split('@').next().unwrap_or("github_user");
            self.submit(
                ui,
                "github_sign_in",
                vec![json!(handle), json!(name)],
                false,
            );
            self.submit(
                ui,
                "update_settings",
                vec![json!({"onboarding_completed": true})],
                false,
            );
            ui.set_show_onboarding(false);
        } else if kind == "confirm" {
            let pending = self
                .shared
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pending_confirm
                .take();
            if let Some((method, args)) = pending {
                self.submit(ui, &method, args, false);
                close_modal(&ui);
            } else {
                ui.set_modal_busy(false);
            }
        } else {
            ui.set_modal_busy(false);
        }
    }
    fn start_workers(&self, ui: &AppWindow) {
        let Some(receiver) = self.receiver.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return;
        };
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..4 {
            let receiver = receiver.clone();
            let this = self.clone();
            let weak = ui.as_weak();
            std::thread::spawn(move || loop {
                let command = { receiver.lock().unwrap_or_else(|e| e.into_inner()).recv() };
                let Ok(command) = command else {
                    break;
                };
                if matches!(
                    command.method.as_str(),
                    "configure_cloud"
                        | "restore_session"
                        | "sign_in"
                        | "sign_up"
                        | "sign_out"
                        | "clear_history"
                        | "import_library"
                        | "import_library_file"
                        | "import_legacy_library"
                ) {
                    {
                        let _gate = this
                            .shared
                            .playback_gate
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        this.shared.generation.fetch_add(1, Ordering::SeqCst);
                        this.audio.pause();
                    }
                    if let Some(report) = this.audio.drain() {
                        let _ = this.desktop.record_audio(&report);
                    }
                    this.audio.stop();
                }
                let result = match command.method.as_str() {
                    "_refresh" => Ok(Value::Null),
                    "_flow_like" => this
                        .desktop
                        .call("is_liked", &[command.args[0]["id"].clone()])
                        .and_then(|liked| {
                            if liked == true {
                                Ok(json!({"is_liked":true}))
                            } else {
                                this.desktop.call("toggle_like", &command.args)
                            }
                        }),
                    "_play"
                        if command.generation != this.shared.generation.load(Ordering::SeqCst) =>
                    {
                        Ok(Value::Null)
                    }
                    "_play" => this
                        .desktop
                        .call("get_stream", &[command.args[0].clone()])
                        .and_then(|source| {
                            let _gate = this
                                .shared
                                .playback_gate
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            if command.generation == this.shared.generation.load(Ordering::SeqCst) {
                                let user = command.args[3].as_str().unwrap_or("guest").to_owned();
                                this.audio.set_identity(user);
                                this.audio.load(
                                    command.args[0].clone(),
                                    source.as_str().ok_or("Недійсне джерело аудіо")?.into(),
                                    command.args[1].as_f64().unwrap_or(0.0),
                                    if command.args[2] == true {
                                        Some(30.0)
                                    } else {
                                        None
                                    },
                                );
                            }
                            Ok(Value::Null)
                        }),
                    _ => this.desktop.call(&command.method, &command.args),
                };
                let snapshot = {
                    let core = this.desktop.core.lock().unwrap_or_else(|e| e.into_inner());
                    json!({"library":core.library.snapshot(),"settings":core.library.data.settings,"cloud":core.account_status(),"user":core.user(),"version":this.shared.snapshot_sequence.fetch_add(1,Ordering::SeqCst)+1})
                };
                if !command.quiet {
                    this.shared.pending.fetch_sub(1, Ordering::Relaxed);
                }
                let app = this.clone();
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    if account_method(&command.method) {
                        app.shared.account_pending.fetch_sub(1, Ordering::SeqCst);
                    }
                    if command.method == "_play"
                        && command.generation != app.shared.generation.load(Ordering::SeqCst)
                    {
                        ui.set_task_loading(app.shared.pending.load(Ordering::Relaxed) > 0);
                        return;
                    }
                    apply_response(
                        &ui,
                        &app,
                        &command.method,
                        &command.args,
                        result,
                        snapshot,
                        &command.scope,
                        command.quiet,
                    );
                });
            });
        }
        if let Some(events) = self.audio_events.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let this = self.clone();
            let weak = ui.as_weak();
            let (reports, report_receiver) = mpsc::sync_channel(64);
            let desktop = this.desktop.clone();
            std::thread::spawn(move || {
                while let Ok(report) = report_receiver.recv() {
                    let _ = desktop.record_audio(&report);
                }
            });
            std::thread::spawn(move || {
                while let Ok(event) = events.recv() {
                    match event {
                        AudioEvent::Listening {
                            track,
                            user_id,
                            session_id,
                            started_at,
                            heard_seconds,
                            history_due,
                        } => {
                            let _ = reports.try_send(ListeningReport {
                                track,
                                user_id,
                                session_id,
                                started_at,
                                heard_seconds,
                                history_due,
                            });
                        }
                        AudioEvent::Finished => {
                            this.discord.clear();
                            let this = this.clone();
                            let _ = weak.upgrade_in_event_loop(move |ui| {
                                let (repeat, preview, has_next, infinite_autoplay) = {
                                    let data =
                                        this.shared.data.lock().unwrap_or_else(|e| e.into_inner());
                                    (
                                        data.repeat,
                                        data.preview,
                                        data.shuffle || data.queue_index + 1 < data.queue.len(),
                                        data.settings["infinite_autoplay"].as_bool().unwrap_or(true),
                                    )
                                };
                                if preview || repeat == 1 || (repeat == 0 && has_next) {
                                    this.next(&ui, 1);
                                } else if repeat == 2 {
                                    let id = ui.get_current_track().id;
                                    this.play_track_in(&ui, id.as_str(), false, true);
                                } else if repeat == 0 && !has_next && infinite_autoplay {
                                    this.advance_infinite_radio(&ui);
                                }
                            });
                        }
                        AudioEvent::Error(message) => {
                            this.discord.clear();
                            let _ = weak.upgrade_in_event_loop(move |ui| {
                                ui.set_loading(false);
                                ui.set_message(message.into());
                            });
                        }
                        AudioEvent::State(state) => {
                            let shared = this.shared.clone();
                            let mut duration = state.duration;
                            if duration <= 0.5 {
                                if let Some(t) = state.track.as_ref() {
                                    duration = crate::playback::track_duration(t);
                                }
                            }
                            if this.shared.discord_rpc.load(Ordering::Relaxed) {
                                this.discord.update(
                                    state.track.as_ref(),
                                    state.playing,
                                    state.position,
                                    duration,
                                );
                            } else {
                                this.discord.clear();
                            }
                            let _ = weak.upgrade_in_event_loop(move |ui| {
                                let playing_track = state.track.unwrap_or(Value::Null);
                                ui.set_player_accent(accent_for(&playing_track, &shared.cache));
                                ui.set_current_track(track_row(&playing_track, &shared));
                ui.invoke_track_changed(
                    text(&playing_track, "title").into(),
                    text(&playing_track, "artist").into(),
                );
                                ui.set_playing(state.playing);
                                ui.set_loading(state.loading);
                                ui.set_position(state.position as f32);
                                ui.set_duration(duration as f32);
                                ui.set_volume(state.volume);
                                let data = shared.data.lock().unwrap_or_else(|e| e.into_inner());
                                ui.set_flow_playing(data.preview && state.playing);
                                ui.set_flow_progress(if data.preview {
                                    ((state.position - data.preview_start) / 30.0).clamp(0.0, 1.0)
                                        as f32
                                } else {
                                    0.0
                                });
                            });
                        }
                    }
                }
            });
        }
    }
    pub fn shutdown(&self) {
        self.discord.shutdown();
        if let Some(report) = self.audio.drain() {
            let desktop = self.desktop.clone();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = desktop.record_audio(&report);
                let _ = tx.send(());
            });
            let _ = rx.recv_timeout(Duration::from_millis(500));
        }
        self.audio.shutdown();
        self.desktop.shutdown();
    }
}

fn confirm_selected_match(data: &mut UiData, id: &str) -> Result<Value, String> {
    let original = data
        .pending_track
        .as_ref()
        .ok_or("Спершу оберіть пісню Spotify")?;
    let original_id = original["id"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("Недійсна пісня Spotify")?;
    if original["source"] != "spotify" || data.matches_owner != original_id {
        return Err("Добірка версій змінилася. Оберіть пісню повторно.".into());
    }
    let mut candidate = data
        .matches
        .iter()
        .find(|track| track["id"] == id)
        .cloned()
        .ok_or("Оберіть одну з показаних версій")?;
    if candidate["matched_from"]["id"] != original_id
        || candidate["matched_from"]["source"] != "spotify"
        || !matches!(candidate["source"].as_str(), Some("youtube" | "soundcloud"))
    {
        return Err("Ця версія не відповідає обраній пісні Spotify".into());
    }
    candidate["match_confirmed"] = json!(true);
    data.registry.insert(id.into(), candidate.clone());
    if let Some(track) = data.matches.iter_mut().find(|track| track["id"] == id) {
        *track = candidate.clone();
    }
    Ok(candidate)
}
fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_string()
}
fn reset_modal(ui: &AppWindow) {
    ui.set_modal_description("".into());
    ui.set_modal_error("".into());
    ui.set_modal_value("".into());
    ui.set_modal_busy(false);
}
fn close_modal(ui: &AppWindow) {
    ui.set_modal_open(false);
    reset_modal(ui);
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
    matches!(
        method,
        "toggle_like"
            | "_flow_like"
            | "create_playlist"
            | "rename_playlist"
            | "delete_playlist"
            | "add_to_playlist"
            | "remove_from_playlist"
            | "update_settings"
            | "update_profile"
            | "clear_history"
            | "import_library"
            | "import_library_file"
            | "import_legacy_library"
            | "import_listening_history_file"
            | "import_external_profile"
            | "scan_local_music"
            | "sync_library"
            | "sync_connected_accounts"
            | "set_connected_account"
            | "buy_subscription"
            | "grant_subscription"
            | "cancel_subscription"
            | "github_sign_in"
    )
}
fn rows<T: Clone + 'static>(values: Vec<T>) -> ModelRc<T> {
    std::rc::Rc::new(VecModel::from(values)).into()
}
fn values(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}
fn artwork(value: &Value, cache: &Path) -> slint::Image {
    let url = text(value, "thumbnail");
    if url.is_empty() {
        return slint::Image::default();
    }
    let id = format!("{:x}", Sha256::digest(url.as_bytes()));
    for extension in ["jpg", "png", "webp"] {
        let file = cache.join(format!("{id}.{extension}"));
        if file.is_file() {
            if let Ok(image) = slint::Image::load_from_path(&file) {
                return image;
            }
        }
    }
    slint::Image::default()
}

/// Середній насичений колір обкладинки для градієнта плеєра.
fn accent_for(value: &Value, cache: &Path) -> slint::Color {
    let url = text(value, "thumbnail");
    if url.is_empty() {
        return slint::Color::from_argb_f32(0.0, 0.0, 0.0, 0.0);
    }
    let id = format!("{:x}", Sha256::digest(url.as_bytes()));
    for extension in ["jpg", "png", "webp"] {
        let file = cache.join(format!("{id}.{extension}"));
        if !file.is_file() {
            continue;
        }
        let Ok(image) = image::open(&file) else { continue };
        let small = image
            .resize(14, 14, image::imageops::FilterType::Triangle)
            .to_rgb8();
        let (mut r, mut g, mut b, mut weight) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for pixel in small.pixels() {
            let (pr, pg, pb) = (
                pixel[0] as f32 / 255.0,
                pixel[1] as f32 / 255.0,
                pixel[2] as f32 / 255.0,
            );
            let max = pr.max(pg).max(pb);
            let min = pr.min(pg).min(pb);
            let saturation = if max <= f32::EPSILON { 0.0 } else { (max - min) / max };
            // Насичені пікселі важливіші за сірі, темні — трохи підсвічуємо.
            let w = 0.15 + saturation;
            r += pr * w;
            g += pg * w;
            b += pb * w;
            weight += w;
        }
        if weight <= f32::EPSILON {
            break;
        }
        let (mut r, mut g, mut b) = (r / weight, g / weight, b / weight);
        let max = r.max(g).max(b);
        if max < 0.35 {
            let boost = 0.35 / max.max(0.05);
            r = (r * boost).min(1.0);
            g = (g * boost).min(1.0);
            b = (b * boost).min(1.0);
        }
        return slint::Color::from_argb_f32(0.22, r, g, b);
    }
    slint::Color::from_argb_f32(0.0, 0.0, 0.0, 0.0)
}

/// Команди для палітри Ctrl+K. Заголовки локалізуються під час відкриття.
fn command_catalog(language: &str) -> Vec<CommandRow> {
    let uk = language != "en";
    let section = if uk { "Розділ" } else { "Section" };
    let action = if uk { "Дія" } else { "Action" };
    let sync = if uk { "Хмара" } else { "Cloud" };
    let mut items = Vec::new();
    let mut push = |id: &str, title_uk: &str, title_en: &str, hint: &str| {
        items.push(CommandRow {
            id: id.into(),
            title: (if uk { title_uk } else { title_en }).into(),
            hint: hint.into(),
        });
    };
    push("nav:home", "Головна", "Home", section);
    push("nav:search", "Пошук", "Search", section);
    push("nav:flow", "Flow", "Flow", section);
    push("nav:stats", "Статистика", "Statistics", section);
    push("nav:liked", "Улюблене", "Liked", section);
    push("nav:history", "Історія", "History", section);
    push("nav:downloads", "Завантаження", "Downloads", section);
    push("nav:connections", "Підключення", "Connections", section);
    push("nav:settings", "Налаштування", "Settings", section);
    push("action:create-playlist", "Створити плейлист", "Create playlist", action);
    push(
        "action:toggle-theme",
        "Перемкнути світлу/темну тему",
        "Toggle light/dark theme",
        action,
    );
    push(
        "action:cycle-accent",
        "Змінити акцентний колір",
        "Cycle accent colour",
        action,
    );
    push("action:connected", "Підключені акаунти", "Connected accounts", action);
    push("action:about", "Про SoundFlow", "About SoundFlow", action);
    push("sync:push", "Надіслати бібліотеку в хмару", "Push library to cloud", sync);
    push("sync:pull", "Отримати бібліотеку з хмари", "Pull library from cloud", sync);
    items
}

fn track_row(value: &Value, shared: &Shared) -> TrackRow {
    let liked = shared
        .data
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .library["liked_tracks"]
        .as_array()
        .is_some_and(|tracks| tracks.iter().any(|t| t["id"] == value["id"]));
    let duration = value["duration"].as_f64().unwrap_or(0.0) as u64;
    TrackRow {
        id: text(value, "id").into(),
        title: text(value, "title").into(),
        artist: text(value, "artist").into(),
        source: text(value, "source").into(),
        duration: if duration > 0 {
            format!("{}:{:02}", duration / 60, duration % 60).into()
        } else {
            SharedString::default()
        },
        liked,
        offline: shared
            .offline
            .lock()
            .map(|set| set.contains(value["id"].as_str().unwrap_or("")))
            .unwrap_or(false),
        artwork: artwork(value, &shared.cache),
    }
}
fn refresh_models(ui: &AppWindow, shared: &Shared) {
    let (tracks, playlists, downloads, flow, index, count, cloud, settings, profile, matches, search_history) = {
        let data = shared.data.lock().unwrap_or_else(|e| e.into_inner());
        let tracks = match ui.get_view().as_str() {
            "search" => data.search.clone(),
            "smart" => data.smart.clone(),
            "likes" | "liked" => values(&data.library["liked_tracks"]),
            "home" => {
                if data.home.is_empty() {
                    values(&data.library["history"])
                } else {
                    data.home.clone()
                }
            }
            "history" => values(&data.library["history"]),
            "playlist" | "library" => {
                if !data.selected_playlist.is_empty() {
                    values(&data.library["playlists"][&data.selected_playlist]["tracks"])
                } else {
                    let mut tracks = values(&data.library["liked_tracks"]);
                    if let Some(playlists) = data.library["playlists"].as_object() {
                        for playlist in playlists.values() {
                            tracks.extend(values(&playlist["tracks"]));
                        }
                    }
                    let mut seen = std::collections::HashSet::new();
                    tracks.retain(|track| {
                        seen.insert(format!("{}:{}", text(track, "source"), text(track, "id")))
                    });
                    tracks
                }
            }
            _ => values(&data.library["liked_tracks"]),
        };
        (
            tracks,
            data.library["playlists"]
                .as_object()
                .map(|p| p.values().cloned().collect::<Vec<_>>())
                .unwrap_or_default(),
            data.downloads.clone(),
            data.flow
                .get(data.flow_index)
                .cloned()
                .unwrap_or(Value::Null),
            data.flow_index,
            data.flow.len(),
            data.cloud.clone(),
            data.settings.clone(),
            data.library["profile"].clone(),
            if ui.get_modal_kind() == "match"
                && data
                    .pending_track
                    .as_ref()
                    .is_none_or(|track| text(track, "id") != data.matches_owner)
            {
                Vec::new()
            } else {
                data.matches.clone()
            },
            data.library
                .get("search_history")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        )
    };
    let slint_history: Vec<slint::SharedString> = search_history
        .iter()
        .filter_map(|v| v.as_str())
        .map(Into::into)
        .collect();
    ui.set_search_history(rows(slint_history));
    ui.set_tracks(rows(tracks.iter().map(|t| track_row(t, shared)).collect()));
    ui.set_playlists(rows(
        playlists
            .iter()
            .map(|p| PlaylistRow {
                id: text(p, "id").into(),
                name: text(p, "name").into(),
                count: p["tracks"].as_array().map(|a| a.len()).unwrap_or(0) as i32,
                artwork: artwork(&p["tracks"][0], &shared.cache),
            })
            .collect(),
    ));
    ui.set_downloads(rows(
        downloads
            .iter()
            .map(|p| DownloadRow {
                id: p
                    .get("job_id")
                    .or_else(|| p.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                title: p
                    .get("title")
                    .or_else(|| p.get("track").and_then(|t| t.get("title")))
                    .and_then(Value::as_str)
                    .unwrap_or("Завантаження")
                    .into(),
                artist: p
                    .get("artist")
                    .or_else(|| p.get("track").and_then(|t| t.get("artist")))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                status: text(p, "status").into(),
                details: p
                    .get("error")
                    .or_else(|| p.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                progress: p["progress"].as_f64().unwrap_or(0.0) as f32,
                cancellable: matches!(
                    p["status"].as_str(),
                    Some("queued" | "downloading" | "resolving")
                ),
                artwork: artwork(&p["track"], &shared.cache),
            })
            .collect(),
    ));
    ui.set_flow_track(track_row(&flow, shared));
    ui.set_flow_index(index as i32);
    ui.set_flow_count(count as i32);
    ui.set_match_tracks(rows(matches.iter().map(|t| track_row(t, shared)).collect()));
    ui.set_profile_name(
        profile["name"]
            .as_str()
            .or_else(|| profile["username"].as_str())
            .unwrap_or("Слухач")
            .into(),
    );
    ui.set_cloud_configured(cloud["configured"] == true);
    ui.set_supabase_linked(crate::supabase_auth::current().is_some());
    ui.set_cloud_signed_in(cloud["signed_in"] == true);
    ui.set_cloud_url(cloud["base_url"].as_str().unwrap_or("").into());
    ui.set_cloud_name(cloud["user"]["name"].as_str().unwrap_or("").into());
    ui.set_cloud_email(cloud["user"]["email"].as_str().unwrap_or("").into());
    if let Some(sub) = cloud.get("subscription") {
        let active = sub["active"] == true;
        ui.set_subscription_active(active);
        let msg = sub["expires_formatted"]
            .as_str()
            .or_else(|| sub["message"].as_str())
            .unwrap_or("");
        ui.set_subscription_message(msg.into());
    }
    if let Some(accs) = cloud["accounts"].as_array() {
        let account_rows: Vec<AccountRow> = accs
            .iter()
            .map(|a| AccountRow {
                id: a["id"].as_str().unwrap_or("").into(),
                name: a["name"].as_str().unwrap_or("").into(),
                email: a["email"].as_str().unwrap_or("").into(),
                is_active: a["is_active"].as_bool().unwrap_or(false),
                is_plus: a["is_plus"].as_bool().unwrap_or(false),
                expires_formatted: a["expires_formatted"].as_str().unwrap_or("").into(),
            })
            .collect();
        ui.set_accounts(rows(account_rows));
    }
    ui.set_dark(settings["theme"] == "dark");
    ui.set_reduced_motion(settings["reduced_motion"] == true);
    let close = settings["tray_enabled"].as_bool().unwrap_or(true);
    let hotkeys = settings["hotkeys_enabled"].as_bool().unwrap_or(true);
    shared.close_to_tray.store(close, Ordering::Relaxed);
    shared.media_hotkeys.store(hotkeys, Ordering::Relaxed);
    ui.set_close_to_tray(close);
    ui.set_media_hotkeys(hotkeys);
    ui.set_accent(settings["accent"].as_str().unwrap_or("mono").into());
    let scheme = settings["theme"].as_str().unwrap_or("light");
    ui.set_scheme(scheme.into());
    ui.set_desktop_icon(settings["desktop_icon"].as_str().unwrap_or("default").into());
    ui.set_speed(settings["playback_speed"].as_f64().unwrap_or(1.0) as f32);
    ui.set_volume_norm(settings["volume_norm_enabled"].as_bool().unwrap_or(true));
    ui.set_infinite_autoplay(settings["infinite_autoplay"].as_bool().unwrap_or(true));
    let discord = settings["discord_rpc_enabled"].as_bool().unwrap_or(true);
    shared.discord_rpc.store(discord, Ordering::Relaxed);
    ui.set_discord_rpc(discord);
    let lang = settings["language"].as_str().unwrap_or("uk");
    ui.set_language(lang.into());
    let onboarding_done = settings["onboarding_completed"].as_bool().unwrap_or(false);
    let signed_in = cloud["signed_in"] == true;
    ui.set_show_onboarding(!onboarding_done && !signed_in);
    {
        let data = shared.data.lock().unwrap_or_else(|e| e.into_inner());
        ui.set_shuffle(data.shuffle);
        ui.set_repeat_mode(["off", "all", "one"][data.repeat as usize].into());
    }
    let current = ui.get_current_track().id;
    if !current.is_empty() {
        let value = {
            shared
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .registry
                .get(current.as_str())
                .cloned()
        };
        if let Some(value) = value {
            ui.set_player_accent(accent_for(&value, &shared.cache));
    ui.set_current_track(track_row(&value, shared));
        ui.invoke_track_changed(text(&value, "title").into(), text(&value, "artist").into());
        }
    }
}
fn apply_response(
    ui: &AppWindow,
    app: &Application,
    method: &str,
    args: &[Value],
    result: Result<Value, String>,
    snapshot: Value,
    scope: &str,
    quiet: bool,
) {
    let shared = &app.shared;
    let audio = &app.audio;
    {
        let mut data = shared.data.lock().unwrap_or_else(|e| e.into_inner());
        let user = snapshot["user"].as_str().unwrap_or("guest");
        let version = snapshot["version"].as_u64().unwrap_or(0);
        if version < data.snapshot_version && user != data.user {
            ui.set_task_loading(shared.pending.load(Ordering::Relaxed) > 0);
            return;
        }
        if version >= data.snapshot_version {
            if user != data.user {
                data.registry.clear();
                data.queue.clear();
                data.search.clear();
                data.flow.clear();
                data.home.clear();
                data.matches.clear();
                data.selected_playlist.clear();
                data.stats = Value::Null;
                ui.set_subscription_active(false);
                ui.set_metrics(rows(Vec::new()));
                ui.set_artists(rows(Vec::new()));
                ui.set_top_tracks(rows(Vec::new()));
                ui.set_providers(rows(Vec::new()));
                ui.set_daily(rows(Vec::new()));
            }
            data.library = snapshot["library"].clone();
            data.settings = snapshot["settings"].clone();
            data.cloud = snapshot["cloud"].clone();
            data.user = user.into();
            data.snapshot_version = version;
        }
        let mut tracks = values(&data.library["liked_tracks"]);
        tracks.extend(values(&data.library["history"]));
        if let Some(playlists) = data.library["playlists"].as_object() {
            for playlist in playlists.values() {
                tracks.extend(values(&playlist["tracks"]));
            }
        }
        for track in tracks {
            data.registry.insert(text(&track, "id"), track);
        }
    }
    ui.set_task_loading(shared.pending.load(Ordering::Relaxed) > 0);
    if matches!(
        method,
        "create_playlist"
            | "rename_playlist"
            | "import_external_profile"
            | "delete_playlist"
            | "detect_connected_accounts"
            | "set_connected_account"
            | "sync_connected_accounts"
    ) {
        ui.set_modal_busy(false);
    }
    ui.set_message_error(false);
    match result {
        Err(error) => {
            if !quiet {
                ui.set_message_error(true);
                ui.set_message(error.clone().into());
                if matches!(
                    method,
                    "create_playlist"
                        | "rename_playlist"
                        | "import_external_profile"
                        | "delete_playlist"
                        | "match_spotify_track"
                ) {
                    ui.set_modal_error(error.into());
                }
            }
            if method == "_play" {
                ui.set_loading(false);
            }
            // Якщо імпорт запускали з питання після Google-входу — прибираємо діалог,
            // текст помилки вже показано в тості.
            if matches!(method, "sync_connected_accounts") && ui.get_modal_kind() == "import-prompt" {
                close_modal(ui);
            }
        }
        Ok(value) => {
            let mut data = shared.data.lock().unwrap_or_else(|e| e.into_inner());
            match method {
                "smart_playlist" => {
                    data.smart = values(&value);
                }
                "search" => {
                    if ui.get_search_query().as_str() == args[0].as_str().unwrap_or("")
                        && ui.get_search_source().as_str() == args[1].as_str().unwrap_or("all")
                    {
                        data.search = values(&value);
                        for track in data.search.clone() {
                            data.registry.insert(text(&track, "id"), track);
                        }
                    }
                }
                "get_discover_tracks" => {
                    data.flow = values(&value);
                    data.home = data.flow.clone();
                    data.flow_index = 0;
                    for track in data.flow.clone() {
                        data.registry.insert(text(&track, "id"), track);
                    }
                }
                "match_spotify_track" => {
                    if data
                        .pending_track
                        .as_ref()
                        .is_none_or(|track| track["id"] != args[0]["id"])
                    {
                        return;
                    }
                    data.matches = values(&value);
                    data.matches_owner = text(&args[0], "id");
                    for track in data.matches.clone() {
                        data.registry.insert(text(&track, "id"), track);
                    }
                    if data.matches.is_empty() {
                        ui.set_modal_error(crate::tr!(
                            ui,
                            "Не знайдено доступних версій. Спробуйте пошук вручну.",
                            "No playable versions found. Try searching manually."
                        ));
                    }
                }
                "get_downloads" => {
                    data.downloads = if value.is_array() {
                        values(&value)
                    } else {
                        values(&value["jobs"])
                    };
                    if let Ok(mut set) = shared.offline.lock() {
                        set.clear();
                        for job in &data.downloads {
                            if job["status"].as_str() == Some("completed") {
                                if let Some(id) = job["track"]["id"].as_str() {
                                    if !id.is_empty() {
                                        set.insert(id.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                "resolve_direct_url" => {
                    if let Some(id) = value["id"].as_str() {
                        data.registry.insert(id.into(), value.clone());
                        if value["source"] == "spotify" {
                            data.pending_track = Some(value.clone());
                            reset_modal(&ui);
                            ui.set_modal_kind("match".into());
                            ui.set_modal_title(crate::tr!(ui, "Оберіть версію для завантаження", "Choose a version to download"));
                            ui.set_modal_open(true);
                            app.submit(
                                ui,
                                "match_spotify_track",
                                vec![value, json!("all"), json!(6)],
                                false,
                            );
                        } else {
                            app.submit(ui, "download_track", vec![value], false);
                        }
                    } else {
                        ui.set_message(crate::tr!(ui, "Не вдалося розпізнати музичне посилання", "Could not recognise that music link"));
                    }
                }
                "get_listening_stats" => {
                    data.stats = value.clone();
                    let unlocked = value["locked"] == false;
                    ui.set_subscription_active(unlocked);
                    if !unlocked {
                        ui.set_subscription_message(value["error"].as_str().unwrap_or("").into());
                    } else if let Some(msg) = value["subscription"]["expires_formatted"].as_str() {
                        ui.set_subscription_message(msg.into());
                    }
                }
                "get_subscription_status" => {
                    let active = value["active"] == true;
                    ui.set_subscription_active(active);
                    let msg = value["expires_formatted"]
                        .as_str()
                        .or_else(|| value["message"].as_str())
                        .unwrap_or("");
                    ui.set_subscription_message(msg.into());
                }
                "buy_subscription" | "grant_subscription" => {
                    close_modal(&ui);
                    let msg = value["message"].as_str().unwrap_or("SoundFlow Plus активовано!");
                    ui.set_message(msg.into());
                    ui.set_subscription_active(true);
                    if let Some(fmt) = value["subscription"]["expires_formatted"].as_str() {
                        ui.set_subscription_message(fmt.into());
                    }
                    let args = vec![json!(data.stats_period), json!(data.stats_source)];
                    drop(data);
                    app.submit(ui, "get_subscription_status", vec![], false);
                    app.submit(ui, "get_listening_stats", args, false);
                }
                "cancel_subscription" => {
                    close_modal(&ui);
                    let is_en = ui.get_language() == "en";
                    let msg = if is_en {
                        "SoundFlow Plus subscription has been cancelled."
                    } else {
                        "Підписку SoundFlow Plus успішно скасовано."
                    };
                    ui.set_message(msg.into());
                    ui.set_subscription_active(false);
                    ui.set_subscription_message(if is_en {
                        "Subscription inactive".into()
                    } else {
                        "Підписка не активна".into()
                    });
                    let is_stats = ui.get_view() == "stats";
                    let args = vec![json!(data.stats_period), json!(data.stats_source)];
                    drop(data);
                    app.submit(ui, "get_subscription_status", vec![], false);
                    if is_stats {
                        app.submit(ui, "get_listening_stats", args, false);
                    }
                }
                "switch_account" | "github_sign_in" => {
                    close_modal(&ui);
                    ui.set_show_onboarding(false);
                    let is_en = ui.get_language() == "en";
                    ui.set_message(if is_en { "Account updated successfully".into() } else { "Акаунт успішно оновлено".into() });
                    let is_stats = ui.get_view() == "stats";
                    let args = vec![json!(data.stats_period), json!(data.stats_source)];
                    drop(data);
                    app.submit(ui, "get_subscription_status", vec![], false);
                    if is_stats {
                        app.submit(ui, "get_listening_stats", args, false);
                    }
                }
                "google_sign_in" => {
                    ui.set_show_onboarding(false);
                    let is_en = ui.get_language() == "en";
                    ui.set_message(if is_en {
                        "Google sign-in successful! Searching music accounts...".into()
                    } else {
                        "Вхід через Google успішний! Шукаємо музичні акаунти...".into()
                    });
                    let is_stats = ui.get_view() == "stats";
                    let args = vec![json!(data.stats_period), json!(data.stats_source)];
                    let email = value["user"]["email"]
                        .as_str()
                        .or_else(|| data.cloud["user"]["email"].as_str())
                        .unwrap_or("")
                        .to_owned();
                    let name = value["user"]["name"]
                        .as_str()
                        .or_else(|| data.cloud["user"]["name"].as_str())
                        .unwrap_or("")
                        .to_owned();
                    drop(data);
                    app.submit(ui, "get_subscription_status", vec![], false);
                    if is_stats {
                        app.submit(ui, "get_listening_stats", args, false);
                    }
                    // Жодних проміжних вікон: тихо перевіряємо, чи прив'язані до цього
                    // Google-акаунта музичні сервіси, і питаємо про імпорт лише якщо так.
                    app.submit(
                        ui,
                        "detect_connected_accounts",
                        vec![json!(email), json!(name), json!("prompt")],
                        true,
                    );
                }
                "create_playlist" => {
                    if data.add_to_new_playlist {
                        data.add_to_new_playlist = false;
                        if let Some(track) = data.pending_track.clone() {
                            app.submit(
                                ui,
                                "add_to_playlist",
                                vec![value["id"].clone(), track],
                                false,
                            );
                        }
                    }
                    close_modal(&ui);
                    ui.set_message(crate::tr!(ui, "Плейлист створено", "Playlist created"));
                }
                "rename_playlist" | "import_external_profile" => {
                    close_modal(&ui);
                    if value["profile"].as_bool() == Some(true) {
                        let playlists = value["playlists"].as_u64().unwrap_or(0);
                        let tracks = value["count"].as_u64().unwrap_or(0);
                        ui.set_message(crate::tr!(
                            ui,
                            &format!("Імпортовано {playlists} плейлистів ({tracks} треків)"),
                            &format!("Imported {playlists} playlists ({tracks} tracks)")
                        ));
                    } else {
                        ui.set_message(crate::tr!(ui, "Бібліотеку оновлено", "Library updated"));
                    }
                }
                "detect_connected_accounts" => {
                    ui.set_modal_busy(false);
                    let yt_found = value["youtube"]["found"].as_bool().unwrap_or(false);
                    let yt_acc = value["youtube"]["name"].as_str().or_else(|| value["youtube"]["identifier"].as_str()).unwrap_or("");
                    let sc_found = value["soundcloud"]["found"].as_bool().unwrap_or(false);
                    let sc_acc = value["soundcloud"]["name"].as_str().or_else(|| value["soundcloud"]["identifier"].as_str()).unwrap_or("");
                    let sp_found = value["spotify"]["found"].as_bool().unwrap_or(false);
                    let sp_acc = value["spotify"]["name"].as_str().or_else(|| value["spotify"]["identifier"].as_str()).unwrap_or("");
                    ui.set_yt_found(yt_found);
                    ui.set_yt_account(yt_acc.into());
                    ui.set_sc_found(sc_found);
                    ui.set_sc_account(sc_acc.into());
                    ui.set_sp_found(sp_found);
                    ui.set_sp_account(sp_acc.into());
                    ui.set_accounts_import_status("".into());
                    let is_en = ui.get_language() == "en";
                    let prompt = args.get(2).and_then(|v| v.as_str()) == Some("prompt");
                    let prompt_suppressed =
                        prompt && data.settings["import_prompt_disabled"].as_bool() == Some(true);
                    if prompt_suppressed {
                        // Користувач уже попросив більше не питати про імпорт.
                    } else if prompt {
                        ui.set_import_prompt_disabled(false);
                        // Питаємо про імпорт лише тоді, коли сервіси справді знайдені
                        // через Google-профіль, а не підключені вручну раніше.
                        let mut found: Vec<String> = Vec::new();
                        if yt_found && value["youtube"]["google_linked"].as_bool() == Some(true) {
                            found.push(if yt_acc.is_empty() {
                                "YouTube Music".to_string()
                            } else {
                                format!("YouTube Music · {yt_acc}")
                            });
                        }
                        if sc_found && value["soundcloud"]["google_linked"].as_bool() == Some(true) {
                            found.push(if sc_acc.is_empty() {
                                "SoundCloud".to_string()
                            } else {
                                format!("SoundCloud · {sc_acc}")
                            });
                        }
                        if sp_found && value["spotify"]["google_linked"].as_bool() == Some(true) {
                            found.push(if sp_acc.is_empty() {
                                "Spotify".to_string()
                            } else {
                                format!("Spotify · {sp_acc}")
                            });
                        }
                        if found.is_empty() {
                            ui.set_message(if is_en {
                                "No music accounts linked to your Google profile. You can connect them in Settings.".into()
                            } else {
                                "Не знайдено музичних акаунтів, прив'язаних до Google. Підключіть їх у Налаштуваннях.".into()
                            });
                        } else {
                            let list = found.join(", ");
                            let description = if is_en {
                                format!("These accounts are linked to your Google profile: {list}. Import their music into SoundFlow?")
                            } else {
                                format!("До вашого Google-профілю прив'язані: {list}. Імпортувати цю музику в SoundFlow?")
                            };
                            reset_modal(&ui);
                            ui.set_modal_kind("import-prompt".into());
                            ui.set_modal_title(if is_en {
                                "Import your music library?".into()
                            } else {
                                "Імпортувати музичну бібліотеку?".into()
                            });
                            ui.set_modal_description(description.into());
                            ui.set_modal_open(true);
                        }
                    } else {
                        reset_modal(&ui);
                        ui.set_modal_kind("connected-accounts".into());
                        ui.set_modal_title(if is_en {
                            "Connected Music Accounts (Google Cloud + Supabase)".into()
                        } else {
                            "Підключені музичні акаунти (Google Cloud + Supabase)".into()
                        });
                        ui.set_modal_description(if is_en {
                            "SoundFlow checked your Google profile for music services. Confirm these accounts or connect separate accounts for each service:".into()
                        } else {
                            "SoundFlow перевірив прив'язку сервісів до вашого Google-профілю. Підтвердіть їх або підключіть окремі акаунти для кожного сервісу:".into()
                        });
                        ui.set_modal_open(true);
                    }
                }
                "set_connected_account" => {
                    ui.set_modal_busy(false);
                    let email = data.cloud["user"]["email"].as_str().unwrap_or_default().to_string();
                    let name = data.cloud["user"]["name"].as_str().unwrap_or_default().to_string();
                    drop(data);
                    app.submit(ui, "detect_connected_accounts", vec![json!(email), json!(name)], false);
                }
                "sync_connected_accounts" => {
                    ui.set_modal_busy(false);
                    let total = value["total_imported"].as_u64().unwrap_or(0);
                    let is_en = ui.get_language() == "en";
                    let msg = if is_en {
                        format!("Successfully imported {total} tracks into library!")
                    } else {
                        format!("Успішно імпортовано {total} треків у бібліотеку!")
                    };
                    ui.set_accounts_import_status(msg.clone().into());
                    if ui.get_modal_kind() == "import-prompt" {
                        close_modal(ui);
                    }
                    ui.set_message(msg.into());
                    app.submit(ui, "_refresh", vec![], true);
                }
                "sync_library" => {
                    let revision = value["revision"].as_u64().unwrap_or(0);
                    let push = value["mode"].as_str() == Some("push");
                    let empty = value["empty"].as_bool() == Some(true);
                    let message = if empty {
                        crate::tr!(ui, "У хмарі ще немає збереженої бібліотеки", "There is no library snapshot in the cloud yet")
                    } else if push {
                        crate::tr!(
                            ui,
                            &format!("Бібліотеку надіслано в хмару (ревізія {revision})"),
                            &format!("Library pushed to the cloud (revision {revision})")
                        )
                    } else {
                        crate::tr!(
                            ui,
                            &format!("Бібліотеку отримано з хмари (ревізія {revision})"),
                            &format!("Library pulled from the cloud (revision {revision})")
                        )
                    };
                    ui.set_message(message);
                }
                "get_infinite_recommendations" => {
                    data.radio_fetching = false;
                    let new_tracks = values(&value);
                    for track in &new_tracks {
                        data.registry.insert(text(track, "id"), track.clone());
                    }
                    let existing_ids: std::collections::HashSet<String> = data
                        .queue
                        .iter()
                        .chain(data.radio_buffer.iter())
                        .map(|t| text(t, "id"))
                        .collect();
                    let fresh_tracks: Vec<Value> = new_tracks
                        .into_iter()
                        .filter(|t| !existing_ids.contains(&text(t, "id")))
                        .collect();
                    data.radio_buffer.extend(fresh_tracks);
                    data.radio_seed += 1;

                    if data.radio_waiting && !data.radio_buffer.is_empty() {
                        data.radio_waiting = false;
                        let next_track = data.radio_buffer.remove(0);
                        let next_id = text(&next_track, "id");
                        data.registry.insert(next_id.clone(), next_track.clone());
                        data.queue.push(next_track);
                        data.queue_index = data.queue.len() - 1;
                        drop(data);
                        refresh_models(ui, shared);
                        app.play_track_in(ui, &next_id, false, true);
                    }
                }
                "configure_cloud" | "sign_in" | "sign_up" | "sign_out" | "restore_session"
                | "update_settings" => {
                    if method == "sign_in" {
                        let email = data.cloud["user"]["email"]
                            .as_str()
                            .or_else(|| data.library["profile"]["email"].as_str())
                            .unwrap_or("")
                            .to_owned();
                        let name = data.cloud["user"]["name"]
                            .as_str()
                            .or_else(|| data.library["profile"]["name"].as_str())
                            .unwrap_or("")
                            .to_owned();
                        app.submit(ui, "detect_connected_accounts", vec![json!(email), json!(name)], false);
                    }
                    if let Some(volume) = data.settings["volume"].as_f64() {
                        audio.set_volume(volume as f32);
                        ui.set_volume(volume as f32);
                    }
                    data.shuffle = data.settings["shuffle"].as_bool().unwrap_or(false);
                    data.repeat = match data.settings["repeat"].as_str() {
                        Some("all") => 1,
                        Some("one") => 2,
                        _ => 0,
                    };
                }
                "delete_playlist" => {
                    data.selected_playlist.clear();
                    close_modal(&ui);
                    ui.set_view("playlists".into());
                }
                "create_subscription_checkout" => {
                    let url = value["url"].as_str().unwrap_or("");
                    if !url.is_empty() {
                        let _ = open::that_detached(url);
                    }
                }
                _ => {
                    if !quiet && !["_play", "toggle_like", "_flow_like", "get_infinite_recommendations"].contains(&method) {
                        if value["cancelled"] != true {
                            ui.set_message(
                                value["error"]
                                    .as_str()
                                    .unwrap_or(
                                        if value["ok"] == false || value["success"] == false {
                                            "Не вдалося завершити дію"
                                        } else {
                                            "Готово"
                                        },
                                    )
                                    .into(),
                            );
                        }
                    }
                }
            }
        }
    }
    if ui.get_view().as_str() == scope || !matches!(method, "search" | "get_discover_tracks") {
        refresh_models(ui, shared);
    }
    {
        let (speed, normalize) = {
            let data = shared.data.lock().unwrap_or_else(|e| e.into_inner());
            (
                data.settings["playback_speed"].as_f64().unwrap_or(1.0) as f32,
                data.settings["volume_norm_enabled"].as_bool().unwrap_or(true),
            )
        };
        if let Ok(mut applied) = shared.audio_applied.lock() {
            if (applied.0 - speed).abs() > f32::EPSILON {
                applied.0 = speed;
                app.audio.set_speed(speed);
            }
            if applied.1 != normalize {
                applied.1 = normalize;
                app.audio.set_normalize(normalize);
            }
        }
    }
    let stats = shared
        .data
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .stats
        .clone();
    if stats["locked"] == false {
        ui.set_metrics(rows(vec![
            StatMetric {
                label: "Прослухано".into(),
                value: format!(
                    "{} хв",
                    stats["total_minutes"].as_f64().unwrap_or(0.0).round()
                )
                .into(),
            },
            StatMetric {
                label: "Відтворень".into(),
                value: stats["plays"].to_string().into(),
            },
            StatMetric {
                label: "Виконавців".into(),
                value: stats["unique_artists"].to_string().into(),
            },
            StatMetric {
                label: "Треків".into(),
                value: stats["unique_tracks"].to_string().into(),
            },
        ]));
        let rank = |items: &Value, key: &str| {
            rows(
                values(items)
                    .iter()
                    .map(|item| RankRow {
                        label: text(item, key).into(),
                        detail: format!("{} відтворень · {} хв", item["plays"], item["minutes"])
                            .into(),
                    })
                    .collect(),
            )
        };
        ui.set_artists(rank(&stats["top_artists"], "name"));
        ui.set_top_tracks(rank(&stats["top_tracks"], "title"));
        ui.set_providers(rank(&stats["providers"], "provider"));
        let daily = values(&stats["daily"]);
        let maximum = daily
            .iter()
            .filter_map(|v| v["seconds"].as_f64())
            .fold(0.0, f64::max)
            .max(1.0);
        ui.set_daily(rows(
            daily
                .iter()
                .rev()
                .take(30)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|item| DailyRow {
                    label: text(item, "date")
                        .chars()
                        .skip(5)
                        .collect::<String>()
                        .into(),
                    value: format!("{} хв", item["minutes"].as_f64().unwrap_or(0.0).round()).into(),
                    level: (item["seconds"].as_f64().unwrap_or(0.0) / maximum) as f32,
                })
                .collect(),
        ));
        if stats["unknown_duration_plays"].as_u64().unwrap_or(0) > 0 {
            ui.set_subscription_message(
                format!(
                    "{} відтворень без даних про тривалість",
                    stats["unknown_duration_plays"]
                )
                .into(),
            );
        }
    }
    if shared
        .artwork_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
        .is_ok()
    {
        let candidates = {
            let data = shared.data.lock().unwrap_or_else(|e| e.into_inner());
            let mut tracks = data.search.clone();
            tracks.extend(data.flow.clone());
            tracks.extend(data.registry.values().cloned());
            tracks.truncate(12);
            tracks
        };
        let shared = shared.clone();
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            let changed = fetch_artworks(&candidates, &shared.cache);
            shared.artwork_busy.store(false, Ordering::Release);
            if changed {
                let _ = weak.upgrade_in_event_loop(move |ui| refresh_models(&ui, &shared));
            }
        });
    }
}
fn fetch_artworks(tracks: &[Value], cache: &Path) -> bool {
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        return false;
    };
    let mut changed = false;
    for track in tracks {
        let url = text(track, "thumbnail");
        let Ok(parsed) = url::Url::parse(&url) else {
            continue;
        };
        let host = parsed.host_str().unwrap_or("");
        if parsed.scheme() != "https"
            || ![
                "ytimg.com",
                "ggpht.com",
                "googleusercontent.com",
                "sndcdn.com",
                "scdn.co",
                "spotifycdn.com",
                "mzstatic.com",
            ]
            .iter()
            .any(|d| host == *d || host.ends_with(&format!(".{d}")))
        {
            continue;
        }
        let id = format!("{:x}", Sha256::digest(url.as_bytes()));
        if ["jpg", "png", "webp"]
            .iter()
            .any(|e| cache.join(format!("{id}.{e}")).is_file())
        {
            continue;
        }
        let Ok(response) = client.get(&url).send() else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let extension = match response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
        {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            _ => continue,
        };
        let mut bytes = Vec::new();
        if response
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > 4 * 1024 * 1024
        {
            continue;
        }
        if fs::write(cache.join(format!("{id}.{extension}")), bytes).is_ok() {
            changed = true;
        }
    }
    changed
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    if crate::verification::run_from_args()? {
        return Ok(());
    }
    let guard = crate::platform::initialize_process()?;
    if !guard.is_primary() {
        return Ok(());
    }
    let data = crate::platform::data_dir();
    let resources = std::env::current_exe()?
        .parent()
        .ok_or("Недійсна папка застосунку")?
        .to_path_buf();
    let offline = std::env::args().any(|arg| arg == "--offline");
    let app = Application::new(data, resources, offline)?;
    let ui = AppWindow::new()?;
    let _binding = app.bind(&ui);
    let weak = ui.as_weak();
    guard.listen_activation(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.show();
        }
    });
    // Tray and system shortcuts belong to normal interactive runs only.
    let tray = crate::SoundFlowTray::new()?;
    let weak = ui.as_weak();
    tray.on_show_window(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.show();
        }
    });
    let audio = app.audio.clone();
    tray.on_play_pause(move || audio.toggle());
    let weak = ui.as_weak();
    tray.on_next_track(move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_player_action("next".into());
        }
    });
    tray.on_quit_app(move || {
        let _ = slint::quit_event_loop();
    });
    let weak = ui.as_weak();
    let cached = app.shared.close_to_tray.clone();
    ui.window().on_close_requested(move || {
        if cached.load(Ordering::Relaxed) {
            if let Some(ui) = weak.upgrade() {
                let _ = ui.hide();
            }
            slint::CloseRequestResponse::KeepWindowShown
        } else {
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::HideWindow
        }
    });
    let weak = ui.as_weak();
    let _keys = crate::media_keys::start(app.shared.media_hotkeys.clone(), move |action| {
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if action == "registration-error" {
                ui.set_message(crate::tr!(ui, "Деякі медіаклавіші зайняті іншим застосунком", "Some media hotkeys are taken by another app"));
            } else {
                ui.invoke_player_action(action.into());
            }
        });
    });
    let tray_state = slint::Timer::default();
    let weak = ui.as_weak();
    let tray_weak = tray.as_weak();
    tray_state.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            if let (Some(ui), Some(tray)) = (weak.upgrade(), tray_weak.upgrade()) {
                tray.set_playing(ui.get_playing());
            }
        },
    );
    tray.show()?;
    ui.run()?;
    tray_state.stop();
    tray.hide()?;
    app.shutdown();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spotify_matches() -> UiData {
        let original = json!({"id":"spotify-original","title":"Original","artist":"Artist","source":"spotify","url":"https://open.spotify.com/track/original"});
        let candidate = json!({"id":"youtube-version","title":"Selected version","artist":"Artist","source":"youtube","url":"https://www.youtube.com/watch?v=version","matched_from":original,"match_confirmed":false});
        let mut data = UiData {
            pending_track: Some(original),
            matches_owner: "spotify-original".into(),
            matches: vec![candidate.clone()],
            ..Default::default()
        };
        data.registry.insert("youtube-version".into(), candidate);
        data
    }
    #[test]
    fn choosing_spotify_match_confirms_only_selected_playback_download_candidate() {
        let mut data = spotify_matches();
        assert_eq!(data.registry["youtube-version"]["match_confirmed"], false);
        let chosen = confirm_selected_match(&mut data, "youtube-version").unwrap();
        assert_eq!(chosen["match_confirmed"], true);
        assert_eq!(chosen["matched_from"]["id"], "spotify-original");
        assert_eq!(data.registry["youtube-version"], chosen);
        assert_eq!(data.matches[0], chosen);
    }
    #[test]
    fn stale_or_unlisted_spotify_versions_cannot_be_confirmed() {
        let mut data = spotify_matches();
        assert!(confirm_selected_match(&mut data, "arbitrary-id").is_err());
        data.matches[0]["matched_from"]["id"] = json!("other-song");
        assert!(confirm_selected_match(&mut data, "youtube-version").is_err());
        assert_eq!(data.registry["youtube-version"]["match_confirmed"], false);
        let mut data = spotify_matches();
        data.pending_track.as_mut().unwrap()["id"] = json!("next-song");
        assert!(confirm_selected_match(&mut data, "youtube-version").is_err());
        assert_eq!(data.registry["youtube-version"]["match_confirmed"], false);
    }
}
