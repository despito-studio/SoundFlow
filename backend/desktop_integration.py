import os
import sys
import time
import json
import threading
import queue
import ctypes
from ctypes import wintypes
import winreg
from PIL import Image
import pystray
from pystray import MenuItem as item

# Discord RPC
try:
    from pypresence import Presence
    PYPRESENCE_AVAILABLE = True
except ImportError:
    PYPRESENCE_AVAILABLE = False

# Win32 Constants for Hotkeys
user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32

MOD_ALT = 0x0001
MOD_CONTROL = 0x0002
MOD_SHIFT = 0x0004
MOD_WIN = 0x0008
MOD_NOREPEAT = 0x4000

WM_HOTKEY = 0x0312

VK_LEFT = 0x25
VK_UP = 0x26
VK_RIGHT = 0x27
VK_DOWN = 0x28
VK_SPACE = 0x20
VK_MEDIA_NEXT_TRACK = 0xB0
VK_MEDIA_PREV_TRACK = 0xB1
VK_MEDIA_STOP = 0xB2
VK_MEDIA_PLAY_PAUSE = 0xB3

class GameToastHUD:
    """Toast notifications disabled to avoid intrusive Windows popups."""
    def __init__(self, desktop_mgr=None):
        pass

    def show(self, title, artist):
        pass

