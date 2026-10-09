import webview
import time
import threading
import os
import sys

appdata = os.environ.get("APPDATA") or os.path.expanduser("~")
storage_dir = os.path.join(appdata, "SoundFlow", "WebView2Profile")
os.makedirs(storage_dir, exist_ok=True)

from backend.api import AppApi

def test_runner():
    time.sleep(5)
    w = webview.windows[0]
    
    # 1. Click "Увійти" button
    click_res = w.evaluate_js("""
        (function() {
            var btn = document.getElementById('btn-sign-in');
            if (btn) {
                btn.click();
                return "Clicked btn-sign-in";
            }
            return "No btn-sign-in found";
        })()
    """)
    print("[TEST 1] Button click:", click_res)

    time.sleep(4)

    # 2. Check modal and mounted content
    check = w.evaluate_js("""
        (function() {
            var modal = document.getElementById('modal-clerk-auth');
            var mount = document.getElementById('clerk-auth-mount');
            return JSON.stringify({
                modalDisplay: modal ? window.getComputedStyle(modal).display : 'null',
                htmlLen: mount ? mount.innerHTML.length : 0,
                hasOAuthButtons: !!(mount && mount.querySelector('button')),
                hasInputs: !!(mount && mount.querySelector('input')),
                hasRootBox: !!(mount && mount.querySelector('.cl-rootBox'))
            });
        })()
    """)
    print("[TEST 2] Verification result:", check)

    time.sleep(1)
    w.destroy()

threading.Thread(target=test_runner, daemon=True).start()

base_dir = os.path.dirname(os.path.abspath(__file__))
html_file = os.path.join(base_dir, "frontend", "index.html")
api = AppApi()

w = webview.create_window(
    title="SoundFlow Verification Final",
    url=html_file,
    js_api=api,
    width=1100,
    height=750,
)
webview.start(
    private_mode=False,
    storage_path=storage_dir,
    http_server=True,
    debug=False
)
