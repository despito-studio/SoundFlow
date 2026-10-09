import os
import sys
import json
import threading

# Prevent crashes when run without console in PyInstaller
if sys.stdout is None:
    sys.stdout = open(os.devnull, "w", encoding="utf-8")
if sys.stderr is None:
    sys.stderr = open(os.devnull, "w", encoding="utf-8")

import bottle
import webview
import webview.http
from webview.util import abspath, is_app, is_local_url
from backend.api import AppApi
from backend.desktop_integration import DesktopIntegration
from backend.mobile_server import MobileServer

# Chromium & WebView2 flags: ensure uninterrupted media autoplay
os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = "--autoplay-policy=no-user-gesture-required"

def _patch_pywebview_http():
    """
    Fixes pywebview internal bottle server bug where requesting '/'
    (e.g. Clerk OAuth callback /?__clerk_db_jwt=...) throws 500 Internal Server Error
    due to asset() handler missing the default 'file' parameter.
    """
    orig_cls = webview.http.BottleServer

    @classmethod
    def patched_start_server(cls, urls, http_port, keyfile=None, certfile=None):
        from webview import _state
        apps = [u for u in urls if is_app(u)]
        server = cls()

        if len(apps) > 0:
            app = apps[0]
            common_path = '.'
        else:
            local_urls = [u.split('#')[0] for u in urls if is_local_url(u)]
            common_path = os.path.commonpath(local_urls) if len(local_urls) > 0 else None
            if common_path is not None and not os.path.isdir(abspath(common_path)):
                common_path = os.path.dirname(common_path)
            server.root_path = abspath(common_path) if common_path is not None else None
            app = bottle.Bottle()

            @app.post(f'/js_api/{server.uid}')
            def js_api():
                bottle.response.headers['Access-Control-Allow-Origin'] = '*'
                bottle.response.headers['Access-Control-Allow-Methods'] = 'PUT, GET, POST, DELETE, OPTIONS'
                bottle.response.headers['Access-Control-Allow-Headers'] = 'Origin, Accept, Content-Type, X-Requested-With, X-CSRF-Token'
                body = json.loads(bottle.request.body.read().decode('utf-8'))
                if body['uid'] in server.js_callback:
                    return json.dumps(server.js_callback[body['uid']](body))

            @app.route('/')
            @app.route('/<file:path>')
            def asset(file='index.html'):
                if not server.root_path:
                    return ''
                if not file:
                    file = 'index.html'
                bottle.response.set_header('Cache-Control', 'no-cache, no-store, must-revalidate')
                bottle.response.set_header('Pragma', 'no-cache')
                bottle.response.set_header('Expires', 0)
                return bottle.static_file(file, root=server.root_path)

        server.root_path = abspath(common_path) if common_path is not None else None
        server.port = http_port or webview.http._get_random_port()
        server_adapter = webview.http.ThreadedAdapter
        server.thread = threading.Thread(
            target=lambda: bottle.run(app=app, server=server_adapter, port=server.port, quiet=not _state['debug']),
            daemon=True
        )
        server.thread.start()

        server.running = True
        server.address = f'http://127.0.0.1:{server.port}/'
        cls.common_path = common_path
        server.js_api_endpoint = f'{server.address}js_api/{server.uid}'
        return server.address, common_path, server

    orig_cls.start_server = patched_start_server

_patch_pywebview_http()

def ensure_webview2_runtime():
    """Ensure WebView2 runtime path or Edge path is found so it never falls back to MSHTML."""
    try:
        import winreg
        for hive in (winreg.HKEY_LOCAL_MACHINE, winreg.HKEY_CURRENT_USER):
            for subkey in (
                r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
                r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
            ):
                try:
                    with winreg.OpenKey(hive, subkey) as k:
                        pv, _ = winreg.QueryValueEx(k, "pv")
                        if pv and pv != "0":
                            return
                except Exception:
                    pass

        edge_candidates = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application",
            r"C:\Program Files\Microsoft\Edge\Application",
        ]
        for p in edge_candidates:
            if os.path.exists(os.path.join(p, "msedge.exe")):
                webview.settings["WEBVIEW2_RUNTIME_PATH"] = p
                return
    except Exception as e:
        print("ensure_webview2_runtime error:", e)

def run_app():
    ensure_webview2_runtime()
    # Base directory (supports both script, onefile, and onedir frozen exe)
    if getattr(sys, "frozen", False):
        if hasattr(sys, "_MEIPASS"):
            base_dir = sys._MEIPASS
        else:
            exe_dir = os.path.dirname(sys.executable)
            internal_dir = os.path.join(exe_dir, "_internal")
            if os.path.exists(os.path.join(internal_dir, "frontend", "index.html")):
                base_dir = internal_dir
            elif os.path.exists(os.path.join(exe_dir, "frontend", "index.html")):
                base_dir = exe_dir
            else:
                base_dir = exe_dir
    else:
        base_dir = os.path.dirname(os.path.abspath(__file__))
    html_file = os.path.join(base_dir, "frontend", "index.html")

    # Persistent storage directory for WebView2 (preserves Clerk auth session & cookies)
    appdata = os.environ.get("APPDATA") or os.path.expanduser("~")
    storage_dir = os.path.join(appdata, "SoundFlow", "WebView2Profile")
    os.makedirs(storage_dir, exist_ok=True)

    api = AppApi()
    desktop = DesktopIntegration(api, base_dir)
    api.set_desktop(desktop)

    # Start mobile streaming server for iPhone / LAN
    mobile_server = MobileServer(api, base_dir, port=8088)
    mobile_server.start()
    api.set_mobile_server(mobile_server)

    # Window creation
    window = webview.create_window(
        title="SoundFlow",
        url=html_file,
        js_api=api,
        width=1240,
        height=820,
        min_size=(940, 640),
        background_color="#0e1117",
        text_select=False,
    )

    desktop.set_window(window)

    def on_loaded():
        desktop.start()

    window.events.loaded += on_loaded

    def on_closing():
        if desktop and desktop.tray_enabled:
            desktop.minimize_to_tray()
            return False
        return True

    window.events.closing += on_closing

    # Start pywebview with internal lightweight HTTP server & persistent profile
    webview.start(
        private_mode=False,
        storage_path=storage_dir,
        http_server=True,
        debug=False
    )

if __name__ == "__main__":
    run_app()
