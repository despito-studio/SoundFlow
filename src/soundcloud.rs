//! Імпорт публічних плейлистів SoundCloud із профілю.
//!
//! Музичний рушій розбирає окремі пісні та плейлисти за посиланням, але профіль
//! він обробляє лише як набір треків. Тут ми напряму питаємо публічний API
//! SoundCloud, щоб отримати всі `sets` профілю й зберегти їх як плейлисти.
use serde_json::{json, Value};
use std::{sync::OnceLock, time::Duration};

static CLIENT_ID: OnceLock<String> = OnceLock::new();

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36")
        .build()
        .map_err(|e| e.to_string())
}

/// SoundCloud вбудовує client_id у свої JS-бандли; витягуємо перший-ліпший.
fn find_client_id(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut cursor = 0;
    while cursor + 9 < bytes.len() {
        let rest = text.get(cursor..)?;
        let position = rest.find("client_id")?;
        let mut index = cursor + position + "client_id".len();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index < bytes.len() && (bytes[index] == b':' || bytes[index] == b'=') {
            index += 1;
        }
        while index < bytes.len() && (bytes[index] == b'"' || bytes[index] == b'\'') {
            index += 1;
        }
        let end = index + 32;
        if end <= bytes.len() && bytes[index..end].iter().all(u8::is_ascii_alphanumeric) {
            return Some(String::from_utf8_lossy(&bytes[index..end]).to_string());
        }
        cursor = cursor + position + "client_id".len();
    }
    None
}

pub fn client_id() -> Result<String, String> {
    if let Some(id) = CLIENT_ID.get() {
        return Ok(id.clone());
    }
    let client = http()?;
    let page = client
        .get("https://soundcloud.com/")
        .send()
        .map_err(|e| format!("SoundCloud недоступний: {e}"))?
        .text()
        .map_err(|e| e.to_string())?;
    let mut scripts: Vec<String> = Vec::new();
    for chunk in page.split("src=\"").skip(1) {
        let Some(end) = chunk.find('"') else { continue };
        let src = &chunk[..end];
        if src.contains("sndcdn") && src.ends_with(".js") {
            scripts.push(if src.starts_with("http") {
                src.to_string()
            } else {
                format!("https://soundcloud.com{src}")
            });
        }
        if scripts.len() >= 12 {
            break;
        }
    }
    for src in scripts {
        let Ok(response) = client.get(&src).send() else { continue };
        let Ok(js) = response.text() else { continue };
        if let Some(id) = find_client_id(&js) {
            let _ = CLIENT_ID.set(id.clone());
            return Ok(id);
        }
    }
    Err("Не вдалося отримати доступ до публічного API SoundCloud".into())
}

fn playlist_track(raw: &Value) -> Option<Value> {
    let track_id = raw["id"].as_i64()?;
    let url = raw["permalink_url"].as_str()?;
    if url.is_empty() {
        return None;
    }
    let mut duration = raw["duration"].as_f64().unwrap_or(0.0);
    if duration > 10_000.0 {
        duration /= 1000.0;
    }
    let thumbnail = raw["artwork_url"]
        .as_str()
        .or_else(|| raw["user"]["avatar_url"].as_str())
        .unwrap_or("")
        .replace("-large.", "-t500x500.");
    Some(json!({
        "id": format!("sc_{track_id}"),
        "title": raw["title"].as_str().unwrap_or("Без назви"),
        "artist": raw["user"]["username"].as_str().unwrap_or("SoundCloud"),
        "duration": duration,
        "duration_formatted": format!("{}:{:02}", (duration / 60.0) as u64, duration as u64 % 60),
        "thumbnail": thumbnail,
        "url": url,
        "source": "soundcloud",
        "source_url": url,
        "downloadable": raw["policy"].as_str() == Some("ALLOW"),
        "is_live": false,
    }))
}

