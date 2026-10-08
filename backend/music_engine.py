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
        }

        # Playback extraction options
        self.playback_opts = {
            "quiet": True,
            "no_warnings": True,
            "format": "bestaudio[ext=m4a]/bestaudio/best",
            "noplaylist": True,
            "skip_download": True,
            "ignoreerrors": True,
        }
        if self.node_path:
            self.playback_opts["js_runtimes"] = {"node": {"path": self.node_path}}

    def _find_node(self):
        w = shutil.which("node")
        if w:
            return w
        candidates = [
            os.path.expanduser(r"~/.gemini/antigravity/bin/nodejs/node.exe"),
            r"C:\Program Files\nodejs\node.exe",
            r"C:\Program Files (x86)\nodejs\node.exe",
        ]
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

    def get_stream_url(self, track_info):
        """Resolves direct streamable audio URL for the given track info."""
        if not track_info:
            return None

        track_id = track_info.get("id")
        target_url = track_info.get("url")

        # Check memory cache
        now = time.time()
        if track_id and track_id in self.stream_cache:
            cached = self.stream_cache[track_id]
            if cached["expires_at"] > now:
                return cached["stream_url"]

        if not target_url:
            return None

        try:
            with yt_dlp.YoutubeDL(self.playback_opts) as ydl:
                info = ydl.extract_info(target_url, download=False)
                if not info:
                    return None

                stream_url = info.get("url")

                # If url not directly at top level, check formats
                if not stream_url and "formats" in info:
                    # Prefer audio-only formats (m4a, webm/opus)
                    audio_formats = [
                        f for f in info["formats"]
                        if f.get("vcodec") == "none" and f.get("url")
                    ]
                    if audio_formats:
                        # Sort by audio bitrate
                        audio_formats.sort(key=lambda x: x.get("abr") or 0, reverse=True)
                        stream_url = audio_formats[0]["url"]
                    elif info["formats"]:
                        stream_url = info["formats"][-1].get("url")

                if stream_url:
                    # Cache stream URL for 2 hours
                    if track_id:
                        self.stream_cache[track_id] = {
                            "stream_url": stream_url,
                            "expires_at": now + 7200
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

        candidates = []
        seen_titles = {clean_title.lower()}
        original_duration = track_info.get("duration", 0)

        for q in queries:
            if len(candidates) >= limit:
                break
            found = self.search(q, source="all", limit=limit)
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

        return candidates
