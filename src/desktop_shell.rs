use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::Sender,
    Arc,
};

#[derive(Clone, Debug)]
pub enum ShellEvent {
    ShowMain,
    TogglePlayback,
    Next,
    Previous,
    ToggleIsland { keyboard: bool },
    Quit,
    Error(String),
}

/// Route activation through the existing instance's UI event loop, including hidden windows.
pub(crate) fn activate_existing(pid: u32) -> bool {
    #[cfg(windows)]
    unsafe {
        windows::activate_existing(pid)
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IslandBackdrop {
    Unavailable,
    Blur,
    Acrylic,
}

/// Apply the main window's native caption theme after creation or a theme change.
pub fn configure_main_window(dark: bool) -> bool {
    #[cfg(windows)]
    unsafe {
        windows::configure_main_window(dark)
    }
    #[cfg(not(windows))]
    {
        let _ = dark;
        false
    }
}

pub fn island_backdrop() -> IslandBackdrop {
    #[cfg(windows)]
    {
        windows::backdrop()
    }
    #[cfg(not(windows))]
    {
        IslandBackdrop::Unavailable
    }
}

/// Refresh after changing the Island viewport size; this never changes focus.
pub fn refresh_island_shape() -> bool {
    #[cfg(windows)]
    unsafe {
        let window = windows::island_window();
        !window.is_null() && windows::refresh_shape(window, false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[derive(Clone)]
struct Events {
    sender: Sender<ShellEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Events {
    fn send(&self, event: ShellEvent) {
        if self.sender.send(event).is_ok() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.wake)()));
        }
    }
}

pub struct DesktopShell {
    pub hotkeys_enabled: Arc<AtomicBool>,
    tray_available: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    window: Arc<AtomicUsize>,
    thread: Option<std::thread::JoinHandle<()>>,
    _media_keys: crate::media_keys::MediaKeys,
}

impl DesktopShell {
    pub fn start(sender: Sender<ShellEvent>, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self::start_with_island_hotkeys(sender, wake, true)
    }

    pub fn start_with_island_hotkeys(
        sender: Sender<ShellEvent>,
        wake: Arc<dyn Fn() + Send + Sync>,
        island_hotkeys: bool,
    ) -> Self {
        let events = Events { sender, wake };
        let hotkeys_enabled = Arc::new(AtomicBool::new(!verification_mode()));
        let media_events = events.clone();
        let media_keys = crate::media_keys::start(hotkeys_enabled.clone(), move |action| {
            let event = match action {
                "toggle" => ShellEvent::TogglePlayback,
                "next" => ShellEvent::Next,
                "previous" => ShellEvent::Previous,
                "registration-error" => ShellEvent::Error(
                    "Деякі медіаклавіші зайняті іншою програмою. Керування у SoundFlow доступне."
                        .into(),
                ),
                _ => return,
            };
            media_events.send(event);
        });
        let tray_available = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let window = Arc::new(AtomicUsize::new(0));
        #[cfg(windows)]
        let thread = {
            let finish = stop.clone();
            let available = tray_available.clone();
            let handle = window.clone();
            Some(std::thread::spawn(move || {
                windows::run(events, finish, available, handle, island_hotkeys);
            }))
        };
        #[cfg(not(windows))]
        let thread = {
            events.send(ShellEvent::Error(
                "Системний трей і глобальний виклик Island у цій збірці недоступні. Керування у вікні працює."
                    .into(),
            ));
            None
        };
        Self {
            hotkeys_enabled,
            tray_available,
            stop,
            window,
            thread,
            _media_keys: media_keys,
        }
    }

    pub fn tray_available(&self) -> bool {
        self.tray_available.load(Ordering::Acquire)
    }
}

fn verification_mode() -> bool {
    std::env::args().any(|argument| argument == "--verify-native")
}

impl Drop for DesktopShell {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::{
                Foundation::HWND,
                UI::WindowsAndMessaging::{PostMessageW, WM_CANCELMODE, WM_CLOSE},
            };
            let window = self.window.load(Ordering::Acquire) as HWND;
            if !window.is_null() {
                PostMessageW(window, WM_CANCELMODE, 0, 0);
                PostMessageW(window, WM_CLOSE, 0, 0);
            }
        }
        if let Some(thread) = self.thread.take() {
            if thread.thread().id() != std::thread::current().id() {
                let _ = thread.join();
            }
        }
    }
}

