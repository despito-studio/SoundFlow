use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub type CoreResult<T> = Result<T, String>;

pub fn now() -> f64 {
    chrono::Utc::now().timestamp_millis() as f64 / 1000.0
}
pub fn text(value: Option<&Value>, max: usize) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .chars()
        .take(max)
        .collect()
}

fn default_settings() -> Map<String, Value> {
    json!({"volume":0.8,"repeat":"off","shuffle":false,"preferred_source":"all",
        "hotkeys_enabled":true,"tray_enabled":true,"local_music_enabled":true,"auto_update_enabled":true,
        "auto_update_install":true,"restore_playback_enabled":true,
        "volume_norm_enabled":true,"direct_paste_enabled":true,"smart_cache_enabled":true,
        "offline_download_enabled":true,"infinite_autoplay":true,"discord_rpc_enabled":true,"theme":"dark",
        "language":"uk","accent":"blue","playback_speed":1.0,"onboarding_completed":false,"import_prompt_disabled":false})
    .as_object()
    .unwrap()
    .clone()
}
fn default_profile() -> Map<String, Value> {
    json!({"sf_id":format!("SF-{}", &Uuid::new_v4().simple().to_string()[..8].to_uppercase()),
        "name":"Слухач","role":"listener","avatar_url":"","connected_accounts":{}})
    .as_object()
    .unwrap()
    .clone()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    #[serde(default = "now")]
    pub created_at: f64,
    #[serde(default)]
    pub tracks: Vec<Value>,
    #[serde(default)]
    pub is_auto: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Library {
    #[serde(default)]
    pub liked_tracks: Vec<Value>,
    #[serde(default)]
    pub playlists: BTreeMap<String, Playlist>,
    #[serde(default)]
    pub history: Vec<Value>,
    #[serde(default = "default_settings")]
    pub settings: Map<String, Value>,
    #[serde(default = "default_profile")]
    pub profile: Map<String, Value>,
    #[serde(default)]
    pub search_history: Vec<String>,
    #[serde(default)]
    pub play_counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub track_catalog: BTreeMap<String, Value>,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            liked_tracks: vec![],
            playlists: BTreeMap::new(),
            history: vec![],
            settings: default_settings(),
            profile: default_profile(),
            search_history: vec![],
            play_counts: BTreeMap::new(),
            track_catalog: BTreeMap::new(),
        }
    }
}

const BACKUP_INTERVAL_SECONDS: f64 = 3600.0;
const BACKUP_MAX_BYTES: u64 = 32 * 1024 * 1024;

// Deliberately excludes settings, profile, account links and authentication data.
#[derive(Clone, Serialize, Deserialize)]
struct BackupContent {
    liked_tracks: Vec<Value>,
    playlists: BTreeMap<String, Playlist>,
    history: Vec<Value>,
    search_history: Vec<String>,
    play_counts: BTreeMap<String, u64>,
    track_catalog: BTreeMap<String, Value>,
}
impl BackupContent {
    fn from_library(data: &Library) -> Self {
        Self {
            liked_tracks: data.liked_tracks.clone(), playlists: data.playlists.clone(),
            history: data.history.clone(), search_history: data.search_history.clone(),
            play_counts: data.play_counts.clone(), track_catalog: data.track_catalog.clone(),
        }
    }
    fn validate(&mut self) -> CoreResult<()> {
        for track in self.liked_tracks.iter_mut().chain(self.history.iter_mut())
            .chain(self.track_catalog.values_mut()) {
            *track = validated_track(track)?;
        }
        for (id, playlist) in &mut self.playlists {
            if id != &playlist.id || id.len() > 200 || playlist.name.chars().count() > 160
                || playlist.tracks.len() > 100000 || !playlist.created_at.is_finite() {
                return Err("Недійсний плейлист у резервній копії".into());
            }
            for track in &mut playlist.tracks { *track = validated_track(track)?; }
        }
        if self.search_history.len() > 25 || self.search_history.iter().any(|q| q.chars().count() > 500) {
            return Err("Недійсна історія пошуку у резервній копії".into());
        }
        Ok(())
    }
    fn apply(self, data: &mut Library) {
        data.liked_tracks = self.liked_tracks;
        data.playlists = self.playlists;
        data.history = self.history;
        data.search_history = self.search_history;
        data.play_counts = self.play_counts;
        data.track_catalog = self.track_catalog;
    }
}
#[derive(Serialize, Deserialize)]
struct BackupFile {
    version: u32,
    created_at: f64,
    kind: String,
    content: BackupContent,
}

