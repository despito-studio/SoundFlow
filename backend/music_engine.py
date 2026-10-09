import os
import shutil
import time
from concurrent.futures import ThreadPoolExecutor
import yt_dlp

class MusicEngine:
    """High-performance music search and direct audio stream extractor using yt-dlp."""
    def __init__(self):
        self.node_path = self._find_node()
        self.stream_cache = {}  # {track_id: {"url": str, "expires_at": float}}
        self.search_cache = {}  # {cache_key: {"results": list, "time": float}}

        # Base YDL options for ultra-fast flat search
        self.base_search_opts = {
            "quiet": True,
            "no_warnings": True,
            "extract_flat": True,
            "skip_download": True,
            "ignoreerrors": True,
            "socket_timeout": 8,
        }

        # Playback extraction options: prioritize progressive HTTP/HTTPS audio and exclude HLS/m3u8
        self.playback_opts = {
            "quiet": True,
            "no_warnings": True,
            "format": "bestaudio[protocol^=http][ext=mp3]/bestaudio[protocol^=http][ext=webm]/bestaudio[protocol^=http][ext=m4a]/bestaudio[protocol^=http]/bestaudio/best",
            "noplaylist": True,
            "skip_download": True,
            "ignoreerrors": True,
            "socket_timeout": 8,
        }
        if self.node_path:
            self.playback_opts["js_runtimes"] = {"node": {"path": self.node_path}}

    def _find_node(self):
        w = shutil.which("node")
        if w:
            return w
        candidates = [
            os.path.expanduser(r"~/.gemini/antigravity/bin/nodejs/node.exe"),
            os.path.expanduser(r"~\AppData\Local\Programs\nodejs\node.exe"),
            r"C:\Program Files\nodejs\node.exe",
            r"C:\Program Files (x86)\nodejs\node.exe",
        ]
        local_appdata = os.environ.get("LOCALAPPDATA")
        if local_appdata:
            candidates.append(os.path.join(local_appdata, "Programs", "nodejs", "node.exe"))
        for c in candidates:
            if os.path.exists(c):
                return c
        return None

    def _format_duration(self, seconds):
        if not seconds or seconds <= 0:
            return "--:--"
        try:
            sec = int(seconds)
            m, s = divmod(sec, 60)
            h, m = divmod(m, 60)
            if h > 0:
                return f"{h}:{m:02d}:{s:02d}"
            return f"{m}:{s:02d}"
        except Exception:
            return "--:--"

    def _extract_best_thumbnail(self, entry):
        thumbs = entry.get("thumbnails")
        if thumbs and isinstance(thumbs, list):
            for t in reversed(thumbs):
                url = t.get("url")
                if url:
                    return url
        return entry.get("thumbnail") or "assets/default_art.png"

    def search_soundcloud(self, query, limit=10):
        try:
            with yt_dlp.YoutubeDL(self.base_search_opts) as ydl:
                res = ydl.extract_info(f"scsearch{limit}:{query}", download=False)
                raw_entries = res.get("entries", []) if res else []
                clean = []
                for e in raw_entries:
                    if not e or not e.get("title"):
                        continue
                    track_id = f"sc_{e.get('id') or abs(hash(e.get('url', '')))}"
                    clean.append({
                        "id": track_id,
                        "title": e.get("title", "Unknown Title"),
                        "artist": e.get("uploader") or e.get("channel") or "SoundCloud Artist",
                        "duration": e.get("duration") or 0,
                        "duration_formatted": self._format_duration(e.get("duration")),
                        "thumbnail": self._extract_best_thumbnail(e),
                        "url": e.get("url") or e.get("webpage_url"),
                        "source": "soundcloud",
                    })
                return clean
        except Exception as e:
            print("SoundCloud search error:", e)
            return []

    def search_youtube(self, query, limit=10):
        try:
            opts = dict(self.base_search_opts)
            if self.node_path:
                opts["js_runtimes"] = {"node": {"path": self.node_path}}

            with yt_dlp.YoutubeDL(opts) as ydl:
                res = ydl.extract_info(f"ytsearch{limit}:{query}", download=False)
                raw_entries = res.get("entries", []) if res else []
                clean = []
                for e in raw_entries:
                    if not e or not e.get("title"):
                        continue
                    if e.get("live_status") in ("is_live", "is_upcoming") or e.get("is_live"):
                        continue
                    vid_id = e.get("id") or str(abs(hash(e.get("url", ""))))
                    track_url = e.get("url")
                    if not track_url or not track_url.startswith("http"):
                        track_url = f"https://www.youtube.com/watch?v={vid_id}"

                    clean.append({
                        "id": f"yt_{vid_id}",
                        "title": e.get("title", "Unknown Title"),
                        "artist": e.get("uploader") or e.get("channel") or "YouTube Creator",
                        "duration": e.get("duration") or 0,
                        "duration_formatted": self._format_duration(e.get("duration")),
                        "thumbnail": self._extract_best_thumbnail(e),
                        "url": track_url,
                        "source": "youtube",
                    })
                return clean
        except Exception as e:
            print("YouTube search error:", e)
            return []

    def search(self, query, source="all", limit=12):
        query = query.strip()
        if not query:
            return []

        cache_key = f"{source}:{query}:{limit}"
        now = time.time()
        if cache_key in self.search_cache:
            entry = self.search_cache[cache_key]
            if now - entry["time"] < 600:  # 10 min cache
                return entry["results"]

        results = []
        if source == "soundcloud":
            results = self.search_soundcloud(query, limit=limit)
        elif source == "youtube":
            results = self.search_youtube(query, limit=limit)
        else:
            # Parallel search across both SoundCloud and YouTube
            half = max(4, limit // 2)
            with ThreadPoolExecutor(max_workers=2) as ex:
                f_sc = ex.submit(self.search_soundcloud, query, half)
                f_yt = ex.submit(self.search_youtube, query, half)
                sc_res = f_sc.result()
                yt_res = f_yt.result()

            # Interleave results: 1 SC, 1 YT, 1 SC, 1 YT...
            max_len = max(len(sc_res), len(yt_res))
            for i in range(max_len):
                if i < len(sc_res):
                    results.append(sc_res[i])
                if i < len(yt_res):
                    results.append(yt_res[i])

        self.search_cache[cache_key] = {"results": results, "time": now}
        return results

    def get_stream_url(self, track_info, force_refresh=False):
        """Resolves direct streamable progressive audio URL for the given track info."""
        if not track_info:
            return None

        track_id = track_info.get("id")
        target_url = track_info.get("url")

        if force_refresh and track_id:
            self.stream_cache.pop(track_id, None)

        now = time.time()
        if not force_refresh and track_id and track_id in self.stream_cache:
            cached = self.stream_cache[track_id]
            if cached.get("expires_at", 0) > now:
                return cached["stream_url"]

        if not target_url:
            return None

        try:
            with yt_dlp.YoutubeDL(self.playback_opts) as ydl:
                info = ydl.extract_info(target_url, download=False)
                if not info:
                    return None

                # Discard 24/7 live streams that cannot be played progressively
                if info.get("is_live"):
                    return None

                stream_url = None

                # 1. Filter formats for direct progressive HTTP/HTTPS audio (exclude m3u8)
                formats = info.get("formats", [])
                if formats:
                    valid_audio = []
                    for f in formats:
                        u = f.get("url") or ""
                        proto = f.get("protocol") or ""
                        if not u.startswith("http"):
                            continue
                        if ".m3u8" in u or "m3u8" in proto:
                            continue
                        if f.get("acodec") != "none" or f.get("vcodec") == "none":
                            valid_audio.append(f)

                    if valid_audio:
                        # Prefer audio-only (vcodec == 'none'), then webm/opus or mp3, then m4a, then highest bitrate
                        valid_audio.sort(
                            key=lambda x: (
                                1 if x.get("vcodec") == "none" else 0,
                                2 if x.get("ext") in ("mp3", "webm") else (1 if x.get("ext") == "m4a" else 0),
                                x.get("abr") or x.get("tbr") or 0
                            ),
                            reverse=True
                        )
                        stream_url = valid_audio[0]["url"]

                # 2. Fallback to top-level URL if not m3u8
                if not stream_url:
                    top_url = info.get("url")
                    if top_url and ".m3u8" not in top_url:
                        stream_url = top_url

                if stream_url:
                    # Calculate safe cache TTL based on expire timestamp if present in query string
                    ttl = 3600  # Default 1 hour
                    try:
                        import urllib.parse
                        parsed = urllib.parse.urlparse(stream_url)
                        qs = urllib.parse.parse_qs(parsed.query)
                        if "expire" in qs:
                            exp_ts = float(qs["expire"][0])
                            remaining = exp_ts - now
                            if remaining > 120:
                                ttl = min(3600, remaining - 60)
                    except Exception:
                        pass

                    if track_id:
                        self.stream_cache[track_id] = {
                            "stream_url": stream_url,
                            "expires_at": now + ttl
                        }
                    return stream_url

        except Exception as e:
            print("Error resolving stream URL for", target_url, e)

        return None

    def get_similar_tracks(self, track_info, limit=10, exclude_ids=None):
        """Finds recommended songs with similar theme/genre/artist to provide endless smart autoplay."""
        if not track_info:
            return []

        import re
        exclude_set = set(exclude_ids or [])
        track_id = track_info.get("id")
        if track_id:
            exclude_set.add(track_id)

        title = track_info.get("title", "")
        artist = track_info.get("artist", "").strip()

        # Clean title from common video fluff
        clean_title = re.sub(r'(?i)\b(official\s+(audio|video|music\s+video)|lyrics?|visualizer|remastered|hd|4k|hq)\b', '', title)
        clean_title = re.sub(r'[\(\[\{\]\}\)]', ' ', clean_title)
        clean_title = ' '.join(clean_title.split())

        # If artist is generic, try to split title by '-'
        generic_artists = {"youtube creator", "soundcloud artist", "unknown artist", "soundflow artist", "soundflow"}
        if not artist or artist.lower() in generic_artists:
            if " - " in title:
                parts = title.split(" - ", 1)
                artist = parts[0].strip()
                clean_title = parts[1].strip()
            elif " – " in title:
                parts = title.split(" – ", 1)
                artist = parts[0].strip()
                clean_title = parts[1].strip()

        GENRE_KEYWORDS = [
            "phonk", "synthwave", "retrowave", "cyberpunk", "lofi", "lo-fi", "chill",
            "rock", "metal", "grunge", "punk", "indie", "pop", "hip hop", "rap", "trap",
            "r&b", "soul", "jazz", "blues", "edm", "house", "techno", "trance", "dubstep",
            "bass", "drum and bass", "dnb", "ambient", "acoustic", "classical", "anime"
        ]

        detected_genre = None
        lower_title = title.lower()
        for g in GENRE_KEYWORDS:
            if g in lower_title:
                detected_genre = g
                break

        # Generate search queries for similar music
        queries = []
        if artist and artist.lower() not in generic_artists:
            if detected_genre:
                queries.append(f"{artist} {detected_genre} mix")
            queries.append(f"{artist} radio mix")
            queries.append(f"{artist} similar music")
        
        if clean_title:
            queries.append(f"{clean_title} radio")

        if not queries:
            queries.append(f"{title} similar songs")

        cache_key = f"similar:{track_id or title}:{limit}"
        now = time.time()
        if cache_key in self.search_cache:
            entry = self.search_cache[cache_key]
            if now - entry.get("time", 0) < 1800:
                return entry.get("results", [])

        # Choose the single best search query to avoid locking backend
        target_query = queries[0] if queries else f"{title} mix"

        candidates = []
        seen_titles = {clean_title.lower()}
        original_duration = track_info.get("duration", 0)

        found = self.search(target_query, source="all", limit=limit)
        for item in found:
            item_id = item.get("id")
            item_title = item.get("title", "").strip()
            norm_item_title = re.sub(r'[\(\[\{\]\}\)]', ' ', item_title.lower()).strip()

            if item_id in exclude_set:
                continue
            if norm_item_title in seen_titles:
                continue

            # Avoid 1+ hour compilation sets if original track was a normal song (< 10 mins)
            dur = item.get("duration", 0)
            if original_duration and original_duration < 600 and dur > 1200:
                continue

            seen_titles.add(norm_item_title)
            exclude_set.add(item_id)
            candidates.append(item)
            if len(candidates) >= limit:
                break

        self.search_cache[cache_key] = {"results": candidates, "time": now}
        return candidates
