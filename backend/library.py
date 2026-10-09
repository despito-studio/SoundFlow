import json
import os
import time
import uuid
import threading

class LibraryManager:
    """Manages favorites, custom playlists, and playback history saved in data/library.json."""
    def __init__(self, data_path=None):
        self._lock = threading.Lock()
        if data_path:
            self.data_path = data_path
            self.base_dir = os.path.dirname(data_path)
        else:
            appdata = os.environ.get("APPDATA")
            if appdata:
                self.base_dir = os.path.join(appdata, "SoundFlow")
            else:
                self.base_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data")
            os.makedirs(self.base_dir, exist_ok=True)
            self.data_path = os.path.join(self.base_dir, "library.json")
        self.current_user_id = None
        self._ensure_dir()
        self.data = self._load()

    def generate_unique_sf_id(self):
        """Generates a unique User ID like SF-748291 ensuring no collision across existing libraries."""
        import random
        existing_ids = set()
        if os.path.exists(self.base_dir):
            for fname in os.listdir(self.base_dir):
                if fname.startswith("library") and fname.endswith(".json"):
                    try:
                        with open(os.path.join(self.base_dir, fname), "r", encoding="utf-8") as f:
                            d = json.load(f)
                            eid = d.get("profile", {}).get("sf_id")
                            if eid:
                                existing_ids.add(eid)
                    except Exception:
                        pass
        for _ in range(1000):
            cand = f"SF-{random.randint(100000, 999999)}"
            if cand not in existing_ids:
                return cand
        return f"SF-{uuid.uuid4().hex[:6].upper()}"

    def set_user(self, user_id=None, user_info=None):
        """Switches library to a specific user's file or back to guest."""
        self.current_user_id = user_id
        if user_id:
            safe_id = "".join(c for c in str(user_id) if c.isalnum() or c in ("-", "_"))
            self.data_path = os.path.join(self.base_dir, f"library_{safe_id}.json")
        else:
            self.data_path = os.path.join(self.base_dir, "library.json")
        self._ensure_dir()
        self.data = self._load()

        prof = self.data.setdefault("profile", {})
        if "sf_id" not in prof or not prof["sf_id"]:
            prof["sf_id"] = self.generate_unique_sf_id()

        # Strict requirement: all new downloads/users start with listener role
        if "role" not in prof:
            prof["role"] = "listener"

        if user_info and isinstance(user_info, dict):
            if user_info.get("name"):
                prof["name"] = user_info["name"]
            if user_info.get("email"):
                prof["email"] = user_info["email"]
            if user_info.get("avatar_url"):
                prof["avatar_url"] = user_info["avatar_url"]

        self._save()
        return self.data

    def _ensure_dir(self):
        folder = os.path.dirname(self.data_path)
        if folder and not os.path.exists(folder):
            os.makedirs(folder, exist_ok=True)

    def _load(self):
        if os.path.exists(self.data_path):
            try:
                with open(self.data_path, "r", encoding="utf-8") as f:
                    return json.load(f)
            except Exception:
                pass
        return {
            "liked_tracks": [],
            "playlists": {
                "favorites": {
                    "id": "favorites",
                    "name": "Улюблені треки",
                    "created_at": time.time(),
                    "tracks": []
                }
            },
            "history": [],
            "settings": {
                "volume": 0.8,
                "repeat": "off", # off, all, one
                "shuffle": False,
                "preferred_source": "all"
            }
        }

    def _save(self):
        with self._lock:
            for attempt in range(5):
                try:
                    self._ensure_dir()
                    tmp_path = self.data_path + f".tmp.{os.getpid()}"
                    with open(tmp_path, "w", encoding="utf-8") as f:
                        json.dump(self.data, f, ensure_ascii=False, indent=2)
                    os.replace(tmp_path, self.data_path)
                    return True
                except Exception as e:
                    if attempt == 4:
                        print("Error saving library:", e)
                    time.sleep(0.05)
            return False

    def get_all(self):
        return self.data

    def toggle_like(self, track):
        if not track or not track.get("id"):
            return False, self.data["liked_tracks"]
        
        track_id = track["id"]
        liked = self.data.setdefault("liked_tracks", [])
        existing_idx = next((i for i, t in enumerate(liked) if t.get("id") == track_id), -1)

        is_liked = False
        if existing_idx >= 0:
            liked.pop(existing_idx)
            is_liked = False
        else:
            liked.insert(0, track)
            is_liked = True

        self._save()
        return is_liked, liked

    def is_liked(self, track_id):
        liked = self.data.get("liked_tracks", [])
        return any(t.get("id") == track_id for t in liked)

    def create_playlist(self, name):
        if not name or not name.strip():
            name = "Новий плейлист"
        name = name.strip()
        pl_id = "pl_" + str(uuid.uuid4())[:8]
        self.data.setdefault("playlists", {})[pl_id] = {
            "id": pl_id,
            "name": name,
            "created_at": time.time(),
            "tracks": []
        }
        self._save()
        return self.data["playlists"][pl_id]

    def delete_playlist(self, pl_id):
        if pl_id in self.data.get("playlists", {}):
            del self.data["playlists"][pl_id]
            self._save()
            return True
        return False

    def rename_playlist(self, pl_id, new_name):
        if pl_id in self.data.get("playlists", {}) and new_name.strip():
            self.data["playlists"][pl_id]["name"] = new_name.strip()
            self._save()
            return True
        return False

    def add_to_playlist(self, pl_id, track):
        playlists = self.data.setdefault("playlists", {})
        if pl_id in playlists and track and track.get("id"):
            tracks = playlists[pl_id].setdefault("tracks", [])
            # avoid immediate duplicate
            if not any(t.get("id") == track["id"] for t in tracks):
                tracks.append(track)
                self._save()
                return True
        return False

    def remove_from_playlist(self, pl_id, track_id):
        playlists = self.data.setdefault("playlists", {})
        if pl_id in playlists:
            tracks = playlists[pl_id].get("tracks", [])
            initial_len = len(tracks)
            playlists[pl_id]["tracks"] = [t for t in tracks if t.get("id") != track_id]
            if len(playlists[pl_id]["tracks"]) != initial_len:
                self._save()
                return True
        return False

    def add_history(self, track):
        if not track or not track.get("id"):
            return
        history = self.data.setdefault("history", [])
        # Remove previous occurrence to bring to top
        history = [t for t in history if t.get("id") != track["id"]]
        track_with_time = dict(track)
        track_with_time["played_at"] = time.time()
        history.insert(0, track_with_time)
        # Keep last 100
        self.data["history"] = history[:100]
        self._save()

    def clear_history(self):
        self.data["history"] = []
        self._save()
        return True

    def update_settings(self, new_settings):
        if isinstance(new_settings, dict):
            self.data.setdefault("settings", {}).update(new_settings)
            self._save()
            return True
        return False

    def get_profile(self):
        prof = self.data.setdefault("profile", {})
        if "sf_id" not in prof or not prof["sf_id"]:
            prof["sf_id"] = self.generate_unique_sf_id()
            self._save()
        if "role" not in prof:
            prof["role"] = "listener"
            self._save()

        is_admin = (prof.get("role") == "admin")
        default_profile = {
            "sf_id": prof["sf_id"],
            "name": prof.get("name", "Слухач"),
            "role": prof.get("role", "listener"),
            "banner_preset": "beige",
            "banner_url": "",
            "avatar_url": "",
            "followers": 0,
            "following": 0,
            "hq_audio": is_admin,
            "bass_boost": is_admin,
            "bass_level": 6 if is_admin else 0
        }
        for k, v in default_profile.items():
            prof.setdefault(k, v)
        return prof

    def update_profile(self, profile_data):
        if isinstance(profile_data, dict):
            prof = self.get_profile()
            # Prevent changing sf_id directly
            profile_data.pop("sf_id", None)
            # Roles can only be changed via set_role_by_id / Owner Console
            profile_data.pop("role", None)
            prof.update(profile_data)
            self._save()
            return prof
        return self.get_profile()

    def set_role_by_id(self, target_sf_id, new_role):
        """
        Secret command backend: Finds user by sf_id across all library files and sets their role.
        Supported roles: admin, moderator, artist, listener
        """
        valid_roles = ["admin", "moderator", "artist", "listener"]
        target_role = str(new_role).strip().lower()
        if target_role not in valid_roles:
            return {
                "success": False,
                "error": f"Невідома роль '{new_role}'. Доступні: admin, moderator, artist, listener"
            }

        target_sf_id = str(target_sf_id).strip().upper()
        if not target_sf_id.startswith("SF-") and target_sf_id.isalnum():
            target_sf_id = f"SF-{target_sf_id}"

        # 1. Check currently active profile
        cur_prof = self.get_profile()
        if cur_prof.get("sf_id") == target_sf_id:
            cur_prof["role"] = target_role
            if target_role == "admin":
                cur_prof["hq_audio"] = True
                cur_prof["bass_boost"] = True
                cur_prof["bass_level"] = 6
            self._save()
            return {
                "success": True,
                "sf_id": target_sf_id,
                "role": target_role,
                "name": cur_prof.get("name", "Користувач"),
                "is_current": True
            }

        # 2. Search all library*.json files in base_dir
        found = False
        target_name = ""
        if os.path.exists(self.base_dir):
            for fname in os.listdir(self.base_dir):
                if fname.startswith("library") and fname.endswith(".json"):
                    fpath = os.path.join(self.base_dir, fname)
                    try:
                        with open(fpath, "r", encoding="utf-8") as f:
                            file_data = json.load(f)
                        f_prof = file_data.get("profile", {})
                        if f_prof.get("sf_id") == target_sf_id:
                            f_prof["role"] = target_role
                            if target_role == "admin":
                                f_prof["hq_audio"] = True
                                f_prof["bass_boost"] = True
                                f_prof["bass_level"] = 6
                            with open(fpath, "w", encoding="utf-8") as f:
                                json.dump(file_data, f, ensure_ascii=False, indent=2)
                            found = True
                            target_name = f_prof.get("name", "Користувач")
                            break
                    except Exception as e:
                        print(f"Error checking {fname}: {e}")

        if found:
            return {
                "success": True,
                "sf_id": target_sf_id,
                "role": target_role,
                "name": target_name,
                "is_current": False
            }

        return {
            "success": False,
            "error": f"Користувача з ID '{target_sf_id}' не знайдено."
        }

    def get_all_users(self):
        """Scans all library files and returns list of registered users with their IDs, names, and roles."""
        users = []
        seen_ids = set()
        if os.path.exists(self.base_dir):
            for fname in sorted(os.listdir(self.base_dir)):
                if fname.startswith("library") and fname.endswith(".json"):
                    fpath = os.path.join(self.base_dir, fname)
                    try:
                        with open(fpath, "r", encoding="utf-8") as f:
                            d = json.load(f)
                        p = d.setdefault("profile", {})
                        sid = p.get("sf_id")
                        if not sid:
                            sid = self.generate_unique_sf_id()
                            p["sf_id"] = sid
                            if "role" not in p:
                                p["role"] = "listener"
                            try:
                                with open(fpath, "w", encoding="utf-8") as fw:
                                    json.dump(d, fw, ensure_ascii=False, indent=2)
                            except Exception:
                                pass
                        if sid and sid not in seen_ids:
                            seen_ids.add(sid)
                            users.append({
                                "sf_id": sid,
                                "name": p.get("name", "Слухач"),
                                "role": p.get("role", "listener"),
                                "email": p.get("email", ""),
                                "is_guest": (fname == "library.json")
                            })
                    except Exception:
                        pass
        # Ensure current active user is in the list
        cur_p = self.get_profile()
        if cur_p.get("sf_id") and cur_p["sf_id"] not in seen_ids:
            users.insert(0, {
                "sf_id": cur_p["sf_id"],
                "name": cur_p.get("name", "Слухач"),
                "role": cur_p.get("role", "listener"),
                "email": cur_p.get("email", ""),
                "is_guest": (self.current_user_id is None)
            })
        return users

    def get_friends(self):
        return self.data.setdefault("friends", [])

    def add_friend(self, friend_data):
        friends = self.get_friends()
        if isinstance(friend_data, str):
            name = friend_data.strip()
            if not name:
                return False, "Ім'я не може бути порожнім"
            if any(f.get("name", "").lower() == name.lower() for f in friends):
                return False, "Користувач вже у ваших друзях"
            safe_id = "".join(c for c in name.lower() if c.isalnum()) or str(uuid.uuid4())[:8]
            new_friend = {
                "id": f"friend_{safe_id}",
                "name": name,
                "avatar_url": f"https://api.dicebear.com/7.x/bottts/svg?seed={name}",
                "role": "listener",
                "status": "online",
                "current_track": "SoundFlow Chill Mix",
                "added_at": time.time()
            }
            friends.append(new_friend)
            self._save()
            return True, new_friend
        elif isinstance(friend_data, dict):
            name = friend_data.get("name", "").strip()
            if any(f.get("name", "").lower() == name.lower() for f in friends):
                return False, "Користувач вже у ваших друзях"
            friend_data.setdefault("id", str(uuid.uuid4()))
            friend_data.setdefault("added_at", time.time())
            friends.append(friend_data)
            self._save()
            return True, friend_data
        return False, "Некоректні дані"

    def remove_friend(self, friend_id):
        friends = self.get_friends()
        initial_len = len(friends)
        self.data["friends"] = [f for f in friends if f.get("id") != friend_id and f.get("name") != friend_id]
        if len(self.data["friends"]) != initial_len:
            self._save()
            return True
        return False

    def clear_cache(self):
        import shutil
        cache_dir = os.path.join(self.base_dir, "cache")
        if os.path.exists(cache_dir):
            shutil.rmtree(cache_dir, ignore_errors=True)
        return True

    def export_library(self):
        return json.dumps(self.data, ensure_ascii=False, indent=2)

    def import_library(self, json_str):
        try:
            new_data = json.loads(json_str)
            if isinstance(new_data, dict) and "liked_tracks" in new_data:
                self.data = new_data
                self._save()
                return True
        except Exception:
            pass
        return False
