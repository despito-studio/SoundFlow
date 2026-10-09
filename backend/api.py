import threading
from backend.music_engine import MusicEngine
from backend.library import LibraryManager
from backend.system_stats import SystemStats

class AppApi:
    """API bridge exposed to JavaScript in pywebview."""
    def __init__(self):
        self._engine = MusicEngine()
        self._library = LibraryManager()
        self._stats = SystemStats()
        self._desktop = None
        self._mobile_server = None

        # Popular genres for instant discovery
        self._genres_map = {
            "lofi": "lofi hip hop chill beats study relax",
            "synthwave": "synthwave retrowave electronic 80s",
            "phonk": "drift phonk aggressive bass workout",
            "deephouse": "deep house vocal chill summer mix",
            "rock": "classic rock modern rock hits",
            "gaming": "gaming music nocopyrightsounds edm",
            "ambient": "ambient relaxation meditation peaceful",
            "ua": "українська музика тренди хіти indie"
        }

    def search(self, query, source="all", limit=16):
        return self._engine.search(query, source=source, limit=limit)

    def get_discover_tracks(self, genre_key="lofi"):
        query = self._genres_map.get(genre_key, self._genres_map["lofi"])
        return self._engine.search(query, source="all", limit=16)

    def get_stream(self, track, force_refresh=False):
        return self._engine.get_stream_url(track, force_refresh=force_refresh)

    def get_similar_tracks(self, track, limit=10, exclude_ids=None):
        return self._engine.get_similar_tracks(track, limit=limit, exclude_ids=exclude_ids)

    def get_library(self):
        return self._library.get_all()

    def set_user(self, user_id=None, user_info=None):
        return self._library.set_user(user_id, user_info)

    def toggle_like(self, track):
        is_liked, liked_list = self._library.toggle_like(track)
        return {"is_liked": is_liked, "liked_count": len(liked_list)}

    def is_liked(self, track_id):
        return self._library.is_liked(track_id)

    def create_playlist(self, name):
        return self._library.create_playlist(name)

    def delete_playlist(self, playlist_id):
        return self._library.delete_playlist(playlist_id)

    def rename_playlist(self, playlist_id, new_name):
        return self._library.rename_playlist(playlist_id, new_name)

    def add_to_playlist(self, playlist_id, track):
        return self._library.add_to_playlist(playlist_id, track)

    def remove_from_playlist(self, playlist_id, track_id):
        return self._library.remove_from_playlist(playlist_id, track_id)

    def record_history(self, track):
        self._library.add_history(track)
        return True

    def clear_history(self):
        return self._library.clear_history()

    def update_settings(self, settings):
        return self._library.update_settings(settings)

    def get_profile(self):
        return self._library.get_profile()

    def update_profile(self, profile_data):
        return self._library.update_profile(profile_data)

    def clear_cache(self):
        return self._library.clear_cache()

    def export_library(self):
        return self._library.export_library()

    def import_library(self, json_str):
        return self._library.import_library(json_str)

    def get_system_stats(self):
        return self._stats.get_stats()

    def get_friends(self):
        return self._library.get_friends()

    def add_friend(self, friend_data):
        self._library.add_friend(friend_data)
        return self._library.get_friends()

    def remove_friend(self, friend_id):
        self._library.remove_friend(friend_id)
        return self._library.get_friends()

    def set_role_by_id(self, target_sf_id, new_role):
        return self._library.set_role_by_id(target_sf_id, new_role)

    def get_user_sf_id(self):
        return self._library.get_profile().get("sf_id", "")

    def get_all_users(self):
        return self._library.get_all_users()

    def trim_memory(self):
        return self._stats.trim_memory()

    def import_external_profile(self, platform, identifier):
        platform = str(platform).lower().strip()
        if platform == "soundcloud":
            tracks = self._engine.import_soundcloud_profile(identifier)
        elif platform in ("youtube", "yt"):
            tracks = self._engine.import_youtube_profile(identifier)
        else:
            return {"success": False, "error": f"Непідтримувана платформа: {platform}"}

        if not tracks:
            return {"success": False, "error": "Не знайдено пісень для імпорту або профіль недоступний"}

        return self._library.import_external_tracks(platform, tracks, identifier)

    def get_connected_accounts(self):
        return self._library.get_connected_accounts()

    def sync_connected_accounts(self):
        accs = self._library.get_connected_accounts()
        results = {}
        total_imported = 0
        for platform, ident in accs.items():
            if ident:
                res = self.import_external_profile(platform, ident)
                results[platform] = res
                if res.get("success"):
                    total_imported += res.get("count", 0)
        return {"success": True, "total_imported": total_imported, "details": results}

    def add_search_query(self, query):
        return self._library.add_search_query(query)

    def get_search_history(self):
        return self._library.get_search_history()

    def clear_search_history(self):
        return self._library.clear_search_history()

    def remove_search_query(self, query):
        return self._library.remove_search_query(query)

    def get_game_status(self):
        return self._stats.get_game_status()

    def set_desktop(self, desktop):
        self._desktop = desktop

    def update_playback_state(self, track, is_playing=True, position=0):
        if self._desktop:
            self._desktop.update_now_playing(track, is_playing, position)
        return True

    def resolve_direct_url(self, url):
        return self._engine.resolve_direct_url(url)

    def download_track(self, track):
        """Downloads audio track directly to user's Music/SoundFlow folder."""
        import urllib.request
        import re
        import os
        if not track or not track.get("id"):
            return {"success": False, "error": "Недійсний трек"}

        title = track.get("title") or "Track"
        artist = track.get("artist") or "SoundFlow"
        stream_url = self.get_stream(track)
        if not stream_url:
            return {"success": False, "error": "Не вдалося отримати аудіопотік для завантаження"}

        music_dir = os.path.join(os.path.expanduser("~"), "Music", "SoundFlow")
        os.makedirs(music_dir, exist_ok=True)

        safe_name = re.sub(r'[\\/*?:"<>|]', "", f"{artist} - {title}")[:90].strip() + ".mp3"
        dest_path = os.path.join(music_dir, safe_name)

        try:
            req = urllib.request.Request(stream_url, headers={"User-Agent": "Mozilla/5.0"})
            with urllib.request.urlopen(req, timeout=30) as resp, open(dest_path, "wb") as out:
                out.write(resp.read())
            return {"success": True, "file_path": dest_path, "title": title}
        except Exception:
            # Fallback to yt-dlp direct download
            target_url = track.get("url") or track.get("source_url")
            if target_url:
                try:
                    import yt_dlp
                    ydl_opts = {
                        'format': 'bestaudio/best',
                        'outtmpl': os.path.join(music_dir, f"{safe_name}"),
                        'quiet': True,
                        'no_warnings': True,
                        'noplaylist': True,
                    }
                    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                        ydl.download([target_url])
                    return {"success": True, "file_path": dest_path, "title": title}
                except Exception as ex:
                    return {"success": False, "error": str(ex)}
            return {"success": False, "error": "Не вдалося завантажити трек"}

    def set_mobile_server(self, mobile_server):
        self._mobile_server = mobile_server

    def get_mobile_url(self):
        if self._mobile_server:
            return self._mobile_server.get_mobile_url()
        return "http://127.0.0.1:8088"

    def scan_local_music(self, folder=None):
        """Scans Music folder for local tracks and adds to a dedicated playlist."""
        import os
        import time
        if not folder:
            folder = os.path.join(os.path.expanduser("~"), "Music")
        if not os.path.exists(folder):
            return {"success": False, "count": 0, "tracks": []}

        valid_exts = (".mp3", ".m4a", ".flac", ".wav", ".ogg")
        local_tracks = []
        for root, dirs, files in os.walk(folder):
            for file in files:
                if file.lower().endswith(valid_exts):
                    full_p = os.path.join(root, file)
                    base_name = os.path.splitext(file)[0]
                    parts = base_name.split(" - ", 1)
                    artist = parts[0].strip() if len(parts) > 1 else "Локальний трек"
                    title = parts[1].strip() if len(parts) > 1 else base_name.strip()
                    file_url = "file:///" + full_p.replace("\\", "/")
                    t_id = f"local_{abs(hash(full_p))}"

                    local_tracks.append({
                        "id": t_id,
                        "title": title,
                        "artist": artist,
                        "duration": 0,
                        "duration_formatted": "--:--",
                        "thumbnail": "assets/icon.png",
                        "url": file_url,
                        "source": "local",
                        "is_local": True
                    })
                    if len(local_tracks) >= 300:
                        break
            if len(local_tracks) >= 300:
                break

        if local_tracks:
            playlists = self._library.data.setdefault("playlists", {})
            pl_id = "pl_local_music"
            playlists[pl_id] = {
                "id": pl_id,
                "name": "📁 Локальна музика",
                "created_at": time.time(),
                "tracks": local_tracks,
                "is_auto": True
            }
            self._library._save()

        return {"success": True, "count": len(local_tracks), "playlist_id": "pl_local_music"}

    def get_settings(self):
        settings = self._library.data.setdefault("settings", {
            "hotkeys_enabled": True,
            "tray_enabled": True,
            "discord_enabled": True,
            "hud_enabled": True,
            "volume_norm_enabled": True,
            "offline_download_enabled": True,
            "smart_cache_enabled": True,
            "local_music_enabled": True,
            "lyrics_enabled": True,
            "direct_paste_enabled": True,
            "startup_enabled": False
        })
        if self._desktop:
            settings["startup_enabled"] = self._desktop.is_startup_enabled()
        return settings

    def update_settings(self, new_settings):
        if not isinstance(new_settings, dict):
            return False
        settings = self._library.data.setdefault("settings", {})
        settings.update(new_settings)
        self._library._save()

        if self._desktop:
            if "hotkeys_enabled" in new_settings:
                self._desktop.hotkeys_enabled = bool(new_settings["hotkeys_enabled"])
            if "hud_enabled" in new_settings:
                self._desktop.hud_enabled = bool(new_settings["hud_enabled"])
            if "discord_enabled" in new_settings:
                self._desktop.discord_enabled = bool(new_settings["discord_enabled"])
            if "tray_enabled" in new_settings:
                self._desktop.tray_enabled = bool(new_settings["tray_enabled"])
            if "startup_enabled" in new_settings:
                self._desktop.set_startup(bool(new_settings["startup_enabled"]))

        return True




