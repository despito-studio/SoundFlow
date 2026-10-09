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
                window.Clerk.openSignIn();
                return "openSignIn called successfully!";
            } catch (e) {
                return "Error: " + e.toString();
            }
        })()
    """)
    print("openSignIn call:", res)
    time.sleep(3)
    dom = w.evaluate_js("""
        (function() {
            var modal = document.querySelector('.cl-modalBackdrop, .cl-modal, [class*="cl-"]');
            return JSON.stringify({
                foundModal: !!modal,
                tagName: modal ? modal.tagName : null,
                className: modal ? modal.className : null
            });
        })()
    """)
    print("DOM Check modal:", dom)
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
    <h1>Clerk openSignIn Test</h1>
</body>
</html>
"""

html_file = os.path.abspath("test_page_open.html")
with open(html_file, "w", encoding="utf-8") as f:
    f.write(html)

w = webview.create_window("Clerk openSignIn Test", url=html_file, js_api=api)
webview.start(private_mode=False, storage_path=storage_dir, http_server=True)