#[derive(Clone)]
pub struct LibraryStore {
    pub data: Library,
    path: PathBuf,
    last_automatic_backup: std::cell::Cell<Option<f64>>,
}

pub fn validated_track(track: &Value) -> CoreResult<Value> {
    let input = track.as_object().ok_or("Недійсний трек")?;
    let id = text(input.get("id"), 500);
    let title = text(input.get("title"), 500);
    if id.is_empty() || title.is_empty() {
        return Err("Трек повинен мати ID та назву".into());
    }
    let mut output = input.clone();
    output.insert("id".into(), json!(id));
    output.insert("title".into(), json!(title));
    for field in [
        "artist",
        "uploader",
        "source",
        "url",
        "source_url",
        "thumbnail",
        "duration_formatted",
    ] {
        if let Some(value) = output.get(field) {
            if !value.is_string() {
                return Err(format!("Недійсне поле треку: {field}"));
            }
        }
    }
    if !output.contains_key("url") {
        if let Some(source) = output.get("source_url").cloned() {
            output.insert("url".into(), source);
        }
    }
    output
        .entry("thumbnail")
        .or_insert(json!("assets/icon.png"));
    // A file URL in an imported library never grants filesystem access.
    output.remove("local_path");
    Ok(Value::Object(output))
}

impl LibraryStore {
    pub fn open(path: PathBuf) -> CoreResult<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = if path.exists() {
            let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            serde_json::from_str::<Library>(&raw).map_err(|_| {
                "Файл бібліотеки пошкоджено. Оригінал збережено без змін.".to_string()
            })?
        } else {
            Library::default()
        };
        let mut store = Self { data, path, last_automatic_backup: std::cell::Cell::new(None) };
        store.data.profile.insert("role".into(), json!("listener"));
        for (key, value) in default_settings() {
            store.data.settings.entry(key).or_insert(value);
        }
        store.save()?;
        Ok(store)
    }
    pub fn snapshot(&self) -> Value {
        serde_json::to_value(&self.data).unwrap_or(Value::Null)
    }
    pub fn save(&self) -> CoreResult<()> {
        // Legacy metadata or an unavailable backup directory must not prevent
        // startup and ordinary library writes. Explicit backups and the safety
        // copy before restoration still propagate every error to the caller.
        if let Err(error) = self.automatic_backup() {
            eprintln!("SoundFlow: automatic library backup failed: {error}");
        }
        self.persist_data()
    }
    fn backup_dir(&self) -> CoreResult<PathBuf> {
        let parent = self.path.parent().ok_or("Недійсний шлях бібліотеки")?;
        let name = self.path.file_name().and_then(|n| n.to_str()).ok_or("Недійсна бібліотека")?;
        Ok(parent.join("library-backups").join(name))
    }
    fn read_backup(&self, id: &str) -> CoreResult<BackupFile> {
        if id.len() > 100 || !id.ends_with(".json") || !id[..id.len()-5].bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-') {
            return Err("Недійсний ID резервної копії".into());
        }
        let root = self.backup_dir()?.canonicalize().map_err(|e| e.to_string())?;
        let path = root.join(id);
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !metadata.file_type().is_file() || metadata.len() > BACKUP_MAX_BYTES
            || path.canonicalize().map_err(|e| e.to_string())?.parent() != Some(root.as_path()) {
            return Err("Недійсна резервна копія".into());
        }
        let mut backup: BackupFile = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|_| "Резервну копію пошкоджено")?;
        if backup.version != 1 || !backup.created_at.is_finite() || backup.created_at < 0.0
            || !["automatic", "manual", "before_restore"].contains(&backup.kind.as_str()) {
            return Err("Непідтримуваний формат резервної копії".into());
        }
        backup.content.validate()?;
        Ok(backup)
    }
    fn backup_entries(&self) -> CoreResult<Vec<Value>> {
        let directory = self.backup_dir()?;
        if !directory.exists() { return Ok(vec![]); }
        let mut entries = Vec::new();
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let id = entry.file_name().to_string_lossy().into_owned();
            // An incomplete or damaged backup must not hide the remaining usable copies.
            if let Ok(backup) = self.read_backup(&id) {
                entries.push(json!({"id":id,"created_at":backup.created_at,"kind":backup.kind,
                    "liked_count":backup.content.liked_tracks.len(),
                    "playlist_count":backup.content.playlists.len(),
                    "history_count":backup.content.history.len(),
                    "size_bytes":entry.metadata().map_err(|e| e.to_string())?.len()}));
            }
        }
        entries.sort_by(|a,b| b["created_at"].as_f64().unwrap().total_cmp(&a["created_at"].as_f64().unwrap())
            .then_with(|| b["id"].as_str().cmp(&a["id"].as_str())));
        Ok(entries)
    }
    pub fn list_backups(&self) -> CoreResult<Value> {
        Ok(json!({"backups":self.backup_entries()?,"automatic_interval_seconds":3600,
            "automatic_limit":10,"manual_limit":10,"safety_limit":3}))
    }
    fn write_backup(&self, data: &Library, kind: &str) -> CoreResult<Value> {
        let directory = self.backup_dir()?;
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let mut content = BackupContent::from_library(data);
        content.validate()?;
        let backup = BackupFile { version:1, created_at:now(), kind:kind.into(), content };
        let raw = serde_json::to_vec_pretty(&backup).map_err(|e| e.to_string())?;
        if raw.len() as u64 > BACKUP_MAX_BYTES { return Err("Резервна копія перевищує 32 МБ".into()); }
        let id = format!("{}-{}.json", chrono::Utc::now().timestamp_millis(), Uuid::new_v4().simple());
        let mut temp = tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
        temp.write_all(&raw).map_err(|e| e.to_string())?;
        temp.flush().map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist_noclobber(directory.join(&id)).map_err(|e| e.error.to_string())?;
        let entries = self.backup_entries()?;
        let limit = if kind == "before_restore" { 3 } else { 10 };
        for entry in entries.iter().filter(|e| e["kind"] == kind && e["id"] != id).skip(limit - 1) {
            let old_id = entry["id"].as_str().ok_or("Недійсний ID копії")?;
            // IDs here came from validated direct children of this account's backup directory.
            fs::remove_file(directory.join(old_id)).map_err(|e| e.to_string())?;
        }
        Ok(json!({"id":id,"created_at":backup.created_at,"kind":kind,
            "liked_count":backup.content.liked_tracks.len(),"playlist_count":backup.content.playlists.len(),
            "history_count":backup.content.history.len(),"size_bytes":raw.len()}))
    }
    fn automatic_backup(&self) -> CoreResult<()> {
        if !self.path.is_file() { return Ok(()); }
        let latest = match self.last_automatic_backup.get() {
            Some(value) => value,
            None => {
                let value = self.backup_entries()?.into_iter().filter(|e| e["kind"] == "automatic")
                    .filter_map(|e| e["created_at"].as_f64()).fold(0.0_f64, f64::max);
                self.last_automatic_backup.set(Some(value));
                value
            }
        };
        if now() - latest < BACKUP_INTERVAL_SECONDS { return Ok(()); }
        let raw = read_legacy(&self.path)?;
        let previous: Library = serde_json::from_str(&raw).map_err(|_| "Файл бібліотеки пошкоджено")?;
        let saved = self.write_backup(&previous, "automatic")?;
        self.last_automatic_backup.set(saved["created_at"].as_f64());
        Ok(())
    }
    pub fn create_backup(&self) -> CoreResult<Value> {
        self.write_backup(&self.data, "manual")
    }
    pub fn restore_backup(&mut self, id: &str) -> CoreResult<Value> {
        let backup = self.read_backup(id)?;
        let safety = self.write_backup(&self.data, "before_restore")?;
        let previous = self.data.clone();
        backup.content.apply(&mut self.data);
        if let Err(error) = self.persist_data() {
            self.data = previous;
            return Err(error);
        }
        Ok(json!({"ok":true,"liked_count":self.data.liked_tracks.len(),
            "playlists":self.data.playlists.len(),"safety_backup_id":safety["id"]}))
    }
    fn persist_data(&self) -> CoreResult<()> {
        let parent = self.path.parent().ok_or("Недійсний шлях бібліотеки")?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&mut temp, &self.data).map_err(|e| e.to_string())?;
        temp.flush().map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist(&self.path).map_err(|e| e.error.to_string())?;
        Ok(())
    }
    pub fn toggle_like(&mut self, track: &Value) -> CoreResult<Value> {
        let track = validated_track(track)?;
        let id = track["id"].as_str().unwrap();
        let is_liked =
            if let Some(index) = self.data.liked_tracks.iter().position(|t| t["id"] == id) {
                self.data.liked_tracks.remove(index);
                false
            } else {
                self.data.liked_tracks.insert(0, track);
                true
            };
        self.save()?;
        Ok(json!({"is_liked":is_liked,"liked_count":self.data.liked_tracks.len()}))
    }
    pub fn create_playlist(&mut self, name: &str) -> CoreResult<Value> {
        if name.chars().count() > 160 {
            return Err("Назва плейлиста має містити до 160 символів".into());
        }
        let name = if name.trim().is_empty() {
            "Новий плейлист"
        } else {
            name.trim()
        };
        let id = format!("pl_{}", Uuid::new_v4().simple());
        let playlist = Playlist {
            id: id.clone(),
            name: name.into(),
            created_at: now(),
            tracks: vec![],
            is_auto: false,
        };
        let result = serde_json::to_value(&playlist).map_err(|e| e.to_string())?;
        self.data.playlists.insert(id, playlist);
        self.save()?;
        Ok(result)
    }
    pub fn create_playlist_with_tracks(&mut self, name: &str, tracks: &Value) -> CoreResult<Value> {
        if name.chars().count() > 160 {
            return Err("Назва плейліста має містити до 160 символів".into());
        }
        let input = tracks.as_array().ok_or("Недійсні треки плейліста")?;
        let mut seen = HashSet::new();
        let mut validated = Vec::new();
        for track in input {
            let track = validated_track(track)?;
            if seen.insert(track["id"].as_str().unwrap().to_owned()) {
                validated.push(track);
            }
        }
        let id = format!("pl_{}", Uuid::new_v4().simple());
        let added = validated.len();
        let playlist = Playlist {
            id: id.clone(),
            name: if name.trim().is_empty() {
                "Новий плейліст".into()
            } else {
                name.trim().into()
            },
            created_at: now(),
            tracks: validated,
            is_auto: false,
        };
        self.data.playlists.insert(id.clone(), playlist);
        if let Err(error) = self.save() {
            self.data.playlists.remove(&id);
            return Err(error);
        }
        Ok(json!({"id": id, "added": added}))
    }
    pub fn add_to_playlist(&mut self, id: &str, track: &Value) -> CoreResult<bool> {
        let track = validated_track(track)?;
        let playlist = self
            .data
            .playlists
            .get_mut(id)
            .ok_or("Плейлист не знайдено")?;
        if playlist.tracks.iter().any(|t| t["id"] == track["id"]) {
            return Ok(false);
        }
        playlist.tracks.push(track);
        self.save()?;
        Ok(true)
    }
    pub fn rename_playlist(&mut self, id: &str, name: &str) -> CoreResult<bool> {
        if name.trim().is_empty() || name.chars().count() > 160 {
            return Err("Вкажіть назву плейлиста до 160 символів".into());
        }
        let playlist = self
            .data
            .playlists
            .get_mut(id)
            .ok_or("Плейлист не знайдено")?;
        playlist.name = name.trim().into();
        self.save()?;
        Ok(true)
    }
    pub fn delete_playlist(&mut self, id: &str) -> CoreResult<bool> {
        let removed = self.data.playlists.remove(id).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }
    pub fn remove_track(&mut self, id: &str, track_id: &str) -> CoreResult<bool> {
        let playlist = self
            .data
            .playlists
            .get_mut(id)
            .ok_or("Плейлист не знайдено")?;
        let before = playlist.tracks.len();
        playlist.tracks.retain(|t| t["id"] != track_id);
        let changed = before != playlist.tracks.len();
        if changed {
            self.save()?;
        }
        Ok(changed)
    }
    pub fn record_history(&mut self, track: &Value) -> CoreResult<bool> {
        let mut track = validated_track(track)?;
        let id = track["id"].as_str().unwrap().to_string();
        self.data.track_catalog.insert(id.clone(), track.clone());
        *self.data.play_counts.entry(id.clone()).or_default() += 1;
        track["played_at"] = json!(now());
        self.data.history.retain(|t| t["id"] != id);
        self.data.history.insert(0, track);
        self.data.history.truncate(150);
        let mut counts: Vec<_> = self.data.play_counts.iter().collect();
        counts.sort_by(|a, b| b.1.cmp(a.1));
        let tracks = counts
            .into_iter()
            .filter_map(|(id, _)| self.data.track_catalog.get(id).cloned())
            .take(50)
            .collect();
        self.data.playlists.insert(
            "frequently_played".into(),
            Playlist {
                id: "frequently_played".into(),
                name: "Часто слухаєш".into(),
                created_at: now(),
                tracks,
                is_auto: true,
            },
        );
        self.save()?;
        Ok(true)
    }
    pub fn clear_history(&mut self) -> CoreResult<bool> {
        self.data.history.clear();
        self.data.play_counts.clear();
        self.data.track_catalog.clear();
        self.data.playlists.remove("frequently_played");
        self.save()?;
        Ok(true)
    }
    pub fn update_settings(&mut self, payload: &Value) -> CoreResult<bool> {
        let values = payload.as_object().ok_or("Недійсні налаштування")?;
        for (key, value) in values {
            match key.as_str() {
                "playback_speed"
                    if value
                        .as_f64()
                        .is_some_and(|v| v.is_finite() && (0.5..=2.0).contains(&v)) => {}
                "volume"
                    if value
                        .as_f64()
                        .is_some_and(|v| v.is_finite() && (0.0..=1.0).contains(&v)) => {}
                "repeat" if ["off", "all", "one"].contains(&value.as_str().unwrap_or_default()) => {
                }
                "desktop_icon"
                    if ["default", "rose", "violet", "ocean"]
                        .contains(&value.as_str().unwrap_or_default()) => {}
                "theme"
                    if ["light", "dark", "rose", "violet", "ocean", "system"]
                        .contains(&value.as_str().unwrap_or_default()) => {}
                "language" if ["uk", "en"].contains(&value.as_str().unwrap_or_default()) => {}
                "accent"
                    if ["mono", "blue", "violet", "rose", "amber", "green"]
                        .contains(&value.as_str().unwrap_or_default()) => {}
                "preferred_source"
                    if ["all", "youtube", "soundcloud", "spotify", "local"]
                        .contains(&value.as_str().unwrap_or_default()) => {}
                "shuffle"
                | "reduced_motion"
                | "hotkeys_enabled"
                | "tray_enabled"
                | "auto_update_enabled"
                | "auto_update_install"
                | "restore_playback_enabled"
                | "local_music_enabled"
                | "volume_norm_enabled"
                | "direct_paste_enabled"
                | "smart_cache_enabled"
                | "offline_download_enabled"
                | "infinite_autoplay"
                | "discord_rpc_enabled"
                | "onboarding_completed"
                | "import_prompt_disabled"
                    if value.is_boolean() => {}
                _ => return Err(format!("Непідтримуване налаштування: {key}")),
            }
        }
        self.data.settings.extend(values.clone());
        self.save()?;
        Ok(true)
    }
    pub fn update_profile(&mut self, value: &Value) -> CoreResult<Value> {
        let payload = value.as_object().ok_or("Недійсний профіль")?;
        let mut profile = self.data.profile.clone();
        for field in [
            "name",
            "email",
            "avatar_url",
            "banner_url",
            "banner_preset",
            "bio",
        ] {
            if let Some(value) = payload.get(field) {
                let clean = text(
                    Some(value),
                    if field.ends_with("url") || field == "bio" {
                        2000
                    } else {
                        254
                    },
                );
                if field.ends_with("url")
                    && !clean.is_empty()
                    && url::Url::parse(&clean).map(|u| {
                        if u.scheme() == "https" { return false; }
                        if u.scheme() == "file" {
                            let root = crate::platform::data_dir().join("profile-images").canonicalize();
                            let path = u.to_file_path().ok().and_then(|p| p.canonicalize().ok());
                            return !matches!((root, path), (Ok(root), Some(path)) if path.starts_with(&root));
                        }
                        true
                    }).unwrap_or(true)
                {
                    return Err("Обери фото з комп’ютера або вкажи посилання HTTPS".into());
                }
                profile.insert(field.into(), json!(clean));
            }
        }
        if let Some(hidden) = payload.get("avatar_hidden").and_then(Value::as_bool) {
            profile.insert("avatar_hidden".into(), json!(hidden));
        }
        self.data.profile = profile;
        self.save()?;
        Ok(Value::Object(self.data.profile.clone()))
    }
    pub fn add_search(&mut self, query: &str) -> CoreResult<Value> {
        let query: String = query.trim().chars().take(500).collect();
        if !query.is_empty() {
            self.data.search_history.retain(|q| q != &query);
            self.data.search_history.insert(0, query);
        }
        self.data.search_history.truncate(25);
        self.save()?;
        Ok(json!(self.data.search_history))
    }
    pub fn import_json(&mut self, raw: &str, merge: bool) -> CoreResult<Value> {
        if raw.len() > 32 * 1024 * 1024 {
            return Err("Бібліотека перевищує 32 МБ".into());
        }
        let payload: Value = serde_json::from_str(raw).map_err(|_| "Недійсний JSON бібліотеки")?;
        let likes = payload["liked_tracks"]
            .as_array()
            .ok_or("У файлі немає liked_tracks")?;
        let mut validated = likes
            .iter()
            .map(validated_track)
            .collect::<CoreResult<Vec<_>>>()?;
        let mut playlists: BTreeMap<String, Playlist> =
            serde_json::from_value(payload.get("playlists").cloned().unwrap_or(json!({})))
                .map_err(|_| "Недійсні плейлисти")?;
        for playlist in playlists.values_mut() {
            if playlist.id.len() > 200
                || playlist.name.chars().count() > 160
                || playlist.tracks.len() > 100000
            {
                return Err("Недійсний плейлист".into());
            }
            playlist.tracks = playlist
                .tracks
                .iter()
                .map(validated_track)
                .collect::<CoreResult<Vec<_>>>()?;
        }
        let history = payload
            .get("history")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let history = history
            .iter()
            .take(150)
            .map(validated_track)
            .collect::<CoreResult<Vec<_>>>()?;
        let before = self.data.clone();
        if merge {
            let mut known: HashSet<String> = self
                .data
                .liked_tracks
                .iter()
                .filter_map(|t| t["id"].as_str().map(str::to_owned))
                .collect();
            validated.retain(|t| known.insert(t["id"].as_str().unwrap().into()));
            self.data.liked_tracks.extend(validated);
            for (mut id, mut playlist) in playlists {
                if self
                    .data
                    .playlists
                    .get(&id)
                    .is_some_and(|existing| existing.name != playlist.name)
                {
                    use sha2::{Digest, Sha256};
                    id = format!(
                        "import_{:x}",
                        Sha256::digest(format!("{}:{}", id, playlist.name).as_bytes())
                    );
                    playlist.id = id.clone();
                }
                if let Some(existing) = self.data.playlists.get_mut(&id) {
                    let mut known: HashSet<String> = existing
                        .tracks
                        .iter()
                        .filter_map(|t| t["id"].as_str().map(str::to_owned))
                        .collect();
                    existing.tracks.extend(
                        playlist
                            .tracks
                            .into_iter()
                            .filter(|t| known.insert(t["id"].as_str().unwrap().into())),
                    );
                } else {
                    self.data.playlists.insert(id, playlist);
                }
            }
            let mut known: HashSet<String> = self
                .data
                .history
                .iter()
                .filter_map(|t| t["id"].as_str().map(str::to_owned))
                .collect();
            self.data.history.extend(
                history
                    .into_iter()
                    .filter(|t| known.insert(t["id"].as_str().unwrap().into())),
            );
            self.data.history.truncate(150);
        } else {
            self.data.liked_tracks = validated;
            self.data.playlists = playlists;
            self.data.history = history;
        }
        // Profiles, roles, account links and paid flags are intentionally never imported.
        if let Err(error) = self.save() {
            self.data = before;
            return Err(error);
        }
        Ok(
            json!({"ok":true,"liked_count":self.data.liked_tracks.len(),"playlists":self.data.playlists.len()}),
        )
    }
    pub fn all_tracks(&self) -> Vec<&Value> {
        self.data
            .liked_tracks
            .iter()
            .chain(self.data.history.iter())
            .chain(self.data.playlists.values().flat_map(|p| p.tracks.iter()))
            .collect()
    }
}

