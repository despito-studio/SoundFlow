import os
import sys
import json
import threading
import urllib.request
import bottle
import webview
import webview.http
from webview.util import abspath, is_app, is_local_url

def _patch_pywebview_http():
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
            target=lambda: bottle.run(app=app, server=server_adapter, port=server.port, quiet=True),
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

base_dir = os.path.dirname(os.path.abspath(__file__))
html_file = os.path.join(base_dir, "frontend", "index.html")

server_address, common_path, server = webview.http.BottleServer.start_server([html_file], 42099)
print("Server started at:", server_address)

# Test requesting the exact Clerk OAuth callback URL that crashed:
test_url = "http://127.0.0.1:42099/?__clerk_db_jwt=dvb_3KPmH384deLLR232sPWzDr2nPw6"
try:
    req = urllib.request.Request(test_url)
    with urllib.request.urlopen(req) as resp:
        content = resp.read().decode("utf-8")
        print("Response status:", resp.status)
        print("Content contains <!DOCTYPE html>:", "<!DOCTYPE html>" in content)
        print("Content length:", len(content))
except Exception as e:
    print("Request failed:", e)
