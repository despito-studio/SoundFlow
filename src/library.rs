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
        "hotkeys_enabled":true,"tray_enabled":true,"local_music_enabled":true,
        "volume_norm_enabled":true,"direct_paste_enabled":true,"smart_cache_enabled":true,
        "offline_download_enabled":true,"infinite_autoplay":true,"theme":"light"})
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

#[derive(Clone)]
pub struct LibraryStore {
    pub data: Library,
    path: PathBuf,
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
        let mut store = Self { data, path };
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
                "volume"
                    if value
                        .as_f64()
                        .is_some_and(|v| v.is_finite() && (0.0..=1.0).contains(&v)) => {}
                "repeat" if ["off", "all", "one"].contains(&value.as_str().unwrap_or_default()) => {
                }
                "theme"
                    if ["light", "dark", "system"]
                        .contains(&value.as_str().unwrap_or_default()) => {}
                "preferred_source"
                    if ["all", "youtube", "soundcloud", "spotify", "local"]
                        .contains(&value.as_str().unwrap_or_default()) => {}
                "shuffle"
                | "reduced_motion"
                | "hotkeys_enabled"
                | "tray_enabled"
                | "local_music_enabled"
                | "volume_norm_enabled"
                | "direct_paste_enabled"
                | "smart_cache_enabled"
                | "offline_download_enabled"
                | "infinite_autoplay"
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
        for field in ["name", "email", "avatar_url", "banner_url", "banner_preset"] {
            if let Some(value) = payload.get(field) {
                let clean = text(Some(value), if field.ends_with("url") { 2000 } else { 254 });
                if field.ends_with("url")
                    && !clean.is_empty()
                    && url::Url::parse(&clean)
                        .map(|u| u.scheme() != "https")
                        .unwrap_or(true)
                {
                    return Err("Зображення профілю повинно використовувати HTTPS".into());
                }
                self.data.profile.insert(field.into(), json!(clean));
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    fn track(id: &str) -> Value {
        json!({"id":id,"title":"Song","artist":"Artist","source":"youtube","url":"https://youtube.com/watch?v=a"})
    }
    #[test]
    fn likes_playlists_persist_and_dedupe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let mut store = LibraryStore::open(path.clone()).unwrap();
        assert_eq!(store.toggle_like(&track("a")).unwrap()["is_liked"], true);
        let id = store.create_playlist("Тест").unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(store.add_to_playlist(&id, &track("a")).unwrap());
        assert!(!store.add_to_playlist(&id, &track("a")).unwrap());
        let loaded = LibraryStore::open(path).unwrap();
        assert_eq!(loaded.data.liked_tracks.len(), 1);
        assert_eq!(loaded.data.playlists[&id].tracks.len(), 1);
    }
    #[test]
    fn import_does_not_import_roles_or_entitlements() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = LibraryStore::open(dir.path().join("library.json")).unwrap();
        store
            .import_json(
                &json!({"liked_tracks":[track("a")],"profile":{"role":"admin"},"active":true})
                    .to_string(),
                true,
            )
            .unwrap();
        assert_eq!(store.data.profile["role"], "listener");
        assert!(!store.snapshot().get("active").is_some());
        assert!(store.import_json("{\"liked_tracks\":[{}]}", true).is_err());
        assert_eq!(store.data.liked_tracks.len(), 1);
    }
    #[test]
    fn corrupt_library_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        fs::write(&path, "broken").unwrap();
        assert!(LibraryStore::open(path.clone()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken");
    }
    #[test]
    fn settings_validate_all_values_before_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = LibraryStore::open(dir.path().join("library.json")).unwrap();
        assert!(store
            .update_settings(&json!({"volume":0.2,"active":true}))
            .is_err());
        assert_eq!(store.data.settings["volume"], 0.8);
    }
    #[test]
    fn repeated_import_merges_playlists_without_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = LibraryStore::open(dir.path().join("library.json")).unwrap();
        let raw=json!({"liked_tracks":[track("a")],"playlists":{"old":{"id":"old","name":"Old playlist","tracks":[track("a")]}}}).to_string();
        store.import_json(&raw, true).unwrap();
        store.import_json(&raw, true).unwrap();
        assert_eq!(store.data.liked_tracks.len(), 1);
        assert_eq!(store.data.playlists.len(), 1);
        assert_eq!(store.data.playlists["old"].tracks.len(), 1);
    }
}
