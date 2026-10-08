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
    def err(self, msg):
        print("[JS ERR]", msg)

api = Api()

def check():
    time.sleep(5)
    w = webview.windows[0]
    res = w.evaluate_js("""
        (function() {
            var clerkStatus = "none";
            if (window.soundFlowAuth) {
                clerkStatus = window.soundFlowAuth.clerk ? "loaded" : "no clerk";
            }
            return JSON.stringify({
                clerkGlobal: !!window.Clerk,
                authObj: !!window.soundFlowAuth,
                authClerk: clerkStatus,
                clerkVersion: window.Clerk ? window.Clerk.version : null
            });
        })()
    """)
    print("Auth check result JSON:", res)
    
    # Try calling openSignIn
    open_res = w.evaluate_js("""
        (function() {
            try {
                if (window.soundFlowAuth) {
                    window.soundFlowAuth.openSignIn();
                    return "openSignIn called successfully";
                }
                return "no soundFlowAuth";
            } catch(e) {
                return "error: " + e.toString();
            }
        })()
    """)
    print("Open sign in result:", open_res)
    time.sleep(3)
    w.destroy()

threading.Thread(target=check, daemon=True).start()

html_file = os.path.abspath("frontend/index.html")
w = webview.create_window("Auth Test", url=html_file, js_api=api)
webview.start(private_mode=False, storage_path=storage_dir, http_server=True)
