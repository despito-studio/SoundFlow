use crate::{
    desktop_shell::{DesktopShell, ShellEvent},
    native_session::NativeSession,
    platform::InstanceGuard,
};
use serde_json::{json, Value};
use std::{
    ffi::{c_char, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    sync::{atomic::Ordering, mpsc, Arc, Mutex, OnceLock},
};

struct Runtime {
    session: NativeSession,
    shell: DesktopShell,
    events: mpsc::Receiver<ShellEvent>,
    _instance: InstanceGuard,
    transfer: StateTransfer,
}

/// Top-level replacement patches: null is a value; deletion is explicit.
/// The baseline belongs to this open session and the first poll is always full.
#[derive(Default)]
struct StateTransfer {
    enabled: bool,
    previous: Option<Value>,
}

impl StateTransfer {
    fn new(config: &Value) -> Self {
        Self {
            enabled: config["state_patch_v1"].as_bool() == Some(true),
            previous: None,
        }
    }

    fn poll(&mut self, state: Value, events: Vec<Value>) -> Value {
        if !self.enabled {
            return json!({"ok": true, "state": state, "events": events});
        }
        let result = match (self.previous.as_ref().and_then(Value::as_object), state.as_object()) {
            (Some(previous), Some(current)) => {
                let set: serde_json::Map<String, Value> = current.iter()
                    .filter(|(key, value)| previous.get(*key) != Some(*value))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();
                let remove: Vec<&String> = previous.keys()
                    .filter(|key| !current.contains_key(*key))
                    .collect();
                json!({"ok": true, "state_patch": {"set": set, "remove": remove}, "events": events})
            }
            _ => json!({"ok": true, "state": state, "events": events}),
        };
        self.previous = Some(state);
        result
    }
}

static RUNTIME: OnceLock<Mutex<Option<Runtime>>> = OnceLock::new();

fn runtime() -> &'static Mutex<Option<Runtime>> {
    RUNTIME.get_or_init(|| Mutex::new(None))
}

fn response(f: impl FnOnce() -> Result<Value, String>) -> *mut c_char {
    let value = match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => json!({"ok": false, "error": error}),
        Err(_) => json!({"ok": false, "error": "Не вдалося виконати дію"}),
    };
    CString::new(value.to_string()).unwrap().into_raw()
}

unsafe fn request(bytes: *const u8, length: usize) -> Result<Value, String> {
    if bytes.is_null() || length == 0 || length > 2 * 1024 * 1024 {
        return Err("Недійсний запит інтерфейсу".into());
    }
    serde_json::from_slice(std::slice::from_raw_parts(bytes, length))
        .map_err(|_| "Недійсний формат запиту".into())
}

#[no_mangle]
pub unsafe extern "C" fn sf_open(bytes: *const u8, length: usize) -> *mut c_char {
    response(|| {
        let config = request(bytes, length)?;
        let mut lock = runtime().lock().map_err(|e| e.to_string())?;
        if let Some(rt) = lock.as_mut() {
            // Reopening also starts a new transfer baseline, including when an
            // older client attaches after a client that requested patches.
            rt.transfer = StateTransfer::new(&config);
            return Ok(json!({"ok": true, "state_patch_v1": rt.transfer.enabled}));
        }
        let instance = crate::platform::initialize_process()?;
        if !instance.is_primary() {
            return Ok(json!({"ok": false, "secondary": true}));
        }
        let resources = config["resources"]
            .as_str()
            .map(PathBuf::from)
            .ok_or("Не знайдено ресурси SoundFlow")?;
        let session = NativeSession::new(
            crate::platform::data_dir(),
            resources,
            config["offline"].as_bool().unwrap_or(false),
        )?;
        let (sender, events) = mpsc::channel();
        let activate = sender.clone();
        instance.listen_activation(move || {
            let _ = activate.send(ShellEvent::ShowMain);
        });
        let shell = DesktopShell::start_with_island_hotkeys(sender, Arc::new(|| {}), false);
        *lock = Some(Runtime {
            session,
            shell,
            events,
            _instance: instance,
            transfer: StateTransfer::new(&config),
        });
        Ok(json!({"ok": true, "state_patch_v1": config["state_patch_v1"].as_bool() == Some(true)}))
    })
}