pub fn remember_foreground() -> usize {
    #[cfg(windows)]
    unsafe {
        let foreground = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if foreground == windows::island_window() {
            0
        } else {
            foreground as usize
        }
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Call after creating/showing the Island, and once per input-mode transition.
/// A false result means the native window is not present yet; the caller can retry.
pub fn configure_island(keyboard: bool) -> bool {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let window = windows::island_window();
        if window.is_null() {
            return false;
        }
        let mut style = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
        style = (style | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW;
        if keyboard {
            style &= !WS_EX_NOACTIVATE;
        } else {
            style |= WS_EX_NOACTIVATE;
        }
        SetWindowLongPtrW(window, GWL_EXSTYLE, style as isize);
        windows::refresh_shape(window, true);
        windows::apply_backdrop(window);
        let changed = SetWindowPos(
            window,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        ) != 0;
        if keyboard {
            SetForegroundWindow(window);
        }
        changed
    }
    #[cfg(not(windows))]
    {
        let _ = keyboard;
        false
    }
}

pub fn restore_foreground(previous: usize) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{
                GetForegroundWindow, IsWindow, IsWindowVisible, SetForegroundWindow,
            },
        };
        let previous = previous as HWND;
        let island = windows::island_window();
        if !previous.is_null()
            && !island.is_null()
            && GetForegroundWindow() == island
            && IsWindow(previous) != 0
            && IsWindowVisible(previous) != 0
        {
            SetForegroundWindow(previous);
        }
    }
    #[cfg(not(windows))]
    let _ = previous;
}

