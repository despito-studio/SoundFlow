//! Native commands. Slow provider and cloud calls execute outside the UI thread.
use crate::{
    core::{self, arg, string_arg, Core},
    library::{self, CoreResult},
    media::ProviderBridge,
};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::Mutex,
};

pub struct Desktop {
    pub core: Mutex<Core>,
    provider: Mutex<ProviderBridge>,
    pub data_dir: PathBuf,
    audio_listening: Mutex<crate::listening::ListeningStore>,
    audio_barriers: Mutex<HashMap<String, f64>>,
    recorded_history: Mutex<HashSet<String>>,
}
fn lock_error<T>(_: std::sync::PoisonError<T>) -> String {
    "Локальне сховище тимчасово недоступне".into()
}
impl Desktop {
    pub fn open(data_dir: PathBuf, resources: PathBuf, offline: bool) -> CoreResult<Self> {
        let core = Core::open(data_dir.clone(), offline)?;
        let audio_listening =
            crate::listening::ListeningStore::open(&data_dir.join("listening.sqlite3"))?;
        Ok(Self {
            core: Mutex::new(core),
            audio_listening: Mutex::new(audio_listening),
            provider: Mutex::new(ProviderBridge::new(resources, data_dir.clone())),
            data_dir,
            audio_barriers: Mutex::new(HashMap::new()),
            recorded_history: Mutex::new(HashSet::new()),
        })
    }
    pub fn call(&self, method: &str, args: &[Value]) -> CoreResult<Value> {
        if method.len() > 80 || args.len() > 12 {
            return Err("Недійсний запит SoundFlow".into());
        }
        match method {
            "clear_history" => {
                let mut core = self.core.lock().map_err(lock_error)?;
                self.audio_barriers
                    .lock()
                    .map_err(lock_error)?
                    .insert(core.user().into(), library::now());
                core.local_rpc(method, args)
            }
            "get_discover_tracks" => {
                let mut args = args.to_vec();
                if args.first() == Some(&json!("deep")) {
                    args[0] = json!("deephouse");
                }
                self.provider
                    .lock()
                    .map_err(lock_error)?
                    .call(method, &args)
            }
            "search"
            | "resolve_direct_url"
            | "get_similar_tracks"
            | "get_infinite_recommendations"
            | "match_spotify_track"
            | "download_track"
            | "get_downloads"
            | "get_download_job"
            | "cancel_download"
            | "provider_ping" => self.provider.lock().map_err(lock_error)?.call(
                if method == "provider_ping" {
                    "ping"
                } else {
                    method
                },
                args,
            ),
            "get_stream" => {
                if arg(args, 0)["source"] == "local" || arg(args, 0)["is_local"] == true {
                    Ok(json!(self
                        .core
                        .lock()
                        .map_err(lock_error)?
                        .local_path(arg(args, 0))?
                        .to_string_lossy()))
                } else {
                    let value = self
                        .provider
                        .lock()
                        .map_err(lock_error)?
                        .call("get_playback_file", args)?;
                    let path = PathBuf::from(value.as_str().ok_or("Недійсний аудіокеш")?)
                        .canonicalize()
                        .map_err(|_| "Аудіокеш недоступний")?;
                    let root = self
                        .data_dir
                        .join("provider-audio")
                        .canonicalize()
                        .map_err(|_| "Аудіокеш недоступний")?;
                    if !path.starts_with(root) || !path.is_file() || !core::is_audio(&path) {
                        return Err("Недійсний файл аудіокешу".into());
                    }
                    Ok(json!(path.to_string_lossy()))
                }
            }
            "import_external_profile" => {
                let user = self.core.lock().map_err(lock_error)?.user().to_owned();
                let tracks = self
                    .provider
                    .lock()
                    .map_err(lock_error)?
                    .call(method, args)?;
                let mut core = self.core.lock().map_err(lock_error)?;
                if core.user() != user {
                    return Err("Профіль змінився. Повторіть імпорт.".into());
                }
                core.import_provider_tracks(string_arg(args, 0)?, string_arg(args, 1)?, &tracks)
            }
            "detect_connected_accounts" => {
                let email = string_arg(args, 0).unwrap_or("");
                let name = string_arg(args, 1).unwrap_or("");
                let detected = self.provider.lock().map_err(lock_error)?.call(
                    "detect_connected_accounts",
                    &[json!(email), json!(name)],
                )?;
                let mut core = self.core.lock().map_err(lock_error)?;
                let saved = core.local_rpc("get_connected_accounts", &[])?;
                let mut merged = detected.clone();
                // Позначаємо сервіси, які справді знайдені через Google-профіль
                // (а не підключені вручну раніше): за цим прапорцем застосунок
                // питає про імпорт бібліотеки одразу після входу.
                if let Some(obj) = merged.as_object_mut() {
                    for info in obj.values_mut() {
                        let found = info["found"].as_bool() == Some(true);
                        let identified = info["identifier"]
                            .as_str()
                            .is_some_and(|id| !id.trim().is_empty());
                        if let Some(map) = info.as_object_mut() {
                            map.insert("google_linked".into(), json!(found && identified));
                        }
                    }
                }
                if let Some(saved_obj) = saved.as_object() {
                    for (provider, val) in saved_obj {
                        if let Some(id_str) = val.as_str() {
                            if !id_str.is_empty() {
                                if let Some(m_obj) = merged.get_mut(provider) {
                                    m_obj["found"] = json!(true);
                                    m_obj["identifier"] = json!(id_str);
                                    if m_obj["name"].as_str().is_none_or(str::is_empty) {
                                        m_obj["name"] = json!(id_str);
                                    }
                                }
                            }
                        }
                    }
                }
                let user_id = core.user().to_string();
                if let Ok(sup_accounts) = crate::supabase::fetch_connected_accounts(&user_id) {
                    if let Some(sup_obj) = sup_accounts.as_object() {
                        for (provider, val) in sup_obj {
                            if let Some(id_str) = val.as_str() {
                                if !id_str.is_empty() {
                                    if let Some(m_obj) = merged.get_mut(provider) {
                                        m_obj["found"] = json!(true);
                                        m_obj["identifier"] = json!(id_str);
                                        if m_obj["name"].as_str().is_none_or(str::is_empty) {
                                            m_obj["name"] = json!(id_str);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(merged_obj) = merged.as_object() {
                    for (provider, info) in merged_obj {
                        if info["found"].as_bool() == Some(true) {
                            if let Some(id) = info["identifier"].as_str() {
                                if !id.is_empty() {
                                    let _ = core.local_rpc("set_connected_account", &[json!(provider), json!(id)]);
                                    crate::supabase::sync_one_account(&user_id, provider, id);
                                }
                            }
                        }
                    }
                }
                Ok(merged)
            }
            "set_connected_account" => {
                let provider = string_arg(args, 0)?;
                let identifier = string_arg(args, 1)?;
                let mut core = self.core.lock().map_err(lock_error)?;
                let user_id = core.user().to_string();
                let res = core.local_rpc("set_connected_account", &[json!(provider), json!(identifier)])?;
                crate::supabase::sync_one_account(&user_id, provider, identifier);
                Ok(res)
            }
            "sync_connected_accounts" => {
                let (accounts, user) = {
                    let mut core = self.core.lock().map_err(lock_error)?;
                    (
                        core.local_rpc("get_connected_accounts", &[])?,
                        core.user().to_owned(),
                    )
                };
                let mut details = serde_json::Map::new();
                let mut total = 0;
                // Можна передати один сервіс ("spotify") або список ("youtube,soundcloud").
                let targets: Vec<&str> = arg(args, 0)
                    .as_str()
                    .unwrap_or("")
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                if let Some(accounts) = accounts.as_object() {
                    for (provider, identifier) in accounts {
                        if !targets.is_empty() && !targets.iter().any(|t| *t == provider.as_str()) {
                            continue;
                        }
                        if identifier.as_str().is_none_or(str::is_empty) {
                            continue;
                        }
                        let result = self.provider.lock().map_err(lock_error)?.call(
                            "import_external_profile",
                            &[json!(provider), identifier.clone()],
                        );
                        let mut core = self.core.lock().map_err(lock_error)?;
                        if core.user() != user {
                            return Err("Профіль змінився. Повторіть синхронізацію.".into());
                        }
                        match result.and_then(|tracks| {
                            core.import_provider_tracks(
                                provider,
                                identifier.as_str().unwrap_or(""),
                                &tracks,
                            )
                        }) {
                            Ok(value) => {
                                total += value["count"].as_u64().unwrap_or(0);
                                details.insert(provider.clone(), value);
                            }
                            Err(error) => {
                                details.insert(
                                    provider.clone(),
                                    json!({"success":false,"error":error}),
                                );
                            }
                        }
                    }
                }
                Ok(json!({"success":true,"total_imported":total,"details":details}))
            }
            "scan_local_music" => {
                let selected = arg(args, 0).is_null();
                let path = if let Some(raw) = arg(args, 0).as_str() {
                    PathBuf::from(raw)
                } else {
                    match rfd::FileDialog::new()
                        .set_title("Оберіть папку музики")
                        .pick_folder()
                    {
                        Some(path) => path,
                        None => return Ok(json!({"success":false,"cancelled":true,"count":0})),
                    }
                };
                let mut core = self.core.lock().map_err(lock_error)?;
                if selected {
                    core.approve_root(&path)?;
                }
                core.scan(&path)
            }
            "export_library_file" => {
                let raw = self
                    .core
                    .lock()
                    .map_err(lock_error)?
                    .local_rpc("export_library", &[])?;
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Зберегти бібліотеку SoundFlow")
                    .add_filter("SoundFlow JSON", &["json"])
                    .set_file_name("soundflow-library.json")
                    .save_file()
                else {
                    return Ok(json!({"success":false,"cancelled":true}));
                };
                fs::write(&path, raw.as_str().ok_or("Недійсна бібліотека")?)
                    .map_err(|_| "Не вдалося зберегти бібліотеку")?;
                Ok(json!({"success":true,"path":path}))
            }
            "import_library_file" => {
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Імпортувати бібліотеку SoundFlow")
                    .add_filter("SoundFlow JSON", &["json"])
                    .pick_file()
                else {
                    return Ok(json!({"success":false,"cancelled":true}));
                };
                let raw = library::read_legacy(&path)?;
                let mut core = self.core.lock().map_err(lock_error)?;
                let previous = core.library.clone();
                match core
                    .library
                    .import_json(&raw, arg(args, 0).as_bool().unwrap_or(true))
                {
                    Ok(value) => {
                        core.refresh_local_files();
                        Ok(value)
                    }
                    Err(error) => {
                        core.library = previous;
                        Err(error)
                    }
                }
            }
            "import_listening_history_file" => {
                let provider = string_arg(args, 0)?;
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Імпортувати історію прослуховування")
                    .add_filter("JSON", &["json"])
                    .pick_file()
                else {
                    return Ok(json!({"success":false,"cancelled":true}));
                };
                let raw = library::read_legacy(&path)?;
                self.core
                    .lock()
                    .map_err(lock_error)?
                    .local_rpc("import_listening_history", &[json!(provider), json!(raw)])
            }
            "open_external_url" => {
                open::that_detached(validated_url(string_arg(args, 0)?)?)
                    .map_err(|_| "Не вдалося відкрити посилання")?;
                Ok(json!(true))
            }
            "open_downloads_directory" => {
                let directory = self.data_dir.join("downloads");
                fs::create_dir_all(&directory).map_err(|_| "Папка завантажень недоступна")?;
                open::that_detached(&directory)
                    .map_err(|_| "Не вдалося відкрити папку завантажень")?;
                Ok(json!(true))
            }
            "reveal_download" => {
                let job = self
                    .provider
                    .lock()
                    .map_err(lock_error)?
                    .call("get_download_job", args)?;
                let path = PathBuf::from(
                    job.get("file_path")
                        .or_else(|| job.get("path"))
                        .and_then(Value::as_str)
                        .ok_or("Файл ще не завантажено")?,
                )
                .canonicalize()
                .map_err(|_| "Файл завантаження відсутній")?;
                let root = self
                    .data_dir
                    .join("downloads")
                    .canonicalize()
                    .map_err(|_| "Папка завантажень відсутня")?;
                if !path.starts_with(root) || !core::is_audio(&path) {
                    return Err("Недійсний файл завантаження".into());
                }
                #[cfg(windows)]
                {
                    std::process::Command::new("explorer.exe")
                        .arg(format!("/select,{}", path.display()))
                        .spawn()
                        .map_err(|_| "Не вдалося показати файл")?;
                }
                #[cfg(not(windows))]
                {
                    open::that_detached(path.parent().ok_or("Недійсна папка")?)
                        .map_err(|_| "Не вдалося показати файл")?;
                }
                Ok(json!(true))
            }
            "clear_cache" => {
                self.provider.lock().map_err(lock_error)?.stop();
                Ok(json!({"ok":true}))
            }
            "get_system_stats" => {
                let mut system = sysinfo::System::new();
                let pid = sysinfo::get_current_pid().map_err(|e| e.to_string())?;
                system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                Ok(
                    json!({"memory_mb":system.process(pid).map(|p| p.memory() as f64 / 1024.0 / 1024.0),"runtime":"native","provider_isolated":true}),
                )
            }
            "update_playback_state" => Ok(json!(true)),
            _ => self
                .core
                .lock()
                .map_err(lock_error)?
                .local_rpc(method, args),
        }
    }
    pub fn shutdown(&self) {
        if let Ok(mut provider) = self.provider.try_lock() {
            provider.stop();
        }
    }
    pub fn record_audio(&self, report: &crate::playback::ListeningReport) -> CoreResult<Value> {
        // Persist heard time before acquiring Core, which may be waiting on a
        // cloud response. Holding the barrier through the SQLite write orders
        // this heartbeat against clear_history without blocking native events.
        let result = {
            let barriers = self.audio_barriers.lock().map_err(lock_error)?;
            if barriers
                .get(&report.user_id)
                .is_some_and(|barrier| report.started_at <= *barrier)
            {
                return Ok(json!({"ok":true,"discarded":true}));
            }
            self.audio_listening.lock().map_err(lock_error)?.record(
                &report.user_id,
                &report.track,
                &report.session_id,
                &json!(report.heard_seconds),
                &json!(report.started_at),
            )?
        };
        let mut core = self.core.lock().map_err(lock_error)?;
        if self
            .audio_barriers
            .lock()
            .map_err(lock_error)?
            .get(&report.user_id)
            .is_some_and(|barrier| report.started_at <= *barrier)
        {
            return Ok(json!({"ok":true,"discarded":true}));
        }
        let key = format!("{}:{}", report.user_id, report.session_id);
        if report.heard_seconds >= 10.0
            && !self
                .recorded_history
                .lock()
                .map_err(lock_error)?
                .contains(&key)
        {
            if core.user() == report.user_id {
                core.local_rpc("record_history", &[report.track.clone()])?;
            } else {
                let mut library = library::LibraryStore::open(core::account_file(
                    &self.data_dir,
                    &report.user_id,
                ))?;
                library.record_history(&report.track)?;
            }
            self.recorded_history
                .lock()
                .map_err(lock_error)?
                .insert(key);
        }
        Ok(result)
    }
}
fn validated_url(raw: &str) -> CoreResult<String> {
    let url = url::Url::parse(raw).map_err(|_| "Недійсна адреса посилання")?;
    let host = url.host_str().unwrap_or("");
    let permitted = [
        "youtube.com",
        "youtu.be",
        "soundcloud.com",
        "spotify.com",
        "music.apple.com",
        "checkout.stripe.com",
        "billing.stripe.com",
        "stats.fm",
        "soundflow.app",
    ]
    .iter()
    .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")));
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|p| p != 443)
        || !permitted
    {
        return Err(
            "SoundFlow відкриває лише HTTPS посилання підтримуваних музичних сервісів".into(),
        );
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::ListeningReport;
    fn report(user: &str, session: &str, started_at: f64) -> ListeningReport {
        ListeningReport {
            track: json!({"id":"yt-native","title":"Native track","artist":"SoundFlow","source":"youtube","duration":120,"url":"https://www.youtube.com/watch?v=native"}),
            user_id: user.into(),
            session_id: session.into(),
            started_at,
            heard_seconds: 15.0,
            history_due: true,
        }
    }
    #[test]
    fn cumulative_reports_preserve_captured_profile_and_history_once() {
        let folder = tempfile::tempdir().unwrap();
        let desktop = Desktop::open(folder.path().into(), folder.path().into(), true).unwrap();
        let captured = report("prior-profile", "native-session", library::now() - 20.0);
        desktop.record_audio(&captured).unwrap();
        desktop.record_audio(&captured).unwrap();
        let core = desktop.core.lock().unwrap();
        assert!(core.library.data.history.is_empty());
        let summary = core
            .listening
            .summary("prior-profile", "30d", "all")
            .unwrap();
        assert_eq!(summary["plays"], 1);
        assert_eq!(summary["total_seconds"], 15.0);
        assert_eq!(
            core.listening.summary("guest", "30d", "all").unwrap()["plays"],
            0
        );
        let prior = library::LibraryStore::open(core::account_file(folder.path(), "prior-profile"))
            .unwrap();
        assert_eq!(prior.data.play_counts.get("yt-native"), Some(&1));
    }
    #[test]
    fn clearing_history_rejects_queued_reports_from_prior_session() {
        let folder = tempfile::tempdir().unwrap();
        let desktop = Desktop::open(folder.path().into(), folder.path().into(), true).unwrap();
        let prior = report("guest", "prior-session", library::now() - 20.0);
        desktop.record_audio(&prior).unwrap();
        desktop.call("clear_history", &[]).unwrap();
        assert_eq!(desktop.record_audio(&prior).unwrap()["discarded"], true);
        let core = desktop.core.lock().unwrap();
        assert!(core.library.data.history.is_empty());
        assert_eq!(
            core.listening.summary("guest", "30d", "all").unwrap()["plays"],
            0
        );
    }
    #[test]
    fn heard_time_persists_while_cloud_commands_hold_core_lock() {
        let folder = tempfile::tempdir().unwrap();
        let desktop = std::sync::Arc::new(
            Desktop::open(folder.path().into(), folder.path().into(), true).unwrap(),
        );
        let guard = desktop.core.lock().unwrap();
        let worker = desktop.clone();
        let captured = report("guest", "network-session", library::now() - 20.0);
        let recording = std::thread::spawn(move || worker.record_audio(&captured));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if guard.listening.summary("guest", "30d", "all").unwrap()["plays"] == 1 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Audio persistence waited for Core"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        drop(guard);
        recording.join().unwrap().unwrap();
    }
}
