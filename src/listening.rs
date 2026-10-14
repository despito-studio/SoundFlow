use crate::library::{now, text, CoreResult};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

const PROVIDERS: &[&str] = &["youtube", "soundcloud", "spotify", "local"];

pub struct ListeningStore {
    db: Connection,
}
#[derive(Clone)]
struct Event {
    id: String,
    provider: String,
    track_id: String,
    title: String,
    artist: String,
    thumbnail: String,
    played_at: f64,
    seconds: Option<f64>,
}

fn seconds(value: &Value) -> CoreResult<f64> {
    let seconds = value
        .as_f64()
        .ok_or("Недійсна тривалість прослуховування")?;
    if !seconds.is_finite() || !(0.0..=86400.0).contains(&seconds) {
        return Err("Тривалість повинна бути від 0 до 86400 секунд".into());
    }
    Ok((seconds * 1000.0).round() / 1000.0)
}
fn timestamp(value: &Value) -> CoreResult<f64> {
    let result = if let Some(number) = value.as_f64() {
        if number > 100000000000.0 {
            number / 1000.0
        } else {
            number
        }
    } else if let Some(stamp) = value.as_str() {
        if let Ok(date) = DateTime::parse_from_rfc3339(stamp) {
            date.timestamp_millis() as f64 / 1000.0
        } else {
            let date = NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M")
                .or_else(|_| NaiveDateTime::parse_from_str(stamp, "%Y-%m-%dT%H:%M:%S"))
                .map_err(|_| "Недійсна дата прослуховування")?;
            date.and_utc().timestamp_millis() as f64 / 1000.0
        }
    } else {
        return Err("Вкажіть дату прослуховування".into());
    };
    if !result.is_finite() || result < 0.0 || result > now() + 300.0 {
        return Err("Недійсна дата прослуховування".into());
    }
    Ok(result)
}
fn hash(parts: &Value) -> String {
    format!("{:x}", Sha256::digest(parts.to_string().as_bytes()))
}