#[cfg(windows)]
mod windows {
    use super::{Events, IslandBackdrop, ShellEvent};
    use std::{
        ffi::c_void,
        mem::size_of,
        ptr::{null, null_mut},
        sync::{
            atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
            Arc, Mutex, OnceLock,
        },
        time::Duration,
    };
    use windows_sys::Win32::{
        Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn},
        System::{
            LibraryLoader::{
                GetModuleHandleW, GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
            },
            Threading::GetCurrentProcessId,
        },
        UI::{
            Input::KeyboardAndMouse::{
                RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
            },
            Shell::{
                Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD,
                NIM_DELETE, NIM_SETVERSION, NINF_KEY, NIN_SELECT, NOTIFYICONDATAW,
                NOTIFYICON_VERSION_4,
            },
            WindowsAndMessaging::*,
        },
    };

    const TRAY_MESSAGE: u32 = WM_APP + 41;
    const ACTIVATE_MESSAGE: u32 = WM_APP + 42;
    const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
    const QUICK_HOTKEY: i32 = 3101;
    const KEYBOARD_HOTKEY: i32 = 3102;
    const SHOW_MAIN: usize = 1;
    const PLAY_PAUSE: usize = 2;
    const NEXT: usize = 3;
    const SHOW_ISLAND: usize = 4;
    const QUIT: usize = 5;

    static BACKDROP: AtomicU8 = AtomicU8::new(0);
    static SHAPE: Mutex<(usize, i32, i32, u32)> = Mutex::new((0, 0, 0, 0));

    pub(super) unsafe fn activate_existing(pid: u32) -> bool {
        struct Search {
            pid: u32,
            window: HWND,
        }
        unsafe extern "system" fn visit(window: HWND, parameter: LPARAM) -> i32 {
            let search = &mut *(parameter as *mut Search);
            let mut process = 0;
            GetWindowThreadProcessId(window, &mut process);
            if process != search.pid {
                return 1;
            }
            let mut class = [0u16; 64];
            let length = GetClassNameW(window, class.as_mut_ptr(), class.len() as i32);
            if length > 0
                && class[..length as usize]
                    .iter()
                    .copied()
                    .eq("SoundFlow.NativeShell".encode_utf16())
            {
                search.window = window;
                return 0;
            }
            1
        }
        if pid == 0 {
            return false;
        }
        let mut search = Search {
            pid,
            window: null_mut(),
        };
        EnumWindows(Some(visit), (&mut search as *mut Search) as LPARAM);
        if search.window.is_null() {
            return false;
        }
        if pid != GetCurrentProcessId() {
            AllowSetForegroundWindow(pid);
        }
        PostMessageW(search.window, ACTIVATE_MESSAGE, 0, 0) != 0
    }

    pub(super) unsafe fn configure_main_window(dark: bool) -> bool {
        type SetAttribute = unsafe extern "system" fn(HWND, u32, *const c_void, u32) -> i32;
        static FUNCTION: OnceLock<Option<SetAttribute>> = OnceLock::new();
        let window = own_window("SoundFlow");
        if window.is_null() {
            return false;
        }
        let function = FUNCTION.get_or_init(|| {
            let name = wide("dwmapi.dll");
            let mut module = GetModuleHandleW(name.as_ptr());
            if module.is_null() {
                module = LoadLibraryExW(name.as_ptr(), null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32);
            }
            if module.is_null() {
                return None;
            }
            // Keep the module loaded for the lifetime of this cached function pointer.
            GetProcAddress(module, b"DwmSetWindowAttribute\0".as_ptr()).map(|f| {
                std::mem::transmute::<unsafe extern "system" fn() -> isize, SetAttribute>(f)
            })
        });
        let Some(set_attribute) = function else {
            return false;
        };
        let enabled = i32::from(dark);
        let value = (&enabled as *const i32).cast();
        let length = size_of::<i32>() as u32;
        set_attribute(window, 20, value, length) >= 0
            || set_attribute(window, 19, value, length) >= 0
    }

    pub(super) fn backdrop() -> IslandBackdrop {
        match BACKDROP.load(Ordering::Acquire) {
            1 => IslandBackdrop::Blur,
            2 => IslandBackdrop::Acrylic,
            _ => IslandBackdrop::Unavailable,
        }
    }

    unsafe fn window_dpi(window: HWND) -> u32 {
        type GetDpi = unsafe extern "system" fn(HWND) -> u32;
        static FUNCTION: OnceLock<Option<GetDpi>> = OnceLock::new();
        let get_dpi = FUNCTION.get_or_init(|| {
            let module = GetModuleHandleW(wide("user32.dll").as_ptr());
            if module.is_null() {
                return None;
            }
            GetProcAddress(module, b"GetDpiForWindow\0".as_ptr())
                .map(|f| std::mem::transmute::<unsafe extern "system" fn() -> isize, GetDpi>(f))
        });
        get_dpi
            .map(|f| f(window))
            .filter(|dpi| *dpi > 0)
            .unwrap_or(96)
    }

    pub(super) unsafe fn refresh_shape(window: HWND, force: bool) -> bool {
        let mut bounds = RECT::default();
        if GetWindowRect(window, &mut bounds) == 0 {
            return false;
        }
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        if width <= 0 || height <= 0 {
            return false;
        }
        let dpi = window_dpi(window);
        let geometry = (window as usize, width, height, dpi);
        if !force && *SHAPE.lock().unwrap_or_else(|e| e.into_inner()) == geometry {
            return true;
        }
        let diameter = ((64.0 * dpi as f64 / 96.0).round() as i32).clamp(2, width.min(height));
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
        if region.is_null() {
            return false;
        }
        // SetWindowRgn transfers region ownership only after success.
        if SetWindowRgn(window, region, 1) == 0 {
            DeleteObject(region);
            return false;
        }
        *SHAPE.lock().unwrap_or_else(|e| e.into_inner()) = geometry;
        true
    }

    #[repr(C)]
    struct AccentPolicy {
        state: u32,
        flags: u32,
        color: u32,
        animation: u32,
    }
    #[repr(C)]
    struct CompositionData {
        attribute: u32,
        data: *mut c_void,
        length: usize,
    }

    pub(super) unsafe fn apply_backdrop(window: HWND) {
        // WCA_ACCENT_POLICY uses AABBGGRR; bind dynamically for older Windows compositors.
        type SetComposition = unsafe extern "system" fn(HWND, *mut CompositionData) -> i32;
        static FUNCTION: OnceLock<Option<SetComposition>> = OnceLock::new();
        let function = FUNCTION.get_or_init(|| {
            let module = GetModuleHandleW(wide("user32.dll").as_ptr());
            if module.is_null() {
                return None;
            }
            GetProcAddress(module, b"SetWindowCompositionAttribute\0".as_ptr()).map(|f| {
                std::mem::transmute::<unsafe extern "system" fn() -> isize, SetComposition>(f)
            })
        });
        let Some(set_composition) = function else {
            BACKDROP.store(0, Ordering::Release);
            return;
        };
        let mut policy = AccentPolicy {
            state: 4,
            flags: 0,
            color: 0xA8141414,
            animation: 0,
        };
        let mut data = CompositionData {
            attribute: 19,
            data: (&mut policy as *mut AccentPolicy).cast(),
            length: size_of::<AccentPolicy>(),
        };
        if set_composition(window, &mut data) != 0 {
            BACKDROP.store(2, Ordering::Release);
            return;
        }
        let mut blur = AccentPolicy {
            state: 3,
            flags: 2,
            color: 0xA8141414,
            animation: 0,
        };
        data.data = (&mut blur as *mut AccentPolicy).cast();
        let result = set_composition(window, &mut data) != 0;
        BACKDROP.store(if result { 1 } else { 0 }, Ordering::Release);
    }

    struct Context {
        events: Events,
        stop: Arc<AtomicBool>,
        available: Arc<AtomicBool>,
        icon: HICON,
        taskbar_created: u32,
        verification: bool,
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }

    unsafe fn add_tray(window: HWND, context: &Context) {
        if context.verification {
            return;
        }
        let mut data = NOTIFYICONDATAW::default();
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = window;
        data.uID = 1;
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = TRAY_MESSAGE;
        data.hIcon = context.icon;
        for (target, ch) in data.szTip.iter_mut().zip("SoundFlow".encode_utf16()) {
            *target = ch;
        }
        let available = Shell_NotifyIconW(NIM_ADD, &data) != 0;
        context.available.store(available, Ordering::Release);
        if available {
            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &data);
        } else {
            context.events.send(ShellEvent::Error(
                "Не вдалося додати SoundFlow до системного трею. Вікно залишається доступним."
                    .into(),
            ));
        }
    }

    unsafe fn popup(window: HWND, context: &Context) {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        for (id, label) in [
            (SHOW_MAIN, "Відкрити SoundFlow"),
            (PLAY_PAUSE, "Відтворити / призупинити"),
            (NEXT, "Наступна композиція"),
            (SHOW_ISLAND, "Показати / приховати Island"),
        ] {
            let label = wide(label);
            AppendMenuW(menu, MF_STRING, id, label.as_ptr());
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        AppendMenuW(menu, MF_STRING, QUIT, wide("Вийти").as_ptr());
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        let previous = GetForegroundWindow();
        SetForegroundWindow(window);
        let command = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
            point.x,
            point.y,
            0,
            window,
            null(),
        ) as usize;
        DestroyMenu(menu);
        PostMessageW(window, WM_NULL, 0, 0);
        if GetForegroundWindow() == window && !previous.is_null() && IsWindow(previous) != 0 {
            SetForegroundWindow(previous);
        }
        let event = match command {
            SHOW_MAIN => Some(ShellEvent::ShowMain),
            PLAY_PAUSE => Some(ShellEvent::TogglePlayback),
            NEXT => Some(ShellEvent::Next),
            SHOW_ISLAND => Some(ShellEvent::ToggleIsland { keyboard: false }),
            QUIT => Some(ShellEvent::Quit),
            _ => None,
        };
        if let Some(event) = event {
            context.events.send(event);
        }
    }

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *const Context;
        if pointer.is_null() {
            return DefWindowProcW(window, message, wparam, lparam);
        }
        let context = &*pointer;
        if context.taskbar_created != 0 && message == context.taskbar_created {
            add_tray(window, context);
            return 0;
        }
        match message {
            ACTIVATE_MESSAGE => {
                context.events.send(ShellEvent::ShowMain);
                0
            }
            TRAY_MESSAGE => {
                match (lparam as u32 & 0xffff) as u32 {
                    WM_CONTEXTMENU | WM_RBUTTONUP => popup(window, context),
                    NIN_SELECT | NIN_KEYSELECT => context.events.send(ShellEvent::ShowMain),
                    _ => {}
                }
                0
            }
            WM_HOTKEY => {
                match wparam as i32 {
                    QUICK_HOTKEY => context
                        .events
                        .send(ShellEvent::ToggleIsland { keyboard: false }),
                    KEYBOARD_HOTKEY => context
                        .events
                        .send(ShellEvent::ToggleIsland { keyboard: true }),
                    _ => {}
                }
                0
            }
            WM_CANCELMODE => {
                EndMenu();
                0
            }
            WM_CLOSE => {
                context.stop.store(true, Ordering::Release);
                EndMenu();
                0
            }
            WM_DESTROY => {
                context.available.store(false, Ordering::Release);
                context.stop.store(true, Ordering::Release);
                0
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                DefWindowProcW(window, message, wparam, lparam)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }

    pub(super) fn run(
        events: Events,
        stop: Arc<AtomicBool>,
        available: Arc<AtomicBool>,
        handle: Arc<AtomicUsize>,
        island_hotkeys: bool,
    ) {
        unsafe {
            let verification = super::verification_mode();
            let instance = GetModuleHandleW(null());
            let class_name = wide("SoundFlow.NativeShell");
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class_name.as_ptr(),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                events.send(ShellEvent::Error(format!(
                    "Не вдалося створити системне керування SoundFlow ({})",
                    GetLastError()
                )));
                return;
            }
            let mut icon = LoadIconW(instance, 1usize as *const u16);
            if icon.is_null() {
                icon = LoadIconW(null_mut(), IDI_APPLICATION);
            }
            let mut context = Box::new(Context {
                events,
                stop: stop.clone(),
                available: available.clone(),
                icon,
                taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                verification,
            });
            let window = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                wide("SoundFlow system controls").as_ptr(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                instance,
                (&mut *context as *mut Context).cast(),
            );
            if window.is_null() {
                context.events.send(ShellEvent::Error(format!(
                    "Не вдалося відкрити системне керування SoundFlow ({})",
                    GetLastError()
                )));
                UnregisterClassW(class_name.as_ptr(), instance);
                return;
            }
            handle.store(window as usize, Ordering::Release);
            add_tray(window, &context);
            let hotkeys = [
                (
                    QUICK_HOTKEY,
                    MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                    "Ctrl+Alt+M",
                ),
                (
                    KEYBOARD_HOTKEY,
                    MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT,
                    "Ctrl+Alt+Shift+M",
                ),
            ];
            let mut registered = [false; 2];
            if !verification && island_hotkeys {
                for (index, (id, modifiers, name)) in hotkeys.iter().enumerate() {
                    registered[index] =
                        RegisterHotKey(window, *id, *modifiers, u32::from(b'M')) != 0;
                    if !registered[index] {
                        context.events.send(ShellEvent::Error(format!(
                            "Комбінація {name} недоступна. Island можна відкрити у вікні або треї."
                        )));
                    }
                }
            }
            while !stop.load(Ordering::Acquire) {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if message.message == WM_QUIT {
                        stop.store(true, Ordering::Release);
                        break;
                    }
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            handle.store(0, Ordering::Release);
            for (index, (id, _, _)) in hotkeys.iter().enumerate() {
                if registered[index] {
                    UnregisterHotKey(window, *id);
                }
            }
            let data = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: window,
                uID: 1,
                ..Default::default()
            };
            if !verification {
                Shell_NotifyIconW(NIM_DELETE, &data);
            }
            available.store(false, Ordering::Release);
            if IsWindow(window) != 0 {
                DestroyWindow(window);
            }
            UnregisterClassW(class_name.as_ptr(), instance);
        }
    }

    pub(super) unsafe fn island_window() -> HWND {
        own_window("SoundFlow Island")
    }

    unsafe fn own_window(name: &str) -> HWND {
        struct Find {
            window: HWND,
            title: Vec<u16>,
        }
        unsafe extern "system" fn visit(window: HWND, parameter: LPARAM) -> i32 {
            let found = &mut *(parameter as *mut Find);
            let mut process = 0;
            GetWindowThreadProcessId(window, &mut process);
            if process == GetCurrentProcessId() {
                let mut title = [0u16; 128];
                let length = GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32);
                if length > 0 && title[..length as usize] == found.title {
                    found.window = window;
                    return 0;
                }
            }
            1
        }
        let mut found = Find {
            window: null_mut(),
            title: name.encode_utf16().collect(),
        };
        EnumWindows(Some(visit), (&mut found as *mut Find) as LPARAM);
        found.window
    }
}