#[cfg(test)]
mod backup_tests {
    use super::*;

    #[test]
    fn language_preference_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("language-library.json");
        let mut store = LibraryStore::open(path.clone()).unwrap();
        assert_eq!(store.data.settings["language"], "uk");
        store.update_settings(&json!({"language":"en"})).unwrap();
        drop(store);
        let restored = LibraryStore::open(path).unwrap();
        assert_eq!(restored.data.settings["language"], "en");
    }

    fn store(dir: &tempfile::TempDir) -> LibraryStore {
        LibraryStore::open(dir.path().join("library_guest.json")).unwrap()
    }
    fn track(id: &str) -> Value { json!({"id":id,"title":id,"source":"youtube"}) }

    #[test]
    fn legacy_history_does_not_block_open_or_save_when_backup_validation_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library_guest.json");
        let mut legacy = Library::default();
        legacy.history = vec![json!({"id":"old"})];
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let mut store = LibraryStore::open(path.clone()).unwrap();
        store.data.liked_tracks.push(track("new"));
        store.save().unwrap();
        let persisted: Library = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(persisted.history, legacy.history);
        assert_eq!(persisted.liked_tracks[0]["id"], "new");
        // Explicit backup requests retain strict validation and report the failure.
        assert!(store.create_backup().is_err());
    }

    #[test]
    fn unavailable_backup_directory_does_not_block_library_save() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store(&dir);
        fs::write(dir.path().join("library-backups"), b"occupied").unwrap();
        store.data.liked_tracks.push(track("new"));
        store.save().unwrap();
        let reopened = LibraryStore::open(store.path.clone()).unwrap();
        assert_eq!(reopened.data.liked_tracks[0]["id"], "new");
        assert!(store.create_backup().is_err());
        assert_eq!(fs::read(dir.path().join("library-backups")).unwrap(), b"occupied");
    }

    #[test]
    fn restore_round_trip_preserves_current_identity_and_settings() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store(&dir);
        store.data.liked_tracks.push(track("old"));
        store.data.history.push(track("heard"));
        store.data.search_history.push("query".into());
        store.data.play_counts.insert("old".into(), 4);
        store.data.track_catalog.insert("old".into(), track("old"));
        store.create_playlist_with_tracks("Mix", &json!([track("old")])).unwrap();
        store.data.profile.insert("token".into(), json!("secret-profile"));
        store.data.settings.insert("secret".into(), json!("secret-setting"));
        let saved = store.create_backup().unwrap();
        let id = saved["id"].as_str().unwrap();
        let raw = fs::read_to_string(store.backup_dir().unwrap().join(id)).unwrap();
        assert!(!raw.contains("secret-profile"));
        assert!(!raw.contains("secret-setting"));
        store.data.liked_tracks = vec![track("new")];
        store.data.settings.insert("volume".into(), json!(0.2));
        store.data.profile.insert("name".into(), json!("Current"));
        let restored = store.restore_backup(id).unwrap();
        assert_eq!(store.data.liked_tracks[0]["id"], "old");
        assert_eq!(store.data.play_counts["old"], 4);
        assert_eq!(store.data.track_catalog["old"]["id"], "old");
        assert_eq!(store.data.search_history, vec!["query"]);
        assert_eq!(store.data.history[0]["id"], "heard");
        assert_eq!(store.data.playlists.len(), 1);
        assert_eq!(store.data.settings["volume"], 0.2);
        assert_eq!(store.data.profile["name"], "Current");
        let safety_id = restored["safety_backup_id"].as_str().unwrap();
        store.restore_backup(safety_id).unwrap();
        assert_eq!(store.data.liked_tracks[0]["id"], "new");
        let reopened = LibraryStore::open(store.path.clone()).unwrap();
        assert_eq!(reopened.data.liked_tracks[0]["id"], "new");
    }

    #[test]
    fn automatic_backup_captures_previous_disk_state_and_obeys_interval() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store(&dir);
        store.data.liked_tracks.push(track("new"));
        store.save().unwrap();
        store.save().unwrap();
        let entries = store.backup_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["kind"], "automatic");
        let saved = store.read_backup(entries[0]["id"].as_str().unwrap()).unwrap();
        assert!(saved.content.liked_tracks.is_empty());
        // Age the fixture to make the next save eligible without sleeping.
        let mut aged = saved;
        aged.created_at = now() - BACKUP_INTERVAL_SECONDS - 1.0;
        fs::write(store.backup_dir().unwrap().join(entries[0]["id"].as_str().unwrap()),
            serde_json::to_vec(&aged).unwrap()).unwrap();
        store.last_automatic_backup.set(None);
        store.save().unwrap();
        assert_eq!(store.backup_entries().unwrap().len(), 2);
    }

    #[test]
    fn rotation_limits_each_kind_independently() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        for _ in 0..12 { store.write_backup(&store.data, "manual").unwrap(); }
        for _ in 0..12 { store.write_backup(&store.data, "automatic").unwrap(); }
        for _ in 0..5 { store.write_backup(&store.data, "before_restore").unwrap(); }
        let entries = store.backup_entries().unwrap();
        assert_eq!(entries.iter().filter(|e| e["kind"] == "manual").count(), 10);
        assert_eq!(entries.iter().filter(|e| e["kind"] == "automatic").count(), 10);
        assert_eq!(entries.iter().filter(|e| e["kind"] == "before_restore").count(), 3);
    }

    #[test]
    fn invalid_restore_does_not_change_memory_or_disk_or_create_safety_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store(&dir);
        let saved = store.create_backup().unwrap();
        let before = fs::read(&store.path).unwrap();
        for id in ["../library_guest.json", "C:\\outside.json", "", "foo.json/../bar.json"] {
            assert!(store.restore_backup(id).is_err());
        }
        let id = saved["id"].as_str().unwrap();
        let path = store.backup_dir().unwrap().join(id);
        let mut damaged: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        damaged["content"]["liked_tracks"] = json!([{"id":"bad"}]);
        fs::write(&path, serde_json::to_vec(&damaged).unwrap()).unwrap();
        assert!(store.restore_backup(id).is_err());
        assert!(store.data.liked_tracks.is_empty());
        assert_eq!(fs::read(&store.path).unwrap(), before);
        assert!(store.backup_entries().unwrap().is_empty());
        assert_eq!(fs::read_dir(store.backup_dir().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn restore_rolls_back_memory_when_destination_cannot_be_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store(&dir);
        let saved = store.create_backup().unwrap();
        store.data.liked_tracks.push(track("keep"));
        // Point this test store at a directory: atomic file replacement must fail.
        let blocked_path = dir.path().join("blocked.json");
        fs::create_dir(&blocked_path).unwrap();
        let source = store.backup_dir().unwrap().join(saved["id"].as_str().unwrap());
        store.path = blocked_path;
        fs::create_dir_all(store.backup_dir().unwrap()).unwrap();
        fs::copy(source, store.backup_dir().unwrap().join(saved["id"].as_str().unwrap())).unwrap();
        assert!(store.restore_backup(saved["id"].as_str().unwrap()).is_err());
        assert_eq!(store.data.liked_tracks[0]["id"], "keep");
        assert!(store.path.is_dir());
        assert_eq!(store.backup_entries().unwrap().iter()
            .filter(|e| e["kind"] == "before_restore").count(), 1);
    }

    #[test]
    fn unsupported_backup_version_is_rejected_before_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = store(&dir);
        let saved = store.create_backup().unwrap();
        let path = store.backup_dir().unwrap().join(saved["id"].as_str().unwrap());
        let mut backup: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        backup["version"] = json!(999);
        fs::write(path, serde_json::to_vec(&backup).unwrap()).unwrap();
        assert!(store.restore_backup(saved["id"].as_str().unwrap()).is_err());
        assert_eq!(fs::read_dir(store.backup_dir().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn backups_are_account_scoped_and_restore_strips_local_paths() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = store(&dir);
        let mut local = track("local");
        local["local_path"] = json!("/private/file.mp3");
        first.data.liked_tracks.push(local);
        let saved = first.create_backup().unwrap();
        let mut second = LibraryStore::open(dir.path().join("library_other.json")).unwrap();
        assert!(second.backup_entries().unwrap().is_empty());
        assert!(second.restore_backup(saved["id"].as_str().unwrap()).is_err());
        first.restore_backup(saved["id"].as_str().unwrap()).unwrap();
        assert!(first.data.liked_tracks[0].get("local_path").is_none());
    }
}

pub fn legacy_library_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("SoundFlow").join("library.json"))
}
pub fn read_legacy(path: &Path) -> CoreResult<String> {
    if !path.is_file() || fs::metadata(path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Бібліотека відсутня або завелика".into());
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}