fn has_cyrillic(value: &str) -> bool {
    value.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

fn search_tracks(client: &reqwest::blocking::Client, id: &str, query: &str, limit: u32) -> Vec<Value> {
    let Ok(response) = client
        .get("https://api-v2.soundcloud.com/search/tracks")
        .query(&[("q", query), ("limit", &limit.to_string()), ("client_id", id)])
        .send()
    else {
        return Vec::new();
    };
    let Ok(page) = response.json::<Value>() else {
        return Vec::new();
    };
    page["collection"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(playlist_track)
        .collect()
}

/// Схожі треки, коли вбудований рушій не дав рекомендацій.
/// Орієнтуємось на виконавця й назву поточного треку та тримаємо ту саму мову.
pub fn related_tracks(context: &Value, limit: usize) -> Vec<Value> {
    let Ok(client) = http() else { return Vec::new() };
    let Ok(id) = client_id() else { return Vec::new() };
    let seed = context["current_track"].clone();
    let title = seed["title"].as_str().unwrap_or("").to_string();
    let artist = seed["artist"].as_str().unwrap_or("").to_string();
    let seed_id = seed["id"].as_str().unwrap_or("").to_string();
    let seed_cyrillic = has_cyrillic(&title) || has_cyrillic(&artist);

    let mut candidates: Vec<Value> = Vec::new();
    if !artist.is_empty() {
        candidates.extend(search_tracks(&client, &id, &artist, 30));
    }
    if !title.is_empty() {
        candidates.extend(search_tracks(&client, &id, &title, 15));
    }

    let mut seen = std::collections::HashSet::new();
    let mut matched: Vec<Value> = Vec::new();
    let mut rest: Vec<Value> = Vec::new();
    for track in candidates {
        let track_id = track["id"].as_str().unwrap_or("").to_string();
        if track_id.is_empty() || track_id == seed_id || !seen.insert(track_id) {
            continue;
        }
        let same_script = (has_cyrillic(track["title"].as_str().unwrap_or(""))
            || has_cyrillic(track["artist"].as_str().unwrap_or("")))
            == seed_cyrillic;
        if same_script {
            matched.push(track);
        } else {
            rest.push(track);
        }
    }
    matched.extend(rest);
    matched.truncate(limit.max(1));
    matched
}

/// Один публічний плейлист за посиланням: (назва, треки).
pub fn playlist_tracks(url: &str) -> Result<(String, Vec<Value>), String> {
    let client = http()?;
    let id = client_id()?;
    let resolved: Value = client
        .get("https://api-v2.soundcloud.com/resolve")
        .query(&[("url", url), ("client_id", id.as_str())])
        .send()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    if resolved["kind"].as_str() != Some("playlist") {
        return Err("Це не плейлист SoundCloud".into());
    }
    let title = resolved["title"].as_str().unwrap_or("SoundCloud").trim().to_string();
    let tracks: Vec<Value> = resolved["tracks"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(playlist_track)
        .collect();
    if tracks.is_empty() {
        return Err("У плейлисті немає доступних треків".into());
    }
    Ok((title, tracks))
}

/// Повертає плейлисти профілю: (назва, треки).
pub fn profile_playlists(profile_url: &str) -> Result<Vec<(String, Vec<Value>)>, String> {
    let client = http()?;
    let id = client_id()?;
    let resolved: Value = client
        .get("https://api-v2.soundcloud.com/resolve")
        .query(&[("url", profile_url), ("client_id", id.as_str())])
        .send()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    if resolved["kind"].as_str() != Some("user") {
        return Err("Це не профіль SoundCloud".into());
    }
    let Some(user_id) = resolved["id"].as_i64() else {
        return Err("SoundCloud не повернув профіль".into());
    };
    let mut playlists: Vec<(String, Vec<Value>)> = Vec::new();
    let mut next = Some(format!(
        "https://api-v2.soundcloud.com/users/{user_id}/playlists?limit=50&client_id={id}"
    ));
    let mut page_index = 0;
    while let Some(url) = next.take() {
        if page_index >= 4 {
            break;
        }
        page_index += 1;
        let page: Value = client
            .get(&url)
            .send()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;
        for playlist in page["collection"].as_array().cloned().unwrap_or_default() {
            let title = playlist["title"].as_str().unwrap_or("SoundCloud").trim().to_string();
            let tracks: Vec<Value> = playlist["tracks"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(playlist_track)
                .collect();
            if !tracks.is_empty() {
                playlists.push((title, tracks));
            }
        }
        next = page["next_href"].as_str().map(|href| {
            if href.contains("client_id=") {
                href.to_string()
            } else {
                format!("{href}&client_id={id}")
            }
        });
    }
    Ok(playlists)
}

/// Чи це посилання на профіль (один сегмент шляху), а не трек/плейлист.
pub fn is_profile_url(value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else { return false };
    if !matches!(url.host_str(), Some("soundcloud.com" | "www.soundcloud.com" | "m.soundcloud.com")) {
        return false;
    }
    url.path_segments()
        .map(|segments| segments.filter(|part| !part.is_empty()).count() == 1)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires network access to soundcloud.com"]
    fn profile_playlists_are_fetched() {
        let playlists = profile_playlists("https://soundcloud.com/ruraris").expect("fetch playlists");
        assert!(!playlists.is_empty(), "expected at least one playlist");
        let total: usize = playlists.iter().map(|(_, tracks)| tracks.len()).sum();
        assert!(total > 0, "expected tracks");
        for (name, tracks) in &playlists {
            assert!(!name.is_empty());
            assert!(tracks.iter().all(|track| track["source"] == "soundcloud"));
        }
    }

    #[test]
    #[ignore = "requires network access to soundcloud.com"]
    fn related_tracks_keep_the_language() {
        let seed = json!({
            "id": "yt_seed",
            "title": "Официантка",
            "artist": "Папин Олимпос",
            "source": "youtube"
        });
        let tracks = related_tracks(&json!({ "current_track": seed }), 10);
        assert!(!tracks.is_empty(), "expected related tracks");
        let cyrillic = tracks
            .iter()
            .filter(|track| {
                has_cyrillic(track["title"].as_str().unwrap_or(""))
                    || has_cyrillic(track["artist"].as_str().unwrap_or(""))
            })
            .count();
        assert!(
            cyrillic * 2 >= tracks.len(),
            "expected mostly cyrillic tracks, got {cyrillic}/{}",
            tracks.len()
        );
    }

    #[test]
    fn profile_urls_are_detected() {
        assert!(is_profile_url("https://soundcloud.com/ruraris"));
        assert!(!is_profile_url("https://soundcloud.com/ruraris/sets/the-cradle-of-thought"));
        assert!(!is_profile_url("https://soundcloud.com/ruraris/some-track"));
        assert!(!is_profile_url("https://example.com/ruraris"));
    }
}