#[no_mangle]
pub extern "C" fn sf_poll() -> *mut c_char {
    response(|| {
        let mut lock = runtime().lock().map_err(|e| e.to_string())?;
        let rt = lock.as_mut().ok_or("Аудіоядро ще не запущене")?;
        let mut events = Vec::new();
        while let Ok(event) = rt.events.try_recv() {
            match event {
                ShellEvent::TogglePlayback => rt.session.player_action("toggle"),
                ShellEvent::Next => rt.session.player_action("next"),
                ShellEvent::Previous => rt.session.player_action("previous"),
                ShellEvent::ShowMain => events.push(json!({"type": "show"})),
                ShellEvent::ToggleIsland { keyboard } => {
                    events.push(json!({"type": "island", "keyboard": keyboard}))
                }
                ShellEvent::Quit => events.push(json!({"type": "quit"})),
                ShellEvent::Error(error) => events.push(json!({"type": "error", "message": error})),
            }
        }
        let state = rt.session.snapshot_value();
        rt.shell.set_icon_selection(
            state["settings"]["desktop_icon"].as_str().unwrap_or("default"),
        );
        rt.shell.hotkeys_enabled.store(
            state["settings"]["hotkeys_enabled"]
                .as_bool()
                .unwrap_or(true),
            Ordering::Relaxed,
        );
        Ok(rt.transfer.poll(state, events))
    })
}

#[no_mangle]
pub unsafe extern "C" fn sf_command(bytes: *const u8, length: usize) -> *mut c_char {
    response(|| {
        let message = request(bytes, length)?;
        let lock = runtime().lock().map_err(|e| e.to_string())?;
        let rt = lock.as_ref().ok_or("Аудіоядро ще не запущене")?;
        let session = &rt.session;
        match message["type"].as_str().unwrap_or("") {
            "profile-image" => {
                let path = PathBuf::from(message["path"].as_str().ok_or("Зображення не вибране")?);
                let size = std::fs::metadata(&path)
                    .map_err(|_| "Файл недоступний")?
                    .len();
                if size == 0 || size > 12 * 1024 * 1024 {
                    return Err("Обери зображення до 12 МБ".into());
                }
                let data = std::fs::read(&path).map_err(|_| "Не вдалося прочитати фото")?;
                let extension = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
                    "png"
                } else if data.starts_with(b"\xff\xd8\xff") {
                    "jpg"
                } else if data.starts_with(b"GIF8") {
                    "gif"
                } else if data.starts_with(b"BM") {
                    "bmp"
                } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
                    "webp"
                } else {
                    return Err("Обери PNG, JPEG, WebP, GIF або BMP".into());
                };
                let folder = crate::platform::data_dir().join("profile-images");
                std::fs::create_dir_all(&folder).map_err(|_| "Не вдалося зберегти фото")?;
                let destination = folder.join(format!("{}.{}", uuid::Uuid::new_v4(), extension));
                std::fs::write(&destination, &data).map_err(|_| "Не вдалося зберегти фото")?;
                let url =
                    url::Url::from_file_path(destination).map_err(|_| "Недійсний шлях фото")?;
                return Ok(json!({"ok": true, "url": url.as_str()}));
            }
            "navigate" => session.navigate(message["view"].as_str().unwrap_or("home")),
            "playlist" => session.select_playlist(message["id"].as_str().unwrap_or("")),
            "search" => session.search(
                message["query"].as_str().unwrap_or(""),
                message["source"].as_str().unwrap_or("all"),
            ),
            "play" => session.play(
                message["track"].clone(),
                message["queue"].as_array().cloned().unwrap_or_default(),
            ),
            "player" => session.player_action(message["action"].as_str().unwrap_or("toggle")),
            "seek" => session.seek(message["value"].as_f64().unwrap_or(0.0)),
            "volume" => session.set_volume(message["value"].as_f64().unwrap_or(0.8) as f32),
            "volume-commit" => {
                let value = message["value"].as_f64().unwrap_or(0.8).clamp(0.0, 1.0);
                session.set_volume(value as f32);
                session.update_settings(json!({"volume": value}));
            }
            "settings" => session.update_settings(message["patch"].clone()),
            "action" => session.action(
                message["action"].as_str().unwrap_or(""),
                message["payload"].clone(),
            ),
            "call" => session.call(
                message["method"].as_str().unwrap_or(""),
                message["args"].as_array().cloned().unwrap_or_default(),
            ),
            "caption" => {
                crate::desktop_shell::configure_main_window(
                    message["dark"].as_bool().unwrap_or(true),
                );
            }
            _ => return Err("Невідома дія інтерфейсу".into()),
        }
        Ok(json!({"ok": true}))
    })
}

