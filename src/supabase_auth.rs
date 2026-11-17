//! Supabase Auth sessions. Secrets live in the OS credential store, never JSON files.
use crate::supabase::SupabaseConfig;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub email: String,
    pub expires_at: i64,
}

// Never let an incidental debug log disclose bearer credentials.
impl std::fmt::Debug for AuthSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthSession")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("user_id", &self.user_id)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

struct ScopedSession {
    scope: String,
    session: AuthSession,
}
static SESSION: Mutex<Option<ScopedSession>> = Mutex::new(None);

fn session_file(data_dir: &Path) -> PathBuf {
    data_dir.join("session.json")
}

fn credential_scope(data_dir: &Path) -> Option<String> {
    // Canonicalization prevents relative-path aliases from sharing or losing credentials.
    // The application creates its data directory before initializing auth.
    let path = fs::canonicalize(data_dir).ok()?;
    let digest = Sha256::digest(path.as_os_str().as_encoded_bytes());
    Some(format!("supabase-session-{digest:x}"))
}

fn credential(scope: &str) -> Option<keyring::Entry> {
    keyring::Entry::new("SoundFlow.SupabaseAuth", scope).ok()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Discard legacy plaintext credentials instead of trusting/migrating an editable file.
/// If the OS vault is unavailable, the user signs in again; there is no disk fallback.
pub fn load(data_dir: &Path) {
    let _ = fs::remove_file(session_file(data_dir));
    let loaded = credential_scope(data_dir).and_then(|scope| {
        let text = credential(&scope)?.get_password().ok()?;
        let session: AuthSession = serde_json::from_str(&text).ok()?;
        if !valid_session(&session) {
            return None;
        }
        Some(ScopedSession { scope, session })
    });
    if let Ok(mut guard) = SESSION.lock() {
        *guard = loaded;
    }
}

pub fn current() -> Option<AuthSession> {
    SESSION.lock().ok().and_then(|guard| {
        guard.as_ref().map(|stored| stored.session.clone())
    })
}

fn current_for(data_dir: &Path) -> Option<AuthSession> {
    let scope = credential_scope(data_dir)?;
    SESSION.lock().ok().and_then(|guard| {
        guard.as_ref().filter(|stored| stored.scope == scope)
            .map(|stored| stored.session.clone())
    })
}

/// Preserve the Google API. A vault failure leaves a memory-only session.
pub fn store(data_dir: &Path, session: &AuthSession) {
    let _ = store_session(data_dir, session);
}

fn store_session(data_dir: &Path, session: &AuthSession) -> bool {
    let _ = fs::remove_file(session_file(data_dir));
    if !valid_session(session) {
        return false;
    }
    let Some(scope) = credential_scope(data_dir) else { return false; };
    let persisted = credential(&scope).map(|entry| {
        let saved = serde_json::to_string(session).ok()
            .is_some_and(|text| entry.set_password(&text).is_ok());
        if !saved {
            // Do not silently resurrect the previous user at the next startup.
            let _ = entry.delete_credential();
        }
        saved
    }).unwrap_or(false);
    if let Ok(mut guard) = SESSION.lock() {
        *guard = Some(ScopedSession { scope, session: session.clone() });
    }
    persisted
}

pub fn clear(data_dir: &Path) {
    if let Some(scope) = credential_scope(data_dir) {
        if let Some(entry) = credential(&scope) {
            let _ = entry.delete_credential();
        }
        if let Ok(mut guard) = SESSION.lock() {
            if guard.as_ref().is_some_and(|stored| stored.scope == scope) {
                *guard = None;
            }
        }
    }
    let _ = fs::remove_file(session_file(data_dir));
}

pub fn remote_user_id() -> Option<String> {
    current().map(|session| session.user_id).filter(|id| !id.is_empty())
}

fn valid_session(session: &AuthSession) -> bool {
    let parts: Vec<_> = session.access_token.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| !part.is_empty()
        && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
        && !session.refresh_token.trim().is_empty()
        && uuid::Uuid::parse_str(&session.user_id).is_ok()
        && session.expires_at > 0
}

fn parse_session(body: &Value) -> Result<AuthSession, String> {
    let expires_at = body["expires_at"].as_i64().or_else(|| {
        body["expires_in"].as_i64().filter(|seconds| *seconds > 0)
            .and_then(|seconds| now().checked_add(seconds))
    }).ok_or("Supabase повернув некоректний термін сесії")?;
    let session = AuthSession {
        access_token: body["access_token"].as_str().unwrap_or("").into(),
        refresh_token: body["refresh_token"].as_str().unwrap_or("").into(),
        user_id: body["user"]["id"].as_str().unwrap_or("").into(),
        email: body["user"]["email"].as_str().unwrap_or("").into(),
        expires_at,
    };
    if !valid_session(&session) || expires_at <= now() {
        return Err("Supabase повернув некоректну сесію".into());
    }
    Ok(session)
}

fn validated_base(config: &SupabaseConfig) -> Result<url::Url, String> {
    let url = url::Url::parse(config.supabase_url.trim())
        .map_err(|_| "Некоректна адреса Supabase")?;
    if url.scheme() != "https" || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some()
        || url.query().is_some() || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err("Supabase потребує HTTPS-адресу без шляху, пароля чи параметрів".into());
    }
    let key = config.supabase_anon_key.trim();
    if key.is_empty() || key.bytes().any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        || key.starts_with("sb_secret_")
    {
        return Err("Потрібен публічний ключ Supabase".into());
    }
    Ok(url)
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build().map_err(|_| "Не вдалося створити захищене з’єднання".into())
}

