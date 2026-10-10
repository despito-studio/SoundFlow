use crate::{
    cloud::CloudClient,
    library::{legacy_library_path, now, read_legacy, CoreResult, LibraryStore, Playlist},
    listening::ListeningStore,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub struct Core {
    pub library: LibraryStore,
    pub listening: ListeningStore,
    pub cloud: CloudClient,
    pub data_dir: PathBuf,
    pub local_files: HashMap<String, PathBuf>,
    pub approved_roots: Vec<PathBuf>,
    current_user: String,
}

pub fn arg<'a>(args: &'a [Value], index: usize) -> &'a Value {
    args.get(index).unwrap_or(&Value::Null)
}
pub fn string_arg(args: &[Value], index: usize) -> CoreResult<&str> {
    arg(args, index)
        .as_str()
        .ok_or_else(|| format!("Очікується текстовий аргумент {index}"))
}
pub(crate) fn account_file(data_dir: &Path, user: &str) -> PathBuf {
    if user == "guest" {
        data_dir.join("library.json")
    } else {
        data_dir.join(format!(
            "library_{:x}.json",
            Sha256::digest(user.as_bytes())
        ))
    }
}

impl Core {
    pub fn open(data_dir: PathBuf, offline: bool) -> CoreResult<Self> {
        fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
        let library = LibraryStore::open(account_file(&data_dir, "guest"))?;
        let listening = ListeningStore::open(&data_dir.join("listening.sqlite3"))?;
        let cloud = CloudClient::open(&data_dir, offline)?;
        let music = dirs::audio_dir().and_then(|p| p.canonicalize().ok());
        let mut approved_roots = music.into_iter().collect::<Vec<_>>();
        let saved: Value = fs::read_to_string(data_dir.join("local-roots.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null);
        if let Some(roots) = saved.as_array() {
            for root in roots.iter().filter_map(Value::as_str) {
                if let Ok(path) = Path::new(root).canonicalize() {
                    if path.is_dir() && !approved_roots.contains(&path) {
                        approved_roots.push(path)
                    }
                }
            }
        }
        let mut core = Self {
            library,
            listening,
            cloud,
            data_dir,
            local_files: HashMap::new(),
            approved_roots,
            current_user: "guest".into(),
        };
        core.restore_local_files();
        Ok(core)
    }
    fn switch_account(&mut self) -> CoreResult<()> {
        let user = self.cloud.user_id().unwrap_or_else(|| "guest".into());
        if user != self.current_user {
            let mut library = match LibraryStore::open(account_file(&self.data_dir, &user)) {
                Ok(library) => library,
                Err(error) => {
                    self.cloud.invalidate_session();
                    return Err(error);
                }
            };
            if let Some(profile) = self.cloud.status().get("user").filter(|v| v.is_object()) {
                let _ = library.update_profile(profile);
            }
            self.library = library;
            self.current_user = user;
            self.local_files.clear();
            self.restore_local_files();
        }
        Ok(())
    }
    pub fn user(&self) -> &str {
        &self.current_user
    }
    pub fn settings_bool(&self, key: &str) -> bool {
        self.library
            .data
            .settings
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
    pub fn local_rpc(&mut self, method: &str, args: &[Value]) -> CoreResult<Value> {
        let may_change_library = matches!(
            method,
            "toggle_like"
                | "create_playlist"
                | "delete_playlist"
                | "rename_playlist"
                | "add_to_playlist"
                | "remove_from_playlist"
                | "record_history"
                | "clear_history"
                | "update_profile"
                | "update_settings"
                | "add_search_query"
                | "remove_search_query"
                | "clear_search_history"
                | "import_library"
                | "import_legacy_library"
                | "configure_cloud"
                | "restore_session"
                | "sign_in"
                | "sign_up"
                | "sign_out"
                | "sync_library"
        );
        let before = if may_change_library {
            Some(self.library.clone())
        } else {
            None
        };
        let result = self.local_rpc_inner(method, args);
        if result.is_err() {
            if let Some(before) = before {
                self.library = before;
            }
        }
        result
    }
    fn local_rpc_inner(&mut self, method: &str, args: &[Value]) -> CoreResult<Value> {
        match method {
            "get_library" => Ok(self.library.snapshot()),
            "toggle_like" => self.library.toggle_like(arg(args, 0)),
            "is_liked" => Ok(json!(self
                .library
                .data
                .liked_tracks
                .iter()
                .any(|t| t["id"] == *arg(args, 0)))),
            "create_playlist" => self.library.create_playlist(string_arg(args, 0)?),
            "delete_playlist" => Ok(json!(self.library.delete_playlist(string_arg(args, 0)?)?)),
            "rename_playlist" => Ok(json!(self
                .library
                .rename_playlist(string_arg(args, 0)?, string_arg(args, 1)?)?)),
            "add_to_playlist" => Ok(json!(self
                .library
                .add_to_playlist(string_arg(args, 0)?, arg(args, 1))?)),
            "remove_from_playlist" => Ok(json!(self
                .library
                .remove_track(string_arg(args, 0)?, string_arg(args, 1)?)?)),
            "record_history" => Ok(json!(self.library.record_history(arg(args, 0))?)),
            "clear_history" => {
                self.listening.clear(self.user())?;
                Ok(json!(self.library.clear_history()?))
            }
            "get_profile" => Ok(Value::Object(self.library.data.profile.clone())),
            "get_user_sf_id" => Ok(self
                .library
                .data
                .profile
                .get("sf_id")
                .cloned()
                .unwrap_or(Value::Null)),
            "update_profile" => self.library.update_profile(arg(args, 0)),
            "get_settings" => Ok(Value::Object(self.library.data.settings.clone())),
            "update_settings" => Ok(json!(self.library.update_settings(arg(args, 0))?)),
            "get_search_history" => Ok(json!(self.library.data.search_history)),
            "add_search_query" => self.library.add_search(string_arg(args, 0)?),
            "remove_search_query" => {
                let query = string_arg(args, 0)?;
                self.library.data.search_history.retain(|q| q != query);
                self.library.save()?;
                Ok(json!(self.library.data.search_history))
            }
            "clear_search_history" => {
                self.library.data.search_history.clear();
                self.library.save()?;
                Ok(json!([]))
            }
            "export_library" => Ok(json!(serde_json::to_string_pretty(
                &self.library.snapshot()
            )
            .map_err(|e| e.to_string())?)),
            "import_library" => {
                let result = self.library.import_json(string_arg(args, 0)?, false)?;
                self.refresh_local_files();
                Ok(result)
            }
            "get_legacy_library_status" => {
                let path = legacy_active_path();
                Ok(
                    json!({"available":path.as_ref().is_some_and(|p|p.is_file()),"path":path,"automatic":false}),
                )
            }
            "import_legacy_library" => {
                let path = legacy_active_path().ok_or("Попередню бібліотеку не знайдено")?;
                let raw = read_legacy(&path)?;
                let old_user = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.strip_prefix("library_"))
                    .unwrap_or("guest");
                let user = self.current_user.clone();
                let mut result = self.library.import_json(&raw, true)?;
                self.refresh_local_files();
                match self.listening.migrate_legacy(
                    &path
                        .parent()
                        .ok_or("Недійсна папка бібліотеки")?
                        .join("listening.sqlite3"),
                    old_user,
                    &user,
                ) {
                    Ok(count) => result["listening_imported"] = json!(count),
                    Err(error) => result["listening_warning"] = json!(error),
                }
                Ok(result)
            }
            "record_listening" => self.listening.record(
                self.user(),
                arg(args, 0),
                string_arg(args, 1)?,
                arg(args, 2),
                arg(args, 3),
            ),
            "get_subscription_status" => Ok(self.cloud.entitlement()),
            "get_listening_stats" => {
                let subscription = self.cloud.entitlement();
                if subscription["active"] != true {
                    return Ok(
                        json!({"ok":false,"locked":true,"subscription":subscription,"error":"Статистика доступна з SoundFlow Plus за $1/місяць."}),
                    );
                }
                self.listening.summary(
                    self.user(),
                    arg(args, 0).as_str().unwrap_or("30d"),
                    arg(args, 1).as_str().unwrap_or("all"),
                )
            }
            "import_listening_history" => {
                let subscription = self.cloud.entitlement();
                if subscription["active"] != true {
                    return Ok(
                        json!({"ok":false,"locked":true,"subscription":subscription,"error":"Для імпорту статистики потрібна активна підписка."}),
                    );
                }
                let user = self.current_user.clone();
                self.listening
                    .import(&user, string_arg(args, 0)?, string_arg(args, 1)?)
            }
            "get_cloud_status" => Ok(self.cloud.status()),
            "configure_cloud" => {
                let status = self.cloud.configure(arg(args, 0))?;
                self.switch_account()?;
                Ok(status)
            }
            "restore_session" => {
                let status = self.cloud.restore()?;
                self.switch_account()?;
                Ok(status)
            }
            "sign_in" | "sign_up" => {
                let status = self.cloud.sign_in(arg(args, 0), method == "sign_up")?;
                self.switch_account()?;
                Ok(status)
            }
            "sign_out" => {
                let status = self.cloud.sign_out()?;
                self.switch_account()?;
                Ok(status)
            }
            "create_subscription_checkout" => self.cloud.checkout(),
            "sync_library" => {
                let mode = arg(args, 0).as_str().unwrap_or("push");
                if mode == "pull" || mode == "merge" {
                    let result = self.cloud.fetch_library()?;
                    let music = result
                        .get("library")
                        .ok_or("Сервер не повернув бібліотеку")?;
                    if let Err(error) = self
                        .library
                        .import_json(&music.to_string(), mode == "merge")
                    {
                        self.cloud.forget_revision();
                        return Err(error);
                    }
                    self.refresh_local_files();
                    Ok(json!({"ok":true,"mode":mode,"revision":result["revision"]}))
                } else if mode == "push" {
                    if self.cloud.status()["sync_revision"].is_null() {
                        let remote = self.cloud.fetch_library()?;
                        let music = &remote["library"];
                        let has_remote_data = music["liked_tracks"]
                            .as_array()
                            .is_none_or(|tracks| !tracks.is_empty())
                            || music["history"]
                                .as_array()
                                .is_some_and(|tracks| !tracks.is_empty())
                            || music["playlists"]
                                .as_object()
                                .is_some_and(|playlists| !playlists.is_empty());
                        if has_remote_data {
                            self.cloud.forget_revision();
                            return Err("reconciliation_needed: У хмарі вже є бібліотека. Спочатку завантажте її або явно об'єднайте з локальною.".into());
                        }
                    }
                    self.cloud.push_library(&self.library.snapshot())
                } else {
                    Err("Оберіть pull, merge або push для синхронізації".into())
                }
            }
            "get_connected_accounts" => Ok(self
                .library
                .data
                .profile
                .get("connected_accounts")
                .cloned()
                .unwrap_or(json!({}))),
            "get_game_status" => Ok(json!({"is_gaming":false})),
            "clear_cache" => Ok(
                json!({"ok":true,"message":"Кеш медіапосилань очищається після перезапуску музичного рушія."}),
            ),
            "trim_memory" => Ok(json!(true)),
            "set_user" => Err("Користувача визначає перевірений сеанс SoundFlow".into()),
            _ => Err(format!("Непідтримуваний метод: {method}")),
        }
    }
    pub fn import_provider_tracks(
        &mut self,
        provider: &str,
        identifier: &str,
        tracks: &Value,
    ) -> CoreResult<Value> {
        if !["youtube", "soundcloud", "spotify"].contains(&provider) {
            return Err("Непідтримуваний сервіс".into());
        }
        let tracks = tracks.as_array().ok_or("Недійсний список імпорту")?;
        if tracks.is_empty() {
            return Ok(json!({"success":false,"error":"Не знайдено треків для імпорту"}));
        }
        let validated = tracks
            .iter()
            .take(200)
            .map(crate::library::validated_track)
            .collect::<CoreResult<Vec<_>>>()?;
        let before = self.library.clone();
        let id = format!("pl_{provider}_imported");
        let playlist = self
            .library
            .data
            .playlists
            .entry(id.clone())
            .or_insert(Playlist {
                id: id.clone(),
                name: format!("{} Імпорт", provider),
                created_at: now(),
                tracks: vec![],
                is_auto: false,
            });
        let mut added = 0;
        for track in validated {
            if !playlist.tracks.iter().any(|t| t["id"] == track["id"]) {
                playlist.tracks.push(track);
                added += 1
            }
        }
        let name = playlist.name.clone();
        let accounts = self
            .library
            .data
            .profile
            .entry("connected_accounts")
            .or_insert(json!({}));
        accounts[provider] = json!(identifier);
        if let Err(error) = self.library.save() {
            self.library = before;
            return Err(error);
        }
        Ok(json!({"success":true,"count":added,"playlist_id":id,"playlist_name":name}))
    }
    pub fn approve_root(&mut self, path: &Path) -> CoreResult<PathBuf> {
        let root = path
            .canonicalize()
            .map_err(|_| "Музичну папку не знайдено")?;
        if !root.is_dir() {
            return Err("Оберіть папку музики".into());
        }
        if !self.approved_roots.contains(&root) {
            self.approved_roots.push(root.clone());
            fs::write(
                self.data_dir.join("local-roots.json"),
                serde_json::to_vec(&self.approved_roots).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(root)
    }
    pub fn scan(&mut self, root: &Path) -> CoreResult<Value> {
        let root = root
            .canonicalize()
            .map_err(|_| "Музичну папку не знайдено")?;
        if !self.approved_roots.contains(&root) {
            return Err("Оберіть музичну папку через системний діалог".into());
        }
        let mut tracks = vec![];
        for entry in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = match entry.path().canonicalize() {
                Ok(p) if p.starts_with(&root) && is_audio(&p) => p,
                _ => continue,
            };
            let filename = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Музика");
            let (artist, title) = filename
                .split_once(" - ")
                .unwrap_or(("Локальний трек", filename));
            let id = format!(
                "local_{:x}",
                Sha256::digest(path.to_string_lossy().to_lowercase().as_bytes())
            );
            let uri = url::Url::from_file_path(&path).map_err(|_| "Недійсний шлях музики")?;
            let track = json!({"id":id,"title":title,"artist":artist,"duration":0,"duration_formatted":"--:--","thumbnail":"assets/icon.png","url":uri.as_str(),"source":"local","is_local":true});
            self.local_files.insert(id, path);
            tracks.push(track);
            if tracks.len() >= 300 {
                break;
            }
        }
        let count = tracks.len();
        self.library.data.playlists.insert(
            "pl_local_music".into(),
            Playlist {
                id: "pl_local_music".into(),
                name: "Локальна музика".into(),
                created_at: now(),
                tracks,
                is_auto: true,
            },
        );
        self.library.save()?;
        Ok(json!({"success":true,"count":count,"playlist_id":"pl_local_music"}))
    }
    fn restore_local_files(&mut self) {
        for track in self.library.all_tracks() {
            if track["source"] != "local" {
                continue;
            }
            if let (Some(id), Some(raw)) = (track["id"].as_str(), track["url"].as_str()) {
                if let Ok(url) = url::Url::parse(raw) {
                    if let Ok(path) = url.to_file_path() {
                        if let Ok(path) = path.canonicalize() {
                            if is_audio(&path)
                                && self.approved_roots.iter().any(|r| path.starts_with(r))
                            {
                                self.local_files.insert(id.into(), path);
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn refresh_local_files(&mut self) {
        self.local_files.clear();
        self.restore_local_files();
    }
    pub fn local_path(&self, track: &Value) -> CoreResult<PathBuf> {
        let id = track["id"].as_str().ok_or("Недійсний локальний трек")?;
        let path = self
            .local_files
            .get(id)
            .ok_or("Оберіть локальну папку музики повторно")?;
        if path.canonicalize().ok().as_ref() != Some(path)
            || !self
                .approved_roots
                .iter()
                .any(|root| path.starts_with(root))
            || !path.is_file()
            || !is_audio(path)
            || !self
                .library
                .all_tracks()
                .iter()
                .any(|t| t["id"] == id && t["source"] == "local")
        {
            return Err("Локальний трек відсутній у бібліотеці".into());
        }
        Ok(path.clone())
    }
}
pub fn is_audio(path: &Path) -> bool {
    ["mp3", "m4a", "flac", "wav", "ogg", "opus", "webm", "aac"].contains(
        &path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
    )
}
fn legacy_active_path() -> Option<PathBuf> {
    let guest = legacy_library_path()?;
    let parent = guest.parent()?;
    if let Ok(raw) = fs::read_to_string(parent.join("session.json")) {
        if let Ok(session) = serde_json::from_str::<Value>(&raw) {
            if let Some(id) = session["last_user_id"].as_str() {
                if !id.is_empty()
                    && id.len() <= 200
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    let file = parent.join(format!("library_{id}.json"));
                    if file.is_file() {
                        return Some(file);
                    }
                }
            }
        }
    }
    Some(guest)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dispatcher_crud_and_stats_gate() {
        let dir = tempfile::tempdir().unwrap();
        let mut core = Core::open(dir.path().into(), true).unwrap();
        let track = json!({"id":"a","title":"Song","source":"youtube"});
        let playlist = core.local_rpc("create_playlist", &[json!("Test")]).unwrap();
        let id = playlist["id"].clone();
        assert_eq!(
            core.local_rpc("add_to_playlist", &[id.clone(), track])
                .unwrap(),
            true
        );
        assert_eq!(
            core.local_rpc("get_library", &[]).unwrap()["playlists"][id.as_str().unwrap()]
                ["tracks"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            core.local_rpc("get_listening_stats", &[]).unwrap()["locked"],
            true
        );
        assert_eq!(core.local_rpc("delete_playlist", &[id]).unwrap(), true);
        assert!(core.local_rpc("run_shell", &[]).is_err());
    }
    #[test]
    fn local_files_require_approved_audio() {
        let dir = tempfile::tempdir().unwrap();
        let music = tempfile::tempdir().unwrap();
        fs::write(music.path().join("Artist - Song.mp3"), b"audio").unwrap();
        fs::write(music.path().join("secret.txt"), b"secret").unwrap();
        let mut core = Core::open(dir.path().into(), true).unwrap();
        assert!(core.scan(music.path()).is_err());
        core.approve_root(music.path()).unwrap();
        assert_eq!(core.scan(music.path()).unwrap()["count"], 1);
        let track = core.library.data.playlists["pl_local_music"].tracks[0].clone();
        assert!(core
            .local_path(&track)
            .unwrap()
            .ends_with("Artist - Song.mp3"));
        let mut forged = track.clone();
        forged["url"] = json!("file:///C:/Users/private/secret.mp3");
        assert_eq!(
            core.local_path(&forged).unwrap(),
            core.local_path(&track).unwrap()
        );
        forged["id"] = json!("unknown-local-track");
        assert!(core.local_path(&forged).is_err());
    }
}