#[no_mangle]
pub extern "C" fn sf_close() {
    if let Ok(mut lock) = runtime().lock() {
        if let Some(rt) = lock.take() {
            rt.session.shutdown();
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn sf_free(pointer: *mut c_char) {
    if !pointer.is_null() {
        drop(CString::from_raw(pointer));
    }
}

#[cfg(test)]
mod state_transfer_tests {
    use super::*;

    fn apply(previous: &Value, response: &Value) -> Value {
        if let Some(state) = response.get("state") {
            return state.clone();
        }
        let mut next = previous.as_object().unwrap().clone();
        for key in response["state_patch"]["remove"].as_array().unwrap() {
            next.remove(key.as_str().unwrap());
        }
        next.extend(response["state_patch"]["set"].as_object().unwrap().clone());
        Value::Object(next)
    }

    #[test]
    fn patches_reconstruct_full_snapshots_with_removal_null_and_account_swap() {
        let mut transfer = StateTransfer::new(&json!({"state_patch_v1": true}));
        let snapshots = [
            json!({"library": {"profile": {"id": "a"}, "tracks": [1, 2]}, "player": {"position": 1}, "temporary": true}),
            json!({"library": {"profile": {"id": "a"}, "tracks": [1, 2]}, "player": {"position": 2}, "temporary": null}),
            json!({"library": {"profile": {"id": "b"}}, "player": null, "added": null}),
            json!({"library": {}, "player": null}),
            json!({"library": {}, "player": null}),
        ];
        let mut reconstructed = json!({});
        for (index, expected) in snapshots.into_iter().enumerate() {
            let events = vec![json!({"type": "show"})];
            let response = transfer.poll(expected.clone(), events.clone());
            assert_eq!(response["events"], json!(events));
            assert_eq!(response.get("state").is_some(), index == 0);
            reconstructed = apply(&reconstructed, &response);
            assert_eq!(reconstructed, expected);
        }
    }

    #[test]
    fn legacy_clients_always_receive_full_snapshots_and_new_sessions_reset() {
        for config in [json!({}), json!({"state_patch_v1": false}), json!({"state_patch_v1": "true"})] {
            let mut transfer = StateTransfer::new(&config);
            for position in 0..3 {
                let state = json!({"player": {"position": position}});
                assert_eq!(transfer.poll(state.clone(), vec![]), json!({"ok": true, "state": state, "events": []}));
            }
        }
        let config = json!({"state_patch_v1": true});
        let mut transfer = StateTransfer::new(&config);
        transfer.poll(json!({"old_account": true}), vec![]);
        transfer = StateTransfer::new(&config);
        assert_eq!(transfer.poll(json!({}), vec![])["state"], json!({}));
    }

    #[test]
    fn large_library_position_only_patch_reduces_serialized_bytes() {
        let tracks: Vec<Value> = (0..5000).map(|id| json!({"id": id, "title": format!("Track {id}"), "artist": "Example artist"})).collect();
        let initial = json!({"library": {"tracks": tracks}, "player": {"position": 10}, "revision": 1});
        let mut next = initial.clone();
        next["player"]["position"] = json!(11);
        next["revision"] = json!(2);
        let mut transfer = StateTransfer::new(&json!({"state_patch_v1": true}));
        transfer.poll(initial.clone(), vec![]);
        let patch = transfer.poll(next.clone(), vec![]);
        let full = json!({"ok": true, "state": next, "events": []});
        let patch_bytes = patch.to_string().len();
        let full_bytes = full.to_string().len();
        println!("state transfer: full={full_bytes} bytes, patch={patch_bytes} bytes");
        assert!(patch_bytes * 100 < full_bytes);
        assert!(patch["state_patch"]["set"].get("library").is_none());
        assert_eq!(apply(&initial, &patch), next);
    }
}
