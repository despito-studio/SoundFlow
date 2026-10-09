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
            try {
                if (!window.Clerk) return "No window.Clerk";
                var mount = document.getElementById('clerk-mount');
                window.Clerk.mountSignIn(mount);
                return "mountSignIn called successfully!";
            } catch (e) {
                return "Error: " + e.toString();
            }
        })()
    """)
    print("mountSignIn call:", res)
    time.sleep(3)
    dom = w.evaluate_js("""
        (function() {
            var mount = document.getElementById('clerk-mount');
            return JSON.stringify({
                htmlLen: mount ? mount.innerHTML.length : 0,
                childCount: mount ? mount.children.length : 0,
                firstTag: mount && mount.firstElementChild ? mount.firstElementChild.tagName : null
            });
        })()
    """)
    print("DOM Check:", dom)
    time.sleep(2)
    w.destroy()

threading.Thread(target=check, daemon=True).start()

html = """
<!DOCTYPE html>
<html>
<head>
    <script async crossorigin="anonymous"
        data-clerk-publishable-key="pk_test_ZXhhY3QtcXVldHphbC03ODE2LmNsZXJrLmFjY291bnRzLmRldiQ"
        src="https://exact-quetzal-7816.clerk.accounts.dev/npm/@clerk/clerk-js@latest/dist/clerk.browser.js"
        type="text/javascript"></script>
    <script>
        window.addEventListener("load", async function () {
            try {
                await Clerk.load();
                console.log("Clerk loaded! User:", Clerk.user);
            } catch(e) {
                console.error("Clerk load error:", e);
            }
        });
    </script>
</head>
<body style="background:#111; color:#fff;">
    <h1>Clerk Frontend API Test</h1>
    <div id="clerk-mount" style="min-height: 450px; width: 400px; margin: 20px auto; background: #222;"></div>
</body>
</html>
"""

# We must serve as file or via server
html_file = os.path.abspath("test_page.html")
with open(html_file, "w", encoding="utf-8") as f:
    f.write(html)

w = webview.create_window("Clerk Frontend API Test", url=html_file, js_api=api)
webview.start(private_mode=False, storage_path=storage_dir, http_server=True)
