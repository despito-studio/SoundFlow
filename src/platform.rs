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
}

impl InstanceGuard {
    pub fn is_primary(&self) -> bool {
        self.primary
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
    #[cfg(not(windows))]
    {
        // Windows is the packaged platform; other platforms get their own
        // native process activation adapter when they are introduced.
        Ok(InstanceGuard {
            primary: true,
            pid_file,
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

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if self.primary
            && fs::read_to_string(&self.pid_file)
                .ok()
                .is_some_and(|pid| pid.trim() == std::process::id().to_string())
        {
            let _ = fs::remove_file(&self.pid_file);
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle as _);
        }
    }
}
