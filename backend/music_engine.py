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
            with yt_dlp.YoutubeDL(self.base_search_opts) as ydl:
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
        target_url = track_info.get("url") or track_info.get("source_url") or track_info.get("webpage_url")
        if not target_url and track_id and str(track_id).startswith("yt_"):
            target_url = f"https://www.youtube.com/watch?v={track_id[3:]}"
        if target_url:
            track_info["url"] = target_url

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

    def _detect_track_language(self, title, artist):
        """Identifies track language/cultural origin to guarantee language matching."""
        import re
        text = f"{artist} {title}".lower()

        # 1. Explicit Ukrainian artist / keyword checks
        ua_artists = {
            "sadsvit", "структура щастя", "океан ельзи", "бумбокс", "скрябін", "антитіла",
            "христина соловій", "пивоваров", "клавдія петрівна", "go_a", "dorofeeva",
            "the hardkiss", "паліндром", "schmalgauzen", "жадан", "alyona alyona", "kalush",
            "друга ріка", "мотор'ролла", "латексфауна", "latexfauana", "казака", "kazka",
            "тіна кароль", "дзідзьо", "dzidzio", "один в каное", "без обмежень"
        }
        for a in ua_artists:
            if a in text:
                return "ua"
        if any(c in text for c in "іїєґІЇЄҐ"):
            return "ua"

        # 2. Explicit Russian artist / keyword checks
        ru_artists = {
            "молчат дома", "буерак", "перемотка", "дурной вкус", "дайте танк", "черниковская хата",
            "электрофорез", "ssshhhiiittt", "увула", "ploho", "кино", "би-2", "сплин", "земфира",
            "король и шут", "три дня дождя", "нервы", "валентин стрыкало", "kizaru", "shadowraze",
            "miyagi", "скриптонит", "macan", "пошлая молли", "порнофильмы", "сектор газа",
            "zxcursed", "og buda", "mayot", "big baby tape", "платина", "oxxxymiron", "моргенштерн",
            "мукка", "дореволюционный советчик", "петля пристрастия", "двое", "хмурый"
        }
        for a in ru_artists:
            if a in text:
                return "ru"
        if any(c in text for c in "ыэъёЫЭЪЁ"):
            return "ru"

        # 3. Cyrillic frequency check (default Cyrillic without UA characters is RU)
        cyr = len(re.findall(r'[\u0400-\u04FF]', text))
        if cyr >= 2:
            return "ru"

        # 4. Asian languages check
        if re.search(r'[\u3040-\u30ff\u3400-\u4dbf\u4e00-\u9fff]', text):
            return "cjk"

        return "en"

    def _detect_track_theme(self, title, artist):
        """Classifies track into thematic mood categories (depressive/doomer, phonk, rap, rock, lofi, etc.)."""
        text = f"{artist} {title}".lower()

        phonk_words = {
            'phonk', 'фонк', 'drift', 'дрифт', 'zxc', 'shadowraze', 'kordhell', 'playaphonk',
            'interworld', 'gxsder', 'prxjek', 'scarlxrd', 'cowbell', 'aggressive', 'агрессив',
            'workout', 'gym', 'murder in my mind', 'dvrst', 'zxcursed', 'cursed'
        }
        for w in phonk_words:
            if w in text:
                return 'phonk'

        depressive_words = {
            'депрессив', 'грустн', 'печаль', 'тоск', 'домер', 'слез', 'сльоз', 'боль', 'одиночеств',
            'уныни', 'суицид', 'тяжело', 'устал', 'пост-панк', 'постпанк', 'плакать', 'плачу',
            'пустота', 'холод', 'ночь', 'безнадег', 'судно', 'расставание', 'депрессия', 'сумно',
            'страсть к курению', 'электрофорез', 'буерак', 'перемотка', 'ssshhhiiittt', 'дурной вкус',
            'увула', 'черниковская хата', 'sadsvit', 'структура щастя', 'molchat doma', 'молчат дома',
            'ploho', 'плохо', 'sad', 'depress', 'doomer', 'post-punk', 'melanchol', 'heartbreak',
            'cry', 'crying', 'pain', 'alone', 'lonely', 'broken', 'dark', 'slowcore', 'goth',
            'shoegaze', 'gloomy', 'bleak', 'emo', 'hopeless', 'star shopping', 'cigarettes after sex',
            'joji', 'mitski', 'eyedress', 'mareux', 'current joys', 'beach fossils', 'surf curse',
            'дайте танк', 'панелька', 'хрущевка', 'утро'
        }
        for w in depressive_words:
            if w in text:
                return 'depressive'

        rap_words = {
            'рэп', 'реп', 'хип хоп', 'хип-хоп', 'трэп', 'треп', 'дрил', 'дрилл', 'rap', 'hip hop',
            'trap', 'drill', 'kizaru', 'og buda', 'macan', 'скриптонит', 'oxxxymiron', 'моргенштерн',
            'miyagi', 'эндшпиль', 'дежавю', 'travis scott', 'playboi carti', '21 savage', 'drake',
            'майот', 'mayot', 'big baby tape', 'платина', 'friendly thug'
        }
        for w in rap_words:
            if w in text:
                return 'rap'

        rock_words = {
            'рок', 'панк', 'метал', 'альтернатив', 'грандж', 'rock', 'punk', 'metal', 'grunge',
            'alternative', 'король и шут', 'кино', 'би-2', 'сплин', 'земфира', 'нервы',
            'пошлая молли', 'три дня дождя', 'порнофильмы', 'arctic monkeys', '505', 'nirvana',
            'linkin park', 'океан ельзи', 'the hardkiss', 'карна', 'жадан'
        }
        for w in rock_words:
            if w in text:
                return 'rock'

        lofi_words = {'lofi', 'lo-fi', 'chill', 'лофай', 'чилл', 'релакс', 'relax', 'спокойн', 'beats', 'study'}
        for w in lofi_words:
            if w in text:
                return 'lofi'

        pop_words = {'pop', 'dance', 'club', 'поп', 'попса', 'танцевальн', 'клубняк', 'диско', 'disco', 'house'}
        for w in pop_words:
            if w in text:
                return 'pop'

        return 'general'

    def _get_cluster_artists(self, lang, theme):
        """Returns related artists for genre & language clusters to query direct high-quality tracks."""
        clusters = {
            ("ru", "depressive"): ["Перемотка", "Буерак", "Дурной Вкус", "Дайте танк", "Черниковская Хата", "Электрофорез", "ssshhhiiittt", "Увула", "Молчат Дома", "Ploho", "Культура курения", "Хмурый"],
            ("ru", "phonk"): ["DVRST", "Shadowraze", "zxcursed", "KSLV Noh", "plhxntxnm", "Shadxwbxrn", "Gravechill"],
            ("ru", "rap"): ["Kizaru", "OG Buda", "Mayot", "Big Baby Tape", "Платина", "Скриптонит", "Miyagi & Эндшпиль", "Macan", "LSP", "PHARAOH"],
            ("ru", "rock"): ["Кино", "Би-2", "Сплин", "Земфира", "Король и Шут", "Три дня дождя", "Порнофильмы", "Нервы", "Валентин Стрыкало", "Пошлая Молли"],
            ("ru", "pop"): ["ANNA ASTI", "Zivert", "Mary Gu", "HammAli & Navai", "JONY", "Моя Мишель", "Cream Soda"],
            ("ru", "lofi"): ["русский чилл", "русский лофай"],

            ("ua", "depressive"): ["SadSvit", "Структура Щастя", "Паліндром", "Schmalgauzen", "Океан Ельзи", "Скрябін", "Мотор'ролла", "Латексфауна"],
            ("ua", "rock"): ["Океан Ельзи", "Бумбокс", "Скрябін", "Антитіла", "The Hardkiss", "Друга Ріка", "Карна", "Жадан і Собаки"],
            ("ua", "rap"): ["alyona alyona", "Kalush", "OTOY", "Krechet", "Міша Крупін", "Skofka"],
            ("ua", "pop"): ["DOROFEEVA", "KAZKA", "Артем Пивоваров", "Jerry Heil", "KOLA", "Клавдія Петрівна"],

            ("en", "depressive"): ["Mareux", "Eyedress", "Current Joys", "The Smiths", "Joy Division", "Cigarettes After Sex", "Slowdive", "Beach Fossils", "Surf Curse", "Mitski", "Joji", "TV Girl"],
            ("en", "phonk"): ["Kordhell", "SXMPRA", "Interworld", "PlayaPhonk", "MoonDeity", "Twisted", "Hensonn", "Pharmacist"],
            ("en", "rap"): ["Travis Scott", "Playboi Carti", "Lil Peep", "Juice WRLD", "21 Savage", "Metro Boomin", "Don Toliver", "Kendrick Lamar"],
            ("en", "rock"): ["Arctic Monkeys", "The Neighbourhood", "Deftones", "Radiohead", "Nirvana", "Linkin Park", "Bring Me The Horizon", "Green Day"],
            ("en", "lofi"): ["Kudasai", "Idealism", "Jinsang", "potsu", "Saib", "Tomppabeats"],
            ("en", "pop"): ["The Weeknd", "Dua Lipa", "Billie Eilish", "Harry Styles", "Taylor Swift"]
        }
        return clusters.get((lang, theme), [])

    def get_similar_tracks(self, track_info, limit=10, exclude_ids=None):
        """Finds recommended songs matching both the exact theme/mood AND language of the given track."""
        if not track_info:
            return []

        import re
        import random
        exclude_set = set(exclude_ids or [])
        track_id = track_info.get("id")
        if track_id:
            exclude_set.add(track_id)

        title = track_info.get("title", "")
        artist = track_info.get("artist", "").strip()

        # Clean title from common video noise
        clean_title = re.sub(r'(?i)\b(official\s+(audio|video|music\s+video)|lyrics?|visualizer|remastered|hd|4k|hq)\b', '', title)
        clean_title = re.sub(r'[\(\[\{\]\}\)]', ' ', clean_title)
        clean_title = ' '.join(clean_title.split())

        generic_artists = {"youtube creator", "soundcloud artist", "unknown artist", "soundflow artist", "soundflow", ""}
        if not artist or artist.lower() in generic_artists:
            for sep in [" - ", " – ", " — "]:
                if sep in title:
                    parts = title.split(sep, 1)
                    artist = parts[0].strip()
                    clean_title = parts[1].strip()
                    break

        lang = self._detect_track_language(clean_title, artist)
        theme = self._detect_track_theme(clean_title, artist)
        cluster = self._get_cluster_artists(lang, theme)

        cache_key = f"sim_v4:{lang}:{theme}:{track_id or title}:{limit}"
        now = time.time()
        if cache_key in self.search_cache:
            entry = self.search_cache[cache_key]
            if now - entry.get("time", 0) < 1800:
                return entry.get("results", [])

        # Priority 1: Search queries matching artist, theme, and language
        queries = []
        if artist and artist.lower() not in generic_artists:
            if lang == "ru":
                if theme == "depressive":
                    queries.append(f"{artist} похожие песни пост панк")
                    queries.append(f"{artist} русский пост панк")
                elif theme == "phonk":
                    queries.append(f"{artist} русский фонк")
                elif theme == "rap":
                    queries.append(f"{artist} похожие треки рэп")
                elif theme == "rock":
                    queries.append(f"{artist} русский рок")
                else:
                    queries.append(f"{artist} похожие песни русский")
            elif lang == "ua":
                if theme == "depressive":
                    queries.append(f"{artist} український пост панк сумна музика")
                elif theme == "rock":
                    queries.append(f"{artist} український рок")
                elif theme == "rap":
                    queries.append(f"{artist} український реп")
                else:
                    queries.append(f"{artist} схожі українські пісні")
            else: # en / other
                if theme == "depressive":
                    queries.append(f"{artist} similar sad doomer songs")
                elif theme == "phonk":
                    queries.append(f"{artist} drift phonk mix")
                elif theme == "rock":
                    queries.append(f"{artist} similar alternative rock")
                elif theme == "rap":
                    queries.append(f"{artist} similar rap radio")
                else:
                    queries.append(f"{artist} similar songs radio")

        # Priority 2: Direct cluster queries from genre-defining peers
        if cluster:
            sample_artists = [a for a in cluster if a.lower() != artist.lower()]
            if sample_artists:
                for art in random.sample(sample_artists, min(3, len(sample_artists))):
                    queries.append(f"{art} трек")

        # Priority 3: Thematic genre radio queries
        if lang == "ru":
            if theme == "depressive":
                queries.append("русский пост панк домер треки")
            elif theme == "phonk":
                queries.append("русский фонк zxc дрифт")
            elif theme == "rap":
                queries.append("русский рэп новинки хиты")
            elif theme == "rock":
                queries.append("русский рок альтернатива песни")
        elif lang == "ua":
            if theme == "depressive":
                queries.append("український пост панк інді сумна музика")
            elif theme == "rock":
                queries.append("український рок треки")
        elif lang == "en":
            if theme == "depressive":
                queries.append("doomer music sad post-punk tracks")
            elif theme == "phonk":
                queries.append("drift phonk tracks")

        if not queries:
            queries.append(f"{clean_title} radio")

        candidates = []
        seen_titles = {clean_title.lower()}

        for q in queries:
            if len(candidates) >= limit:
                break
            found = self.search(q, source="all", limit=max(6, limit // 2))
            for item in found:
                item_id = item.get("id")
                item_title = item.get("title", "").strip()
                item_artist = item.get("artist", "").strip()
                norm_item_title = re.sub(r'[\(\[\{\]\}\)]', ' ', item_title.lower()).strip()

                if item_id in exclude_set:
                    continue
                if norm_item_title in seen_titles:
                    continue

                dur = item.get("duration", 0)
                # Discard multi-hour compilations and tiny audio clips (<30s or >720s)
                if dur and (dur < 30 or dur > 720):
                    continue

                # Strict language compatibility check
                item_lang = self._detect_track_language(item_title, item_artist)
                if lang == "ru" and item_lang != "ru":
                    continue
                if lang == "ua" and item_lang != "ua":
                    continue
                if lang == "en" and item_lang in ("ru", "ua"):
                    continue

                seen_titles.add(norm_item_title)
                exclude_set.add(item_id)
                candidates.append(item)
                if len(candidates) >= limit:
                    break

        self.search_cache[cache_key] = {"results": candidates, "time": now}
        return candidates

    def import_soundcloud_profile(self, identifier, limit=40):
        """
        Imports liked tracks and uploaded tracks from a SoundCloud user profile.
        Accepts: username or full URL (e.g. 'nocopyrightsounds' or 'https://soundcloud.com/nocopyrightsounds').
        """
        if not identifier or not identifier.strip():
            return []

        clean = identifier.strip().rstrip('/')
        if not clean.startswith('http'):
            clean = f'https://soundcloud.com/{clean}'

        # Try likes first, then tracks
        targets = [f'{clean}/likes', f'{clean}/tracks', clean]
        collected = []
        seen_urls = set()

        opts = {
            'quiet': True,
            'extract_flat': True,
            'playlistend': limit,
            'ignoreerrors': True
        }

        with yt_dlp.YoutubeDL(opts) as ydl:
            for target_url in targets:
                if len(collected) >= limit:
                    break
                try:
                    info = ydl.extract_info(target_url, download=False)
                    if not info:
                        continue
                    entries = info.get('entries') or ([info] if info.get('url') else [])
                    for e in entries:
                        if not e or not isinstance(e, dict):
                            continue
                        track_url = e.get('url') or e.get('webpage_url')
                        if not track_url or track_url in seen_urls:
                            continue
                        seen_urls.add(track_url)

                        title = e.get('title') or 'SoundCloud Track'
                        uploader = e.get('uploader') or e.get('artist') or 'SoundCloud'
                        duration = int(e.get('duration') or 0)
                        thumb = e.get('thumbnail') or self._extract_best_thumbnail(e) or 'assets/icon.png'
                        track_id = f"sc_{abs(hash(track_url))}"

                        collected.append({
                            'id': track_id,
                            'title': title,
                            'artist': uploader,
                            'duration': duration,
                            'duration_formatted': self._format_duration(duration),
                            'thumbnail': thumb,
                            'url': track_url,
                            'source': 'soundcloud',
                            'source_url': track_url,
                            'imported_from': 'soundcloud'
                        })
                        if len(collected) >= limit:
                            break
                except Exception as ex:
                    print(f"Error importing from {target_url}: {ex}")

        return collected

    def import_youtube_profile(self, identifier, limit=40):
        """
        Imports tracks from a YouTube channel, handle, or playlist URL.
        Accepts: handle (@username), channel URL, or playlist URL.
        """
        if not identifier or not identifier.strip():
            return []

        clean = identifier.strip()
        if 'playlist?list=' in clean:
            targets = [clean]
        elif clean.startswith('@'):
            targets = [f'https://www.youtube.com/{clean}/videos', f'https://www.youtube.com/{clean}']
        elif 'youtube.com' in clean or 'youtu.be' in clean:
            targets = [clean if '/videos' in clean else f"{clean.rstrip('/')}/videos", clean]
        else:
            targets = [f'https://www.youtube.com/@{clean}/videos', f'https://www.youtube.com/@{clean}']

        collected = []
        seen_ids = set()

        opts = {
            'quiet': True,
            'extract_flat': True,
            'playlistend': limit,
            'ignoreerrors': True
        }

        with yt_dlp.YoutubeDL(opts) as ydl:
            for target_url in targets:
                if len(collected) >= limit:
                    break
                try:
                    info = ydl.extract_info(target_url, download=False)
                    if not info:
                        continue
                    entries = info.get('entries') or ([info] if info.get('id') else [])
                    for e in entries:
                        if not e or not isinstance(e, dict):
                            continue
                        yt_id = e.get('id')
                        if not yt_id or yt_id in seen_ids:
                            continue
                        seen_ids.add(yt_id)

                        title = e.get('title') or 'YouTube Music'
                        uploader = e.get('uploader') or e.get('channel') or 'YouTube'
                        duration = int(e.get('duration') or 0)
                        thumb = e.get('thumbnail') or f"https://i.ytimg.com/vi/{yt_id}/hqdefault.jpg"
                        yt_watch_url = f"https://www.youtube.com/watch?v={yt_id}"

                        collected.append({
                            'id': f"yt_{yt_id}",
                            'title': title,
                            'artist': uploader,
                            'duration': duration,
                            'duration_formatted': self._format_duration(duration),
                            'thumbnail': thumb,
                            'url': yt_watch_url,
                            'source': 'youtube',
                            'source_url': yt_watch_url,
                            'imported_from': 'youtube'
                        })
                        if len(collected) >= limit:
                            break
                except Exception as ex:
                    print(f"Error importing from {target_url}: {ex}")

        return collected

    def resolve_direct_url(self, url):
        """Resolves a direct YouTube or SoundCloud URL into a playable track object."""
        if not url:
            return None
        url = url.strip()
        opts = {
            'quiet': True,
            'skip_download': True,
            'extract_flat': False,
            'noplaylist': True
        }
        try:
            with yt_dlp.YoutubeDL(opts) as ydl:
                info = ydl.extract_info(url, download=False)
                if not info:
                    return None
                source = 'soundcloud' if 'soundcloud.com' in url else 'youtube'
                raw_id = info.get('id') or abs(hash(url))
                t_id = f"sc_{raw_id}" if source == 'soundcloud' else f"yt_{raw_id}"
                duration = int(info.get('duration') or 0)
                return {
                    'id': t_id,
                    'title': info.get('title') or 'Track',
                    'artist': info.get('uploader') or info.get('artist') or ('SoundCloud' if source == 'soundcloud' else 'YouTube'),
                    'duration': duration,
                    'duration_formatted': self._format_duration(duration),
                    'thumbnail': self._extract_best_thumbnail(info),
                    'url': url,
                    'source': source,
                    'source_url': url
                }
        except Exception as e:
            print(f"Error resolving direct URL {url}: {e}")
            return None