fn post_auth(config: &SupabaseConfig, endpoint: &str, payload: Value) -> Result<Value, String> {
    let url = validated_base(config)?.join(endpoint)
        .map_err(|_| "Некоректна адреса Supabase")?;
    let response = client()?.post(url)
        .header("apikey", config.supabase_anon_key.trim())
        .json(&payload).send().map_err(|_| "Не вдалося з’єднатися з Supabase Auth")?;
    // Never forward untrusted response bodies or request errors containing URLs/secrets.
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            400 | 401 | 403 | 422 => "Вхід або реєстрацію відхилено. Перевірте дані та підтвердження email.",
            429 => "Забагато спроб. Спробуйте пізніше.",
            _ => "Supabase Auth тимчасово недоступний.",
        }.into());
    }
    response.json().map_err(|_| "Supabase повернув некоректну відповідь".into())
}

fn validate_email_password(email: &str, password: &str) -> Result<(), String> {
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    if local.is_empty() || domain.is_empty() || parts.next().is_some()
        || email.len() > 254 || email.chars().any(char::is_whitespace)
        || email.chars().any(char::is_control) {
        return Err("Вкажіть коректну email-адресу".into());
    }
    if password.is_empty() || password.len() > 4096 {
        return Err("Вкажіть пароль (до 4096 байтів)".into());
    }
    Ok(())
}

fn session_summary(session: &AuthSession, persisted: bool) -> Value {
    json!({ "authenticated": true, "user_id": session.user_id, "email": session.email,
        "confirmation_required": false, "session_persisted": persisted })
}

pub fn email_sign_in(
    config: &SupabaseConfig, data_dir: &Path, email: &str, password: &str,
) -> Result<Value, String> {
    let email = email.trim();
    validate_email_password(email, password)?;
    let body = post_auth(config, "auth/v1/token?grant_type=password", json!({
        "email": email, "password": password,
    }))?;
    let session = parse_session(&body)?;
    let persisted = store_session(data_dir, &session);
    Ok(session_summary(&session, persisted))
}

fn signup_summary(body: &Value) -> Result<Value, String> {
    // A successful email-confirmation response has a user but no tokens. Do not
    // treat a malformed token-bearing response as a confirmation-only success.
    if !body["access_token"].is_null() || !body["refresh_token"].is_null() {
        return Err("Supabase повернув некоректну сесію".into());
    }
    let user = body.get("user").filter(|user| user.is_object()).unwrap_or(body);
    if user["id"].as_str().and_then(|id| uuid::Uuid::parse_str(id).ok()).is_none() {
        return Err("Supabase повернув некоректну відповідь реєстрації".into());
    }
    // Avoid exposing identities/account existence from Supabase's anti-enumeration response.
    Ok(json!({ "authenticated": false, "confirmation_required": true }))
}

pub fn email_sign_up(
    config: &SupabaseConfig, data_dir: &Path, name: &str, email: &str, password: &str,
) -> Result<Value, String> {
    let email = email.trim();
    let name = name.trim();
    validate_email_password(email, password)?;
    if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err("Вкажіть ім’я (до 120 символів)".into());
    }
    if password.chars().count() < 8 {
        return Err("Пароль має містити щонайменше 8 символів".into());
    }
    let body = post_auth(config, "auth/v1/signup", json!({
        "email": email, "password": password, "data": { "name": name, "full_name": name },
    }))?;
    if body["access_token"].as_str().is_some_and(|token| !token.is_empty()) {
        let session = parse_session(&body)?;
        let persisted = store_session(data_dir, &session);
        return Ok(session_summary(&session, persisted));
    }
    signup_summary(&body)
}

/// Google API retained; all transports use the same HTTPS/no-redirect policy.
pub fn sign_in_with_id_token(
    config: &SupabaseConfig, data_dir: &Path, id_token: &str,
) -> Result<AuthSession, String> {
    if id_token.trim().is_empty() {
        return Err("Немає даних для входу в Supabase".into());
    }
    let body = post_auth(config, "auth/v1/token?grant_type=id_token", json!({
        "provider": "google", "id_token": id_token,
    }))?;
    let session = parse_session(&body)?;
    store(data_dir, &session);
    Ok(session)
}

pub fn refresh(config: &SupabaseConfig, data_dir: &Path) -> Option<AuthSession> {
    let session = current_for(data_dir)?;
    let body = post_auth(config, "auth/v1/token?grant_type=refresh_token", json!({
        "refresh_token": session.refresh_token,
    })).ok()?;
    let refreshed = parse_session(&body).ok()?;
    if refreshed.user_id != session.user_id {
        return None;
    }
    store(data_dir, &refreshed);
    Some(refreshed)
}

