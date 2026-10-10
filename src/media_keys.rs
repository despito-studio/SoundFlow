//! Windows media keys are registered on their own message thread.
//! Cached preferences keep system event handling independent of network requests.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub(crate) struct MediaKeys {
    stop: Arc<AtomicBool>,
}
impl Drop for MediaKeys {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
pub(crate) fn start(
    enabled: Arc<AtomicBool>,
    handler: impl Fn(&'static str) + Send + 'static,
) -> MediaKeys {
    let stop = Arc::new(AtomicBool::new(false));
    #[cfg(windows)]
    {
        let finish = stop.clone();
        std::thread::spawn(move || {
            use windows_sys::Win32::UI::{
                Input::KeyboardAndMouse::{
                    RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT, VK_MEDIA_NEXT_TRACK,
                    VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
                },
                WindowsAndMessaging::{PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY},
            };
            let keys = [
                (1, VK_MEDIA_PLAY_PAUSE, "toggle"),
                (2, VK_MEDIA_NEXT_TRACK, "next"),
                (3, VK_MEDIA_PREV_TRACK, "previous"),
            ];
            let mut registered = [false; 3];
            let mut configured = None;
            while !finish.load(Ordering::Acquire) {
                let wanted = enabled.load(Ordering::Relaxed);
                if configured != Some(wanted) {
                    for (index, (id, key, _)) in keys.iter().enumerate() {
                        unsafe {
                            if registered[index] {
                                UnregisterHotKey(std::ptr::null_mut(), *id);
                                registered[index] = false;
                            }
                            if wanted {
                                registered[index] = RegisterHotKey(
                                    std::ptr::null_mut(),
                                    *id,
                                    MOD_NOREPEAT,
                                    u32::from(*key),
                                ) != 0;
                            }
                        }
                    }
                    if wanted && registered.iter().any(|value| !*value) {
                        handler("registration-error");
                    }
                    configured = Some(wanted);
                }
                let mut message: MSG = unsafe { std::mem::zeroed() };
                while unsafe {
                    PeekMessageW(
                        &mut message,
                        std::ptr::null_mut(),
                        WM_HOTKEY,
                        WM_HOTKEY,
                        PM_REMOVE,
                    )
                } != 0
                {
                    if wanted {
                        if let Some((_, _, action)) = keys
                            .iter()
                            .find(|(id, _, _)| *id as usize == message.wParam)
                        {
                            handler(action);
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(80));
            }
            for (index, (id, _, _)) in keys.iter().enumerate() {
                if registered[index] {
                    unsafe {
                        UnregisterHotKey(std::ptr::null_mut(), *id);
                    }
                }
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (enabled, handler);
    MediaKeys { stop }
}
