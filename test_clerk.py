import webview
import time
import threading
import os

appdata = os.environ.get("APPDATA")
storage_dir = os.path.join(appdata, "SoundFlow", "WebView2Profile")
os.makedirs(storage_dir, exist_ok=True)

def check():
    time.sleep(6)
    w = webview.windows[0]
    res = w.evaluate_js("""
        (async function() {
            try {
                if (!window.Clerk) return { error: "No Clerk" };
                await window.Clerk.load();
                var mount = document.getElementById('clerk-mount');
                window.Clerk.mountSignIn(mount);
                return { success: true };
            } catch (e) {
                return { error: e.toString() };
            }
        })()
    """)
    print("mountSignIn call:", res)
    time.sleep(3)
    dom_check = w.evaluate_js("""
        (function() {
            var mount = document.getElementById('clerk-mount');
            return {
                innerHTML_len: mount ? mount.innerHTML.length : 0,
                childCount: mount ? mount.children.length : 0,
                firstTag: mount && mount.firstElementChild ? mount.firstElementChild.tagName : null,
                classes: mount && mount.firstElementChild ? mount.firstElementChild.className : null
            };
        })()
    """)
    print("DOM Check after mountSignIn:", dom_check)
    time.sleep(2)
    w.destroy()

threading.Thread(target=check, daemon=True).start()

html = """
<!DOCTYPE html>
<html>
<head>
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

w = webview.create_window("SoundFlow Mount Test", html=html)
webview.start(private_mode=False, storage_path=storage_dir, http_server=True)
