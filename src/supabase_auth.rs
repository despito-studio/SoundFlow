//! Сесія Supabase Auth для десктопного клієнта.
//!
//! Google-вхід виконується локальною loopback-петлею, а отриманий Google
//! `id_token` міняється на сесію Supabase (`grant_type=id_token`). Далі всі
//! запити до PostgREST ідуть з `Authorization: Bearer <access_token>`, тому
//! RLS може перевіряти `auth.uid()`.
use crate::supabase::SupabaseConfig;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub email: String,
    pub expires_at: i64,
}

static SESSION: Mutex<Option<AuthSession>> = Mutex::new(None);

fn session_file(data_dir: &Path) -> PathBuf {
    data_dir.join("session.json")
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Підхоплює збережену сесію під час старту застосунку.
pub fn load(data_dir: &Path) {
    let session = fs::read_to_string(session_file(data_dir))
        .ok()
        .and_then(|text| serde_json::from_str::<AuthSession>(&text).ok());
    if let Ok(mut guard) = SESSION.lock() {
        *guard = session;
    }
}

pub fn current() -> Option<AuthSession> {
    SESSION.lock().ok().and_then(|guard| guard.clone())
}

pub fn store(data_dir: &Path, session: &AuthSession) {
    if let Ok(mut guard) = SESSION.lock() {
        *guard = Some(session.clone());
    }
    if let Ok(text) = serde_json::to_string_pretty(session) {
        let _ = fs::write(session_file(data_dir), text);
    }
}

pub fn clear(data_dir: &Path) {
    if let Ok(mut guard) = SESSION.lock() {
        *guard = None;
    }
    let _ = fs::remove_file(session_file(data_dir));
}

/// Ідентифікатор користувача Supabase (uuid) для записів у PostgREST.
pub fn remote_user_id() -> Option<String> {
    current()
        .map(|session| session.user_id)
        .filter(|id| !id.is_empty())
}

fn parse_session(body: &Value) -> Result<AuthSession, String> {
    let access_token = body["access_token"].as_str().unwrap_or("").to_string();
    if access_token.is_empty() {
        let message = body["error_description"]
            .as_str()
            .or_else(|| body["msg"].as_str())
            .or_else(|| body["error"].as_str())
            .unwrap_or("Supabase не повернув сесію");
        return Err(message.to_string());
    }
    let expires_in = body["expires_in"].as_i64().unwrap_or(3600);
    Ok(AuthSession {
        access_token,
        refresh_token: body["refresh_token"].as_str().unwrap_or("").to_string(),
        user_id: body["user"]["id"].as_str().unwrap_or("").to_string(),
        email: body["user"]["email"].as_str().unwrap_or("").to_string(),
        expires_at: now() + expires_in,
    })
}

/// Міняє Google `id_token` на сесію Supabase.
pub fn sign_in_with_id_token(
    config: &SupabaseConfig,
    data_dir: &Path,
    id_token: &str,
) -> Result<AuthSession, String> {
    if config.supabase_url.trim().is_empty() || id_token.trim().is_empty() {
        return Err("Немає даних для входу в Supabase".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!(
        "{}/auth/v1/token?grant_type=id_token",
        config.supabase_url.trim_end_matches('/')
    );
    let response = client
        .post(&url)
        .header("apikey", &config.supabase_anon_key)
        .header("Content-Type", "application/json")
        .json(&json!({ "provider": "google", "id_token": id_token }))
        .send()
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let body: Value = response.json().unwrap_or(json!({}));
    if !status.is_success() {
        let message = body["error_description"]
            .as_str()
            .or_else(|| body["msg"].as_str())
            .or_else(|| body["error"].as_str())
            .unwrap_or("Supabase Auth недоступний");
        return Err(format!("Supabase Auth: {message} ({status})"));
    }
    let session = parse_session(&body)?;
    store(data_dir, &session);
    Ok(session)
}

/// Оновлює протерміновану сесію refresh-токеном.
pub fn refresh(config: &SupabaseConfig, data_dir: &Path) -> Option<AuthSession> {
    let session = current()?;
    if session.refresh_token.is_empty() {
        return None;
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .ok()?;
    let url = format!(
        "{}/auth/v1/token?grant_type=refresh_token",
        config.supabase_url.trim_end_matches('/')
    );
    let response = client
        .post(&url)
        .header("apikey", &config.supabase_anon_key)
        .header("Content-Type", "application/json")
        .json(&json!({ "refresh_token": session.refresh_token }))
        .send()
        .ok()?;
    let body: Value = response.json().ok()?;
    let refreshed = parse_session(&body).ok()?;
    store(data_dir, &refreshed);
    Some(refreshed)
}
