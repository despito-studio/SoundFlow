//! Windows application identity and a single native application instance.
use std::{fs, path::PathBuf};

pub fn data_dir() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    if let Some(folder) = args
        .iter()
        .position(|arg| arg == "--data-dir")
        .and_then(|index| args.get(index + 1))
        .filter(|folder| !folder.is_empty())
    {
        return PathBuf::from(folder);
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("app.soundflow.desktop")
}

pub struct InstanceGuard {
    primary: bool,
    pid_file: PathBuf,
    #[cfg(windows)]
    handle: usize,
    #[cfg(unix)]
    sock_path: PathBuf,
    #[cfg(unix)]
    listener: std::sync::Mutex<Option<std::os::unix::net::UnixListener>>,
}

impl InstanceGuard {
    pub fn is_primary(&self) -> bool {
        self.primary
    }

    pub fn listen_activation(&self, on_activate: impl Fn() + Send + 'static) {
        #[cfg(unix)]
        {
            if let Ok(mut lock) = self.listener.lock() {
                if let Some(listener) = lock.take() {
                    std::thread::Builder::new()
                        .name("soundflow-instance-activation".into())
                        .spawn(move || {
                            use std::io::Read;
                            while let Ok((mut stream, _)) = listener.accept() {
                                let mut buffer = [0u8; 32];
                                if let Ok(n) = stream.read(&mut buffer) {
                                    if buffer[..n].starts_with(b"activate") {
                                        on_activate();
                                    }
                                }
                            }
                        })
                        .ok();
                }
            }
        }
        #[cfg(not(unix))]
        let _ = on_activate;
    }
}

pub fn initialize_process() -> Result<InstanceGuard, String> {
    let folder = data_dir();
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let pid_file = folder.join("native-instance.pid");
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{GetLastError, ERROR_ALREADY_EXISTS},
            System::Threading::CreateMutexW,
            UI::Shell::SetCurrentProcessExplicitAppUserModelID,
        };
        let identity: Vec<u16> = "app.soundflow.native\0".encode_utf16().collect();
        let mutex_name = if std::env::args().any(|arg| arg == "--data-dir") {
            use sha2::{Digest, Sha256};
            let canonical = folder.canonicalize().map_err(|error| error.to_string())?;
            format!(
                "Local\\SoundFlow.Native.3.{:x}\0",
                Sha256::digest(canonical.to_string_lossy().to_lowercase().as_bytes())
            )
        } else {
            "Local\\SoundFlow.Native.3\0".to_string()
        };
        let name: Vec<u16> = mutex_name.encode_utf16().collect();
        unsafe {
            if SetCurrentProcessExplicitAppUserModelID(identity.as_ptr()) < 0 {
                return Err("Не вдалося встановити системну ідентичність SoundFlow".into());
            }
            let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            if handle.is_null() {
                return Err("Не вдалося відкрити SoundFlow".into());
            }
            let primary = GetLastError() != ERROR_ALREADY_EXISTS;
            if primary {
                fs::write(&pid_file, std::process::id().to_string()).map_err(|e| {
                    windows_sys::Win32::Foundation::CloseHandle(handle);
                    e.to_string()
                })?;
            } else if let Some(pid) = fs::read_to_string(&pid_file)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
            {
                raise_window(pid);
            }
            return Ok(InstanceGuard {
                primary,
                pid_file,
                handle: handle as usize,
            });
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::net::{UnixListener, UnixStream};
        let sock_path = if std::env::args().any(|arg| arg == "--data-dir") {
            use sha2::{Digest, Sha256};
            let canonical = folder.canonicalize().unwrap_or_else(|_| folder.clone());
            folder.join(format!(
                "native-instance-{:x}.sock",
                Sha256::digest(canonical.to_string_lossy().as_bytes())
            ))
        } else {
            folder.join("native-instance.sock")
        };

        let (primary, listener) = match UnixListener::bind(&sock_path) {
            Ok(listener) => (true, Some(listener)),
            Err(_) => {
                if let Ok(mut stream) = UnixStream::connect(&sock_path) {
                    use std::io::Write;
                    let _ = stream.write_all(b"activate\n");
                    (false, None)
                } else {
                    let _ = fs::remove_file(&sock_path);
                    match UnixListener::bind(&sock_path) {
                        Ok(listener) => (true, Some(listener)),
                        Err(e) => return Err(format!("Не вдалося відкрити SoundFlow: {e}")),
                    }
                }
            }
        };

        if primary {
            let _ = fs::write(&pid_file, std::process::id().to_string());
        }

        Ok(InstanceGuard {
            primary,
            pid_file,
            sock_path,
            listener: std::sync::Mutex::new(listener),
        })
    }
}

#[cfg(windows)]
unsafe fn raise_window(pid: u32) {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, GetWindowThreadProcessId, SetForegroundWindow, ShowWindow,
            SW_RESTORE,
        },
    };
    struct Search {
        pid: u32,
        window: HWND,
    }
    unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> i32 {
        let search = &mut *(data as *mut Search);
        let mut process = 0;
        GetWindowThreadProcessId(window, &mut process);
        let mut title = [0u16; 128];
        let length = GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32);
        if process == search.pid
            && length > 0
            && String::from_utf16_lossy(&title[..length as usize]) == "SoundFlow"
        {
            search.window = window;
            0
        } else {
            1
        }
    }
    let mut search = Search {
        pid,
        window: std::ptr::null_mut(),
    };
    EnumWindows(Some(visit), &mut search as *mut Search as isize);
    if !search.window.is_null() {
        ShowWindow(search.window, SW_RESTORE);
        SetForegroundWindow(search.window);
    }
}

/// Best-effort повернення вікна SoundFlow на передній план — використовується
/// після успішного входу через браузер, щоб одразу перекинути користувача в застосунок.
pub fn focus_main_window() {
    #[cfg(windows)]
    unsafe {
        raise_window(std::process::id());
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if self.primary {
            if fs::read_to_string(&self.pid_file)
                .ok()
                .is_some_and(|pid| pid.trim() == std::process::id().to_string())
            {
                let _ = fs::remove_file(&self.pid_file);
            }
            #[cfg(unix)]
            {
                let _ = fs::remove_file(&self.sock_path);
            }
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle as _);
        }
    }
}