class DesktopIntegration:
    """Unified manager for Windows System Tray, Global Hotkeys, Discord Rich Presence, and Game HUD."""
    def __init__(self, api, base_dir):
        self.api = api
        self.base_dir = base_dir
        self.window = None

        self.tray_icon = None
        self.hud = GameToastHUD(self)
        self.rpc = None
        self.rpc_connected = False
        self.last_track = None

        # Settings defaults
        self.hotkeys_enabled = True
        self.tray_enabled = True
        self.discord_enabled = True
        self.hud_enabled = False
        self.volume_norm_enabled = True

        self._hotkey_thread = None
        self._running = True

    def set_window(self, window):
        self.window = window

    def start(self):
        """Initializes all desktop components in non-blocking background threads."""
        self._start_hotkeys()
        self._start_tray()
        self._start_discord_rpc()

    # ==========================================
    # GLOBAL HOTKEYS
    # ==========================================
    def _start_hotkeys(self):
        self._hotkey_thread = threading.Thread(target=self._hotkey_loop, daemon=True)
        self._hotkey_thread.start()

    def _hotkey_loop(self):
        """Win32 Hotkey Message Loop."""
        HOTKEY_NEXT = 101
        HOTKEY_PREV = 102
        HOTKEY_PLAY = 103
        HOTKEY_VOLUP = 104
        HOTKEY_VOLDN = 105
        HOTKEY_M_NEXT = 106
        HOTKEY_M_PREV = 107
        HOTKEY_M_PLAY = 108

        # Register hotkeys
        # Ctrl + Alt + Right / Left / Space / Up / Down
        user32.RegisterHotKey(0, HOTKEY_NEXT, MOD_CONTROL | MOD_ALT, VK_RIGHT)
        user32.RegisterHotKey(0, HOTKEY_PREV, MOD_CONTROL | MOD_ALT, VK_LEFT)
        user32.RegisterHotKey(0, HOTKEY_PLAY, MOD_CONTROL | MOD_ALT, VK_SPACE)
        user32.RegisterHotKey(0, HOTKEY_VOLUP, MOD_CONTROL | MOD_ALT, VK_UP)
        user32.RegisterHotKey(0, HOTKEY_VOLDN, MOD_CONTROL | MOD_ALT, VK_DOWN)

        # Media keys
        user32.RegisterHotKey(0, HOTKEY_M_NEXT, 0, VK_MEDIA_NEXT_TRACK)
        user32.RegisterHotKey(0, HOTKEY_M_PREV, 0, VK_MEDIA_PREV_TRACK)
        user32.RegisterHotKey(0, HOTKEY_M_PLAY, 0, VK_MEDIA_PLAY_PAUSE)

        msg = wintypes.MSG()
        while self._running:
            # Wait up to 300ms for incoming thread messages, then yield
            user32.MsgWaitForMultipleObjectsEx(0, None, 300, 0x04FF, 0)
            while user32.PeekMessageW(ctypes.byref(msg), None, 0, 0, 1):  # PM_REMOVE = 1
                if msg.message == WM_HOTKEY:
                    if not self.hotkeys_enabled:
                        continue
                    hk_id = msg.wParam
                    if hk_id in (HOTKEY_NEXT, HOTKEY_M_NEXT):
                        self.on_hotkey_next()
                    elif hk_id in (HOTKEY_PREV, HOTKEY_M_PREV):
                        self.on_hotkey_prev()
                    elif hk_id in (HOTKEY_PLAY, HOTKEY_M_PLAY):
                        self.on_hotkey_play()
                    elif hk_id == HOTKEY_VOLUP:
                        self.on_hotkey_volume(+0.05)
                    elif hk_id == HOTKEY_VOLDN:
                        self.on_hotkey_volume(-0.05)
                user32.TranslateMessage(ctypes.byref(msg))
                user32.DispatchMessageW(ctypes.byref(msg))

        # Cleanup
        for hid in (HOTKEY_NEXT, HOTKEY_PREV, HOTKEY_PLAY, HOTKEY_VOLUP, HOTKEY_VOLDN, HOTKEY_M_NEXT, HOTKEY_M_PREV, HOTKEY_M_PLAY):
            user32.UnregisterHotKey(0, hid)

    def on_hotkey_next(self):
        if self.window:
            try:
                self.window.run_js("window.soundFlowPlayer?.next();")
            except Exception:
                pass

    def on_hotkey_prev(self):
        if self.window:
            try:
                self.window.run_js("window.soundFlowPlayer?.prev();")
            except Exception:
                pass

    def on_hotkey_play(self):
        if self.window:
            try:
                self.window.run_js("window.soundFlowPlayer?.togglePlay();")
            except Exception:
                pass

    def on_hotkey_volume(self, delta):
        if self.window:
            try:
                self.window.run_js(f"window.soundFlowPlayer?.changeVolumeRelative({delta});")
            except Exception:
                pass

    # ==========================================
    # SYSTEM TRAY (PYSTRAY)
    # ==========================================
    def _start_tray(self):
        tray_thread = threading.Thread(target=self._run_tray, daemon=True)
        tray_thread.start()

    def _run_tray(self):
        icon_path = os.path.join(self.base_dir, "assets", "icon.png")
        if not os.path.exists(icon_path):
            icon_path = os.path.join(self.base_dir, "assets", "icon.ico")
        try:
            image = Image.open(icon_path)
        except Exception:
            image = Image.new('RGB', (64, 64), color='#ff2d75')

        menu = pystray.Menu(
            item('▶️ / ⏸️ Відтворити / Пауза', lambda: self.on_hotkey_play(), default=False),
            item('⏭️ Наступний трек', lambda: self.on_hotkey_next()),
            item('⏮️ Попередній трек', lambda: self.on_hotkey_prev()),
            pystray.Menu.SEPARATOR,
            item('🪟 Відкрити SoundFlow', lambda: self.restore_window(), default=True),
            item('❌ Вийти з програми', lambda: self.quit_app())
        )

        self.tray_icon = pystray.Icon("SoundFlow", image, "SoundFlow Music Player", menu)
        try:
            self.tray_icon.run()
        except Exception as e:
            print("Tray error:", e)

    def restore_window(self):
        if self.window:
            try:
                self.window.show()
                self.window.restore()
            except Exception:
                pass

    def minimize_to_tray(self):
        if self.window:
            try:
                self.window.hide()
            except Exception:
                pass

    def quit_app(self):
        self._running = False
        if self.tray_icon:
            try:
                self.tray_icon.stop()
            except Exception:
                pass
        os._exit(0)

    # ==========================================
    # DISCORD RICH PRESENCE (RPC)
    # ==========================================
    def _start_discord_rpc(self):
        if not PYPRESENCE_AVAILABLE:
            return
        threading.Thread(target=self._init_rpc_worker, daemon=True).start()

    def _init_rpc_worker(self):
        # SoundFlow Discord App Client ID
        client_id = "1231267885564264560"
        try:
            self.rpc = Presence(client_id)
            self.rpc.connect()
            self.rpc_connected = True
            print("Discord RPC connected successfully!")
        except Exception:
            self.rpc_connected = False

    def update_now_playing(self, track, is_playing=True, position=0):
        self.last_track = track

        # Update Discord RPC
        if not self.discord_enabled or not self.rpc_connected or not self.rpc:
            return

        try:
            if not track or not is_playing:
                self.rpc.update(
                    details="SoundFlow Player",
                    state="На паузі ⏸️",
                    large_image="soundflow_logo",
                    large_text="SoundFlow - Музика без меж"
                )
                return

            title = track.get("title") or "Трек"
            artist = track.get("artist") or "SoundFlow"
            duration = int(track.get("duration") or 0)
            now = int(time.time())

            kw = {
                "details": title[:120],
                "state": f"Виконавець: {artist}"[:120],
                "large_image": "soundflow_logo",
                "large_text": "SoundFlow",
                "small_image": "play",
                "small_text": "Відтворюється 🎵",
            }
            if duration > 0 and position >= 0:
                kw["start"] = now - int(position)
                kw["end"] = now + max(0, duration - int(position))
            else:
                kw["start"] = now

            self.rpc.update(**kw)
        except Exception:
            # Reconnect on next change if disconnected
            self.rpc_connected = False

    # ==========================================
    # WINDOWS STARTUP TOGGLE (REGISTRY)
    # ==========================================
    def set_startup(self, enable=True):
        key_path = r"Software\Microsoft\Windows\CurrentVersion\Run"
        app_name = "SoundFlow"
        exe_path = sys.executable if getattr(sys, 'frozen', False) else os.path.abspath(sys.argv[0])

        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key_path, 0, winreg.KEY_SET_VALUE) as key:
                if enable:
                    winreg.SetValueEx(key, app_name, 0, winreg.REG_SZ, f'"{exe_path}"')
                    return True
                else:
                    try:
                        winreg.DeleteValue(key, app_name)
                    except FileNotFoundError:
                        pass
                    return False
        except Exception as e:
            print("Startup registry error:", e)
            return False

    def is_startup_enabled(self):
        key_path = r"Software\Microsoft\Windows\CurrentVersion\Run"
        app_name = "SoundFlow"
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key_path, 0, winreg.KEY_READ) as key:
                val, _ = winreg.QueryValueEx(key, app_name)
                return bool(val)
        except Exception:
            return False
