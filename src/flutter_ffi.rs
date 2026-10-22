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
        if lock.is_some() {
            return Ok(json!({"ok": true}));
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
        });
        Ok(json!({"ok": true}))
    })
}

#[no_mangle]
pub extern "C" fn sf_poll() -> *mut c_char {
    response(|| {
        let lock = runtime().lock().map_err(|e| e.to_string())?;
        let rt = lock.as_ref().ok_or("Аудіоядро ще не запущене")?;
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
        rt.shell.hotkeys_enabled.store(
            state["settings"]["hotkeys_enabled"]
                .as_bool()
                .unwrap_or(true),
            Ordering::Relaxed,
        );
        Ok(json!({"ok": true, "state": state, "events": events}))
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
