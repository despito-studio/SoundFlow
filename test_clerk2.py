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
        (async function() {
            try {
                console.log("Checking Clerk...");
                if (!window.Clerk) return { error: "No Clerk object" };
                console.log("Clerk loaded:", window.Clerk.version || "ok");
                await window.Clerk.load();
                console.log("Clerk.load() completed");
                var mount = document.getElementById('clerk-mount');
                window.Clerk.mountSignIn(mount);
                console.log("mountSignIn called");
                return { success: true };
            } catch (e) {
                console.error("Catch in check:", e.toString(), e.stack);
                return { error: e.toString() };
            }
        })()
    """)
    print("Evaluate result:", res)
    time.sleep(5)
    w.destroy()

threading.Thread(target=check, daemon=True).start()

html = """
<!DOCTYPE html>
<html>
<head>
    <script>
        window.addEventListener('error', function(e) {
            if (window.pywebview && window.pywebview.api) {
                window.pywebview.api.err("Window Error: " + e.message + " at " + e.filename + ":" + e.lineno);
            }
        });
        const origLog = console.log;
        console.log = function(...args) {
            origLog.apply(console, args);
            if (window.pywebview && window.pywebview.api) {
                window.pywebview.api.log(args.map(a => typeof a === 'object' ? JSON.stringify(a) : String(a)).join(' '));
            }
        };
        const origErr = console.error;
        console.error = function(...args) {
            origErr.apply(console, args);
            if (window.pywebview && window.pywebview.api) {
                window.pywebview.api.err(args.map(a => typeof a === 'object' ? JSON.stringify(a) : String(a)).join(' '));
            }
        };
    </script>
    <script async crossorigin="anonymous"
        data-clerk-publishable-key="pk_test_ZXhhY3QtcXVldHphbC03ODE2LmNsZXJrLmFjY291bnRzLmRldiQ"
        src="https://exact-quetzal-7816.clerk.accounts.dev/npm/@clerk/clerk-js@5/dist/clerk.browser.js"
        type="text/javascript"></script>
</head>
<body style="background:#111; color:#fff;">
    <h1>Testing mountSignIn</h1>
    <div id="clerk-mount" style="min-height: 450px; width: 400px; margin: 20px auto; background: #222;"></div>
</body>
</html>
"""

w = webview.create_window("SoundFlow Mount Test", html=html, js_api=api)
webview.start(private_mode=False, storage_path=storage_dir, http_server=True)