impl ListeningStore {
    pub fn open(path: &Path) -> CoreResult<Self> {
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        db.busy_timeout(std::time::Duration::from_secs(15))
            .map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS events (
            user_id TEXT NOT NULL,event_id TEXT NOT NULL,provider TEXT NOT NULL,track_id TEXT NOT NULL,
            title TEXT NOT NULL,artist TEXT NOT NULL,thumbnail TEXT NOT NULL,played_at REAL NOT NULL,
            seconds REAL,origin TEXT NOT NULL,PRIMARY KEY(user_id,event_id));
            CREATE INDEX IF NOT EXISTS events_user_time ON events(user_id,played_at);").map_err(|e|e.to_string())?;
        Ok(Self { db })
    }
    pub fn clear(&self, user: &str) -> CoreResult<Value> {
        let deleted = self
            .db
            .execute("DELETE FROM events WHERE user_id=?", [user])
            .map_err(|e| e.to_string())?;
        Ok(json!({"ok":true,"deleted":deleted}))
    }
    pub fn db_query_count(&self, user: &str) -> CoreResult<i64> {
        let count: i64 = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM events WHERE user_id=?",
                [user],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(count)
    }
    pub fn record(
        &self,
        user: &str,
        track: &Value,
        session: &str,
        heard: &Value,
        started: &Value,
    ) -> CoreResult<Value> {
        if session.is_empty() || session.len() > 200 {
            return Err("Вкажіть ID сесії прослуховування".into());
        }
        let heard = seconds(heard)?;
        let provider = text(track.get("source"), 40).to_lowercase();
        if !PROVIDERS.contains(&provider.as_str()) {
            return Err("Непідтримуване джерело".into());
        }
        let title = text(track.get("title"), 500);
        let artist = text(track.get("artist"), 500);
        let track_id = text(track.get("id"), 500);
        let thumbnail = text(track.get("thumbnail"), 2000);
        if title.is_empty() || track_id.is_empty() {
            return Err("Недійсний трек".into());
        }
        let started = if started.is_null() {
            now()
        } else {
            timestamp(started)?
        };
        let event_id = format!("play:{session}");
        let old = self.db.query_row(
            "SELECT provider,track_id,seconds FROM events WHERE user_id=? AND event_id=?",
            params![user, event_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                ))
            },
        );
        let recorded = match old {
            Ok((old_provider, old_track, old_seconds)) => {
                if old_provider != provider || old_track != track_id {
                    return Err("Сесія прослуховування не може змінити трек".into());
                }
                let heard = heard.max(old_seconds.unwrap_or(0.0));
                self.db
                    .execute(
                        "UPDATE events SET seconds=? WHERE user_id=? AND event_id=?",
                        params![heard, user, event_id],
                    )
                    .map_err(|e| e.to_string())?;
                heard
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                if heard > 0.0 {
                    self.db
                        .execute(
                            "INSERT INTO events VALUES (?,?,?,?,?,?,?,?,?,?)",
                            params![
                                user, event_id, provider, track_id, title, artist, thumbnail,
                                started, heard, "playback"
                            ],
                        )
                        .map_err(|e| e.to_string())?;
                }
                heard
            }
            Err(e) => return Err(e.to_string()),
        };
        Ok(json!({"ok":true,"listened_seconds":recorded}))
    }
    fn imported(provider: &str, row: &Value) -> CoreResult<Option<Event>> {
        if !row.is_object() {
            return Ok(None);
        }
        let (track_id, title, artist, thumbnail, stamp, duration) = match provider {
            "spotify" => {
                let track = &row["track"];
                let title = row
                    .get("master_metadata_track_name")
                    .or_else(|| row.get("trackName"))
                    .or_else(|| track.get("name"));
                let artists = track["artists"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|a| a["name"].as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let artist = row
                    .get("master_metadata_album_artist_name")
                    .or_else(|| row.get("artistName"))
                    .cloned()
                    .unwrap_or(json!(artists));
                let id = row
                    .get("spotify_track_uri")
                    .or_else(|| track.get("uri"))
                    .or_else(|| track.get("id"));
                let stamp = row
                    .get("ts")
                    .or_else(|| row.get("endTime"))
                    .or_else(|| row.get("played_at"))
                    .unwrap_or(&Value::Null)
                    .clone();
                let duration = row
                    .get("ms_played")
                    .or_else(|| row.get("msPlayed"))
                    .map(|value| {
                        seconds(&json!(
                            value.as_f64().ok_or("Недійсна тривалість")? / 1000.0
                        ))
                    })
                    .transpose()?;
                (
                    text(id, 500),
                    text(title, 500),
                    text(Some(&artist), 500),
                    text(track["album"]["images"][0].get("url"), 2000),
                    stamp,
                    duration,
                )
            }
            "youtube" => {
                let raw = text(row.get("titleUrl").or_else(|| row.get("url")), 2000);
                let url = match url::Url::parse(&raw) {
                    Ok(u) => u,
                    Err(_) => return Ok(None),
                };
                if ![
                    "youtube.com",
                    "www.youtube.com",
                    "m.youtube.com",
                    "music.youtube.com",
                    "youtu.be",
                ]
                .contains(&url.host_str().unwrap_or(""))
                {
                    return Ok(None);
                }
                let id = if url.host_str() == Some("youtu.be") {
                    url.path().trim_matches('/').to_string()
                } else {
                    url.query_pairs()
                        .find(|(k, _)| k == "v")
                        .map(|(_, v)| v.into_owned())
                        .unwrap_or_default()
                };
                if id.is_empty() {
                    return Ok(None);
                }
                let title = text(row.get("title"), 500)
                    .trim_start_matches("Watched ")
                    .to_string();
                let artist = text(
                    row["subtitles"][0]
                        .get("name")
                        .or_else(|| row.get("artist")),
                    500,
                );
                let stamp = row
                    .get("time")
                    .or_else(|| row.get("played_at"))
                    .unwrap_or(&Value::Null)
                    .clone();
                let duration = row.get("listened_seconds").map(seconds).transpose()?;
                (id, title, artist, String::new(), stamp, duration)
            }
            "soundcloud" => {
                let track = row.get("track").filter(|t| t.is_object()).unwrap_or(row);
                let id = track
                    .get("id")
                    .map(|id| {
                        id.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| id.to_string())
                    })
                    .unwrap_or_else(|| {
                        text(track.get("permalink_url").or_else(|| track.get("url")), 500)
                    });
                let stamp = row
                    .get("played_at")
                    .or_else(|| row.get("timestamp"))
                    .or_else(|| row.get("time"))
                    .unwrap_or(&Value::Null)
                    .clone();
                let duration = if let Some(value) = row.get("listened_seconds") {
                    Some(seconds(value)?)
                } else {
                    row.get("ms_played")
                        .map(|v| seconds(&json!(v.as_f64().ok_or("Недійсна тривалість")? / 1000.0)))
                        .transpose()?
                };
                (
                    id,
                    text(track.get("title").or_else(|| track.get("name")), 500),
                    text(
                        track
                            .get("artist")
                            .or_else(|| track["user"].get("username")),
                        500,
                    ),
                    text(track.get("artwork_url"), 2000),
                    stamp,
                    duration,
                )
            }
            _ => return Err("Оберіть Spotify, YouTube або SoundCloud".into()),
        };
        if title.is_empty() || stamp.is_null() || duration == Some(0.0) {
            return Ok(None);
        }
        let played_at = timestamp(&stamp)?;
        let track_id = if track_id.is_empty() {
            hash(&json!([title, artist]))
        } else {
            track_id
        };
        let artist = if artist.is_empty() {
            "Unknown artist".into()
        } else {
            artist
        };
        Ok(Some(Event {
            id: format!("import:{}", hash(&json!([provider, track_id, played_at]))),
            provider: provider.into(),
            track_id,
            title,
            artist,
            thumbnail,
            played_at,
            seconds: duration,
        }))
    }
    pub fn import(&mut self, user: &str, provider: &str, raw: &str) -> CoreResult<Value> {
        if !["spotify", "youtube", "soundcloud"].contains(&provider) {
            return Err("Непідтримуване джерело".into());
        }
        if raw.len() > 32 * 1024 * 1024 {
            return Err("Оберіть JSON до 32 МБ".into());
        }
        let payload: Value = serde_json::from_str(raw).map_err(|_| "Недійсний JSON історії")?;
        let rows = payload
            .as_array()
            .or_else(|| {
                ["items", "history", "plays", "collection"]
                    .iter()
                    .find_map(|k| payload[*k].as_array())
            })
            .ok_or("Очікується масив прослуховувань")?;
        if rows.len() > 100000 {
            return Err("Файл містить понад 100000 подій".into());
        }
        let transaction = self.db.transaction().map_err(|e| e.to_string())?;
        let (mut imported, mut skipped, mut duplicates, mut unknown) = (0, 0, 0, 0);
        for row in rows {
            match Self::imported(provider, row) {
                Ok(Some(event)) => {
                    let added = transaction
                        .execute(
                            "INSERT OR IGNORE INTO events VALUES (?,?,?,?,?,?,?,?,?,?)",
                            params![
                                user,
                                event.id,
                                event.provider,
                                event.track_id,
                                event.title,
                                event.artist,
                                event.thumbnail,
                                event.played_at,
                                event.seconds,
                                "import"
                            ],
                        )
                        .map_err(|e| e.to_string())?;
                    if added > 0 {
                        imported += 1;
                        if event.seconds.is_none() {
                            unknown += 1
                        }
                    } else {
                        duplicates += 1
                    }
                }
                _ => skipped += 1,
            }
        }
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(
            json!({"ok":true,"imported":imported,"duplicates":duplicates,"skipped":skipped,"duration_unknown":unknown}),
        )
    }
    pub fn migrate_legacy(&mut self, path: &Path, old_user: &str, user: &str) -> CoreResult<usize> {
        if !path.is_file() {
            return Ok(0);
        }
        // Read-only SQLite keeps legacy data and its original profile untouched.
        let legacy = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| "Не вдалося прочитати попередню статистику")?;
        let mut query=legacy.prepare("SELECT event_id,provider,track_id,title,artist,thumbnail,played_at,seconds FROM events WHERE user_id=? LIMIT 100000").map_err(|_|"Непідтримуваний формат попередньої статистики")?;
        let rows = query
            .query_map([old_user], |row| {
                Ok(Event {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    track_id: row.get(2)?,
                    title: row.get(3)?,
                    artist: row.get(4)?,
                    thumbnail: row.get(5)?,
                    played_at: row.get(6)?,
                    seconds: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let transaction = self.db.transaction().map_err(|e| e.to_string())?;
        let mut added = 0;
        for row in rows {
            let event = row.map_err(|e| e.to_string())?;
            if !PROVIDERS.contains(&event.provider.as_str())
                || event.track_id.is_empty()
                || event.title.is_empty()
                || timestamp(&json!(event.played_at)).is_err()
                || event.seconds.is_some_and(|s| seconds(&json!(s)).is_err())
            {
                continue;
            }
            let id = if event.id.starts_with("import:") {
                // Recompute the canonical native import identity: Python used different
                // JSON spacing for its hash. Re-importing the same export must not add plays.
                format!(
                    "import:{}",
                    hash(&json!([event.provider, event.track_id, event.played_at]))
                )
            } else {
                format!("legacy:{}", hash(&json!([old_user, event.id])))
            };
            added += transaction
                .execute(
                    "INSERT OR IGNORE INTO events VALUES (?,?,?,?,?,?,?,?,?,?)",
                    params![
                        user,
                        id,
                        event.provider,
                        event.track_id,
                        event.title,
                        event.artist,
                        event.thumbnail,
                        event.played_at,
                        event.seconds,
                        "legacy"
                    ],
                )
                .map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(added)
    }
    pub fn summary(&self, user: &str, period: &str, provider: &str) -> CoreResult<Value> {
        if !["7d", "30d", "all"].contains(&period)
            || !(PROVIDERS.contains(&provider) || provider == "all")
        {
            return Err("Недійсний період або джерело".into());
        }
        let days = match period {
            "7d" => 7,
            "30d" => 30,
            _ => 0,
        };
        let today = Utc::now().date_naive();
        let first = today - Duration::days((days - 1).max(0));
        let cutoff = if days > 0 {
            first.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as f64
        } else {
            0.0
        };
        let mut query=self.db.prepare("SELECT provider,track_id,title,artist,thumbnail,played_at,seconds FROM events WHERE user_id=? AND played_at>=? AND played_at<=? AND (?='all' OR provider=?) AND (seconds IS NULL OR seconds>0)").map_err(|e|e.to_string())?;
        let rows = query
            .query_map(params![user, cutoff, now(), provider, provider], |row| {
                Ok(Event {
                    id: String::new(),
                    provider: row.get(0)?,
                    track_id: row.get(1)?,
                    title: row.get(2)?,
                    artist: row.get(3)?,
                    thumbnail: row.get(4)?,
                    played_at: row.get(5)?,
                    seconds: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let (mut tracks, mut artists, mut sources, mut daily): (
            BTreeMap<String, Value>,
            BTreeMap<String, Value>,
            BTreeMap<String, Value>,
            BTreeMap<String, Value>,
        ) = (
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        for offset in 0..days {
            daily.insert(
                (first + Duration::days(offset)).to_string(),
                json!({"seconds":0.0,"plays":0,"unknown_duration_plays":0}),
            );
        }
        let (mut total, mut plays, mut unknown) = (0.0, 0, 0);
        for row in rows {
            let event = row.map_err(|e| e.to_string())?;
            let heard = event.seconds.unwrap_or(0.0);
            let missing = if event.seconds.is_none() { 1 } else { 0 };
            total += heard;
            plays += 1;
            unknown += missing;
            let date = DateTime::from_timestamp(event.played_at as i64, 0)
                .ok_or("Недійсна дата у базі")?
                .date_naive()
                .to_string();
            let track=tracks.entry(format!("{}:{}",event.provider,event.track_id)).or_insert(json!({"id":event.track_id,"title":event.title,"artist":event.artist,"source":event.provider,"thumbnail":event.thumbnail,"seconds":0.0,"plays":0}));
            let artist = artists
                .entry(event.artist.clone())
                .or_insert(json!({"name":event.artist,"seconds":0.0,"plays":0}));
            let source=sources.entry(event.provider.clone()).or_insert(json!({"provider":event.provider,"seconds":0.0,"plays":0,"unknown_duration_plays":0}));
            let day = daily
                .entry(date)
                .or_insert(json!({"seconds":0.0,"plays":0,"unknown_duration_plays":0}));
            for item in [track, artist, source, day] {
                item["seconds"] = json!(item["seconds"].as_f64().unwrap_or(0.0) + heard);
                item["plays"] = json!(item["plays"].as_u64().unwrap_or(0) + 1);
                if item.get("unknown_duration_plays").is_some() {
                    item["unknown_duration_plays"] =
                        json!(item["unknown_duration_plays"].as_u64().unwrap_or(0) + missing);
                }
            }
        }
        fn finish(item: &mut Value) {
            let seconds = item["seconds"].as_f64().unwrap_or(0.0);
            item["seconds"] = json!((seconds * 1000.0).round() / 1000.0);
            item["minutes"] = json!((seconds / 6.0).round() / 10.0);
        }
        for item in tracks
            .values_mut()
            .chain(artists.values_mut())
            .chain(sources.values_mut())
            .chain(daily.values_mut())
        {
            finish(item)
        }
        let rank = |items: BTreeMap<String, Value>| {
            let mut result: Vec<_> = items.into_values().collect();
            result.sort_by(|a, b| {
                b["plays"].as_u64().cmp(&a["plays"].as_u64()).then_with(|| {
                    b["seconds"]
                        .as_f64()
                        .partial_cmp(&a["seconds"].as_f64())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
            result.truncate(20);
            result
        };
        Ok(
            json!({"ok":true,"locked":false,"period":period,"provider":provider,"timezone":"UTC","total_seconds":(total*1000.0).round()/1000.0,"total_minutes":(total/6.0).round()/10.0,
            "plays":plays,"unique_tracks":tracks.len(),"unique_artists":artists.len(),"unknown_duration_plays":unknown,"top_tracks":rank(tracks),"top_artists":rank(artists),"providers":sources.into_values().collect::<Vec<_>>(),
            "daily":daily.into_iter().map(|(date,mut value)|{value["date"]=json!(date);value}).collect::<Vec<_>>()}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track() -> Value {
        json!({"id":"a","title":"Song","artist":"Artist","source":"youtube"})
    }
    #[test]
    fn heartbeats_are_idempotent_and_track_bound() {
        let dir = tempfile::tempdir().unwrap();
        let store = ListeningStore::open(&dir.path().join("stats.sqlite3")).unwrap();
        store
            .record("guest", &track(), "session", &json!(20), &Value::Null)
            .unwrap();
        store
            .record("guest", &track(), "session", &json!(10), &Value::Null)
            .unwrap();
        let result = store.summary("guest", "all", "all").unwrap();
        assert_eq!(result["plays"], 1);
        assert_eq!(result["total_seconds"], 20.0);
        let mut other = track();
        other["id"] = json!("b");
        assert!(store
            .record("guest", &other, "session", &json!(30), &Value::Null)
            .is_err());
    }
    #[test]
    fn imports_do_not_invent_takeout_duration() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ListeningStore::open(&dir.path().join("stats.sqlite3")).unwrap();
        let raw=json!([{"title":"Watched Song","titleUrl":"https://youtube.com/watch?v=a","time":"2024-01-01T12:00:00Z"}]).to_string();
        assert_eq!(
            store.import("guest", "youtube", &raw).unwrap()["duration_unknown"],
            1
        );
        assert_eq!(
            store.import("guest", "youtube", &raw).unwrap()["duplicates"],
            1
        );
        let result = store.summary("guest", "all", "all").unwrap();
        assert_eq!(result["total_seconds"], 0.0);
        assert_eq!(result["unknown_duration_plays"], 1);
        assert_eq!(result["plays"], 1);
    }
    #[test]
    fn legacy_migration_is_read_only_idempotent_and_profile_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let old_path = dir.path().join("old.sqlite3");
        let old = ListeningStore::open(&old_path).unwrap();
        old.record("old-user", &track(), "one", &json!(23), &Value::Null)
            .unwrap();
        old.record("other-user", &track(), "other", &json!(99), &Value::Null)
            .unwrap();
        let mut new = ListeningStore::open(&dir.path().join("new.sqlite3")).unwrap();
        assert_eq!(
            new.migrate_legacy(&old_path, "old-user", "guest").unwrap(),
            1
        );
        assert_eq!(
            new.migrate_legacy(&old_path, "old-user", "guest").unwrap(),
            0
        );
        assert_eq!(
            new.summary("guest", "all", "all").unwrap()["total_seconds"],
            23.0
        );
        assert_eq!(
            old.summary("old-user", "all", "all").unwrap()["total_seconds"],
            23.0
        );
        assert_eq!(
            old.summary("other-user", "all", "all").unwrap()["total_seconds"],
            99.0
        );
    }
    #[test]
    fn reimport_after_python_history_migration_does_not_double_count() {
        let dir = tempfile::tempdir().unwrap();
        let old_path = dir.path().join("old.sqlite3");
        let mut old = ListeningStore::open(&old_path).unwrap();
        let raw=json!([{"master_metadata_track_name":"Song","master_metadata_album_artist_name":"Artist","spotify_track_uri":"spotify:track:1","ts":"2024-01-01T12:00:00Z","ms_played":23000}]).to_string();
        old.import("guest", "spotify", &raw).unwrap();
        old.db
            .execute(
                "UPDATE events SET event_id='import:python-spaced-json-fingerprint'",
                [],
            )
            .unwrap();
        let mut new = ListeningStore::open(&dir.path().join("new.sqlite3")).unwrap();
        assert_eq!(new.migrate_legacy(&old_path, "guest", "guest").unwrap(), 1);
        assert_eq!(
            new.import("guest", "spotify", &raw).unwrap()["duplicates"],
            1
        );
        assert_eq!(new.summary("guest", "all", "all").unwrap()["plays"], 1);
    }
}
