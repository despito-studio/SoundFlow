import webview
import time
import threading
import os

appdata = os.environ.get("APPDATA") or os.path.expanduser("~")
storage_dir = os.path.join(appdata, "SoundFlow", "WebView2Profile")
os.makedirs(storage_dir, exist_ok=True)

from backend.api import AppApi

def test_runner():
    time.sleep(5)
    w = webview.windows[0]
    
    # 1. Click "Увійти"
    w.evaluate_js("""
        (function() {
            var btn = document.getElementById('btn-sign-in');
            if (btn) btn.click();
        })()
    """)
    print("[Clicked Увійти]")
    time.sleep(3)

    # 2. Check DOM for modals
    res = w.evaluate_js("""
        (function() {
            var oldModal = document.getElementById('modal-clerk-auth');
            var clerkModal = document.querySelector('.cl-modalBackdrop, .cl-modal, [class*="cl-"]');
            return JSON.stringify({
                hasOldDuplicateModal: !!oldModal,
                hasNativeClerkModal: !!clerkModal
            });
        })()
    """)
    print("[Modal check]:", res)
    time.sleep(1)
    w.destroy()

threading.Thread(target=test_runner, daemon=True).start()

base_dir = os.path.dirname(os.path.abspath(__file__))
html_file = os.path.join(base_dir, "frontend", "index.html")
api = AppApi()

w = webview.create_window(
    title="SoundFlow Native Modal Test",
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