fn verified_admin_response(body: &Value) -> bool {
    // The SQL RPC returns a scalar boolean. Strings, objects, and arrays fail closed.
    body.as_bool() == Some(true)
}

/// Only the authenticated, server-side RPC can grant admin status. User metadata
/// and the locally stored profile are never authorization evidence.
pub fn admin_status(config: &SupabaseConfig, data_dir: &Path) -> Result<Value, String> {
    let base = validated_base(config)?;
    let denied = |authenticated| json!({ "authenticated": authenticated, "is_admin": false, "verified": false });
    let Some(mut session) = current_for(data_dir) else { return Ok(denied(false)); };
    if session.expires_at <= now() + 30 {
        let Some(refreshed) = refresh(config, data_dir) else { return Ok(denied(false)); };
        session = refreshed;
    }
    let url = base.join("rest/v1/rpc/soundflow_is_admin")
        .map_err(|_| "Некоректна адреса Supabase")?;
    let response = client()?.post(url)
        .header("apikey", config.supabase_anon_key.trim())
        .bearer_auth(&session.access_token).json(&json!({})).send();
    let Ok(response) = response else { return Ok(denied(true)); };
    if !response.status().is_success() {
        return Ok(denied(!matches!(response.status().as_u16(), 401 | 403)));
    }
    let Ok(body) = response.json::<Value>() else { return Ok(denied(true)); };
    Ok(json!({ "authenticated": true, "is_admin": verified_admin_response(&body),
        "verified": body.is_boolean() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(url: &str) -> SupabaseConfig {
        SupabaseConfig { supabase_url: url.into(), supabase_anon_key: "sb_publishable_test".into(),
            google_client_id: String::new(), google_client_secret: String::new() }
    }
    fn response() -> Value {
        json!({ "access_token": "header.payload.signature", "refresh_token": "private-refresh",
            "expires_in": 3600, "user": { "id": "550e8400-e29b-41d4-a716-446655440000", "email": "user@example.com" } })
    }

    #[test]
    fn validates_https_origin_and_public_key() {
        assert!(validated_base(&config("https://ieafdutkxpyqwmfedwqy.supabase.co")).is_ok());
        for url in ["http://example.com", "https://user:pass@example.com", "https://example.com/path",
            "https://example.com?x=y", "https://example.com#fragment", "not a url"] {
            assert!(validated_base(&config(url)).is_err());
        }
        let mut cfg = config("https://example.com");
        for key in ["", "sb_secret_private", "key\r\ninjected"] {
            cfg.supabase_anon_key = key.into();
            assert!(validated_base(&cfg).is_err());
        }
    }

    #[test]
    fn rejects_partial_or_expired_sessions_without_echoing_secrets() {
        assert!(parse_session(&response()).is_ok());
        for field in ["access_token", "refresh_token", "user", "expires_in"] {
            let mut body = response();
            body.as_object_mut().unwrap().remove(field);
            let error = parse_session(&body).unwrap_err();
            assert!(!error.contains("private-refresh"));
        }
        let mut expired = response();
        expired["expires_at"] = json!(1);
        assert!(parse_session(&expired).is_err());
        let mut overflow = response();
        overflow["expires_in"] = json!(i64::MAX);
        assert!(parse_session(&overflow).is_err());
    }

    #[test]
    fn safe_summaries_and_debug_never_contain_tokens() {
        let session = parse_session(&response()).unwrap();
        let output = format!("{} {:?}", session_summary(&session, true), session);
        assert!(!output.contains(&session.access_token));
        assert!(!output.contains(&session.refresh_token));
        assert_eq!(session_summary(&session, false)["session_persisted"], false);
    }

    #[test]
    fn confirmation_is_not_a_session() {
        let user = response()["user"].clone();
        for body in [user.clone(), json!({"user": user, "session": null})] {
            let summary = signup_summary(&body).unwrap();
            assert_eq!(summary["confirmation_required"], true);
            assert_eq!(summary["authenticated"], false);
            assert!(summary.get("user_id").is_none());
        }
        assert!(signup_summary(&json!({})).is_err());
        assert!(signup_summary(&response()).is_err());
    }

    #[test]
    fn admin_requires_exact_server_boolean() {
        assert!(verified_admin_response(&json!(true)));
        for body in [json!(false), json!("true"), json!([true]), json!({"is_admin": true}),
            json!({"user_metadata": {"role": "admin"}}), Value::Null] {
            assert!(!verified_admin_response(&body));
        }
    }

    #[test]
    fn validates_email_without_trimming_password() {
        assert!(validate_email_password("user@example.com", " password ").is_ok());
        for email in ["", "a", "@example.com", "a@", "a@@example.com", "a b@example.com", "a\n@example.com"] {
            assert!(validate_email_password(email, "password").is_err());
        }
        assert!(validate_email_password("user@example.com", "").is_err());
    }

    #[test]
    fn vault_scopes_are_distinct_and_stable() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        assert_ne!(credential_scope(first.path()), credential_scope(second.path()));
        assert_eq!(credential_scope(first.path()), credential_scope(&first.path().join(".")));
    }
}
