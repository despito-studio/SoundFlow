import os
import json
import socket
import threading
import bottle

class MobileServer:
    """Lightweight mobile HTTP server enabling iPhone/iOS devices on LAN to stream SoundFlow."""
    def __init__(self, api, base_dir, port=8088):
        self.api = api
        self.base_dir = base_dir
        self.port = port
        self.frontend_dir = os.path.join(base_dir, "frontend")
        self.app = bottle.Bottle()
        self.thread = None
        self._setup_routes()

    def get_lan_ip(self):
        """Retrieves local network IP address."""
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.connect(("8.8.8.8", 80))
            ip = s.getsockname()[0]
            s.close()
            return ip
        except Exception:
            return "127.0.0.1"

    def get_mobile_url(self):
        ip = self.get_lan_ip()
        return f"http://{ip}:{self.port}"

    def _setup_routes(self):
        app = self.app
        frontend_dir = self.frontend_dir

        @app.hook('after_request')
        def enable_cors():
            bottle.response.headers['Access-Control-Allow-Origin'] = '*'
            bottle.response.headers['Access-Control-Allow-Methods'] = 'GET, POST, OPTIONS'
            bottle.response.headers['Access-Control-Allow-Headers'] = 'Origin, Accept, Content-Type, X-Requested-With'

        @app.route('/manifest.json')
        def manifest():
            bottle.response.content_type = 'application/manifest+json; charset=utf-8'
            return bottle.static_file('manifest.json', root=frontend_dir)

        @app.route('/')
        @app.route('/mobile')
        def index():
            bottle.response.set_header('Cache-Control', 'no-cache, no-store, must-revalidate')
            return bottle.static_file('index.html', root=frontend_dir)

        @app.route('/<filepath:path>')
        def static_files(filepath):
            return bottle.static_file(filepath, root=frontend_dir)

        # ==========================================
        # REST API for Mobile Clients
        # ==========================================
        @app.route('/api/info')
        def api_info():
            bottle.response.content_type = 'application/json'
            return json.dumps({
                "app": "SoundFlow Mobile",
                "version": "2.4.0",
                "lan_url": self.get_mobile_url(),
                "ip": self.get_lan_ip(),
                "port": self.port
            })

        @app.route('/api/search', method=['GET', 'POST'])
        def api_search():
            bottle.response.content_type = 'application/json'
            q = bottle.request.params.get('q') or bottle.request.params.get('query') or ''
            source = bottle.request.params.get('source', 'all')
            limit = int(bottle.request.params.get('limit', 16))
            results = self.api.search(q, source=source, limit=limit)
            return json.dumps(results)

        @app.route('/api/discover', method=['GET'])
        def api_discover():
            bottle.response.content_type = 'application/json'
            genre = bottle.request.params.get('genre', 'lofi')
            results = self.api.get_discover_tracks(genre)
            return json.dumps(results)

        @app.route('/api/stream', method=['GET', 'POST'])
        def api_stream():
            bottle.response.content_type = 'application/json'
            if bottle.request.method == 'POST':
                data = bottle.request.json or {}
                track = data.get('track', data)
            else:
                track_id = bottle.request.params.get('id')
                track_url = bottle.request.params.get('url')
                track_source = bottle.request.params.get('source', 'youtube')
                track = {'id': track_id, 'url': track_url, 'source': track_source}

            stream_url = self.api.get_stream(track)
            return json.dumps({"stream_url": stream_url})

        @app.route('/api/similar', method=['POST'])
        def api_similar():
            bottle.response.content_type = 'application/json'
            data = bottle.request.json or {}
            track = data.get('track', {})
            limit = int(data.get('limit', 10))
            exclude = data.get('exclude_ids', [])
            sim = self.api.get_similar_tracks(track, limit=limit, exclude_ids=exclude)
            return json.dumps(sim)

        @app.route('/api/library', method=['GET'])
        def api_library():
            bottle.response.content_type = 'application/json'
            return json.dumps(self.api.get_library())

        @app.route('/api/like', method=['POST'])
        def api_like():
            bottle.response.content_type = 'application/json'
            track = bottle.request.json or {}
            res = self.api.toggle_like(track)
            return json.dumps(res)

        @app.route('/api/resolve_direct', method=['GET', 'POST'])
        def api_resolve_direct():
            bottle.response.content_type = 'application/json'
            url = bottle.request.params.get('url') or (bottle.request.json or {}).get('url', '')
            res = self.api.resolve_direct_url(url)
            return json.dumps(res)

    def start(self):
        def run_server():
            try:
                bottle.run(app=self.app, host='0.0.0.0', port=self.port, quiet=True)
            except Exception as e:
                print(f"Mobile server error on port {self.port}: {e}")

        self.thread = threading.Thread(target=run_server, daemon=True)
        self.thread.start()
        print(f"SoundFlow Mobile server running at {self.get_mobile_url()}")
