import webview
import time
import threading
import os

appdata = os.environ.get("APPDATA")
storage_dir = os.path.join(appdata, "SoundFlow", "WebView2Profile")
os.makedirs(storage_dir, exist_ok=True)

class Api:
    def log(self, msg):
        print("[JS LOG]", msg)

api = Api()

def check():
    time.sleep(3)
    w = webview.windows[0]
    res = w.evaluate_js("""
        (function() {
            var canStorage = false;
            var err = "";
            try {
                localStorage.setItem("test", "1");
                canStorage = (localStorage.getItem("test") === "1");
                localStorage.removeItem("test");
            } catch(e) {
                err = e.toString();
            }
            return {
                href: window.location.href,
                origin: window.location.origin,
                protocol: window.location.protocol,
                canStorage: canStorage,
                storageErr: err
            };
        })()
    """)
    print("Origin test result:", res)
    time.sleep(1)
    w.destroy()

threading.Thread(target=check, daemon=True).start()

html_file = os.path.abspath("frontend/index.html")
w = webview.create_window("Origin Test", url=html_file, js_api=api)
webview.start(private_mode=False, storage_path=storage_dir, http_server=True)
