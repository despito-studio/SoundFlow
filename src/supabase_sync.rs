//! Синхронізація бібліотеки між пристроями через Supabase.
//!
//! Зберігається один знімок бібліотеки на користувача (таблиця
//! `library_snapshots`), доступ до нього закритий політикою `auth.uid()`.
use crate::supabase::{self, SupabaseConfig};
use serde_json::{json, Value};
use std::time::Duration;

const TABLE: &str = "library_snapshots";

fn context() -> Result<(SupabaseConfig, String, String), String> {
    let session = crate::supabase_auth::current().ok_or("Немає сесії Supabase")?;
    if session.access_token.is_empty() || session.user_id.is_empty() {
        return Err("Немає сесії Supabase".into());
    }
    Ok((
        supabase::current_config(),
        session.access_token,
        session.user_id,
    ))
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

fn endpoint(config: &SupabaseConfig) -> String {
    format!(
        "{}/rest/v1/{TABLE}",
        config.supabase_url.trim_end_matches('/')
    )
}

fn request(
    client: &reqwest::blocking::Client,
    method: reqwest::Method,
    config: &SupabaseConfig,
    token: &str,
    url: String,
) -> reqwest::blocking::RequestBuilder {
    client
        .request(method, url)
        .header("apikey", &config.supabase_anon_key)
        .header("Authorization", format!("Bearer {token}"))
}

/// Завантажує поточний знімок бібліотеки: (JSON, ревізія).
pub fn pull() -> Result<Option<(String, i64)>, String> {
    let (config, token, user) = context()?;
    let client = http()?;
    let url = format!(
        "{}?user_id=eq.{}&select=payload,revision",
        endpoint(&config),
        user
    );
    let response = request(&client, reqwest::Method::GET, &config, &token, url)
        .send()
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let rows: Vec<Value> = response.json().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Supabase: не вдалося прочитати бібліотеку ({status})"));
    }
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let payload = row["payload"].to_string();
    let revision = row["revision"].as_i64().unwrap_or(0);
    Ok(Some((payload, revision)))
}

/// Записує знімок бібліотеки та повертає нову ревізію.
pub fn push(payload: &str) -> Result<i64, String> {
    let (config, token, user) = context()?;
    let client = http()?;
    let payload: Value = serde_json::from_str(payload).map_err(|e| e.to_string())?;

    let list_url = format!("{}?user_id=eq.{}&select=revision", endpoint(&config), user);
    let rows: Vec<Value> = request(&client, reqwest::Method::GET, &config, &token, list_url)
        .send()
        .map_err(|e| e.to_string())?
        .json()
        .unwrap_or_default();
    let revision = rows.first().and_then(|row| row["revision"].as_i64()).unwrap_or(0) + 1;

    let response = request(&client, reqwest::Method::POST, &config, &token, endpoint(&config))
        .header("Prefer", "resolution=merge-duplicates, return=minimal")
        .json(&json!({
            "user_id": user,
            "payload": payload,
            "revision": revision,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        }))
        .send()
        .map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(format!("Supabase: не вдалося зберегти бібліотеку ({status}) {body}"));
    }
    Ok(revision)
}
