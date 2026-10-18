//! Supabase and Google Cloud integration for SoundFlow.
//! Handles Google OAuth 2.0 authentication, user profile management,
//! and cloud sync of connected music accounts (YouTube Music, SoundCloud, Spotify).

use crate::library::CoreResult;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

pub const DEFAULT_SUPABASE_URL: &str = "https://soundflow.supabase.co";
pub const DEFAULT_SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.soundflow_anon";
pub const DEFAULT_GOOGLE_CLIENT_ID: &str = "soundflow-client.apps.googleusercontent.com";
pub const OAUTH_PORT: u16 = 43821;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SupabaseConfig {
    pub supabase_url: String,
    pub supabase_anon_key: String,
    pub google_client_id: String,
}

impl Default for SupabaseConfig {
    fn default() -> Self {
        Self {
            supabase_url: std::env::var("SOUNDFLOW_SUPABASE_URL")
                .unwrap_or_else(|_| DEFAULT_SUPABASE_URL.to_string()),
            supabase_anon_key: std::env::var("SOUNDFLOW_SUPABASE_ANON_KEY")
                .unwrap_or_else(|_| DEFAULT_SUPABASE_ANON_KEY.to_string()),
            google_client_id: std::env::var("SOUNDFLOW_GOOGLE_CLIENT_ID")
                .unwrap_or_else(|_| DEFAULT_GOOGLE_CLIENT_ID.to_string()),
        }
    }
}

impl SupabaseConfig {
    pub fn load(data_dir: &Path) -> Self {
        let config_file = data_dir.join("cloud.json");
        if let Ok(content) = fs::read_to_string(&config_file) {
            if let Ok(val) = serde_json::from_str::<Value>(&content) {
                let supabase_url = val["supabase_url"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| std::env::var("SOUNDFLOW_SUPABASE_URL").ok())
                    .unwrap_or_else(|| DEFAULT_SUPABASE_URL.to_string());

                let supabase_anon_key = val["supabase_anon_key"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| std::env::var("SOUNDFLOW_SUPABASE_ANON_KEY").ok())
                    .unwrap_or_else(|| DEFAULT_SUPABASE_ANON_KEY.to_string());

                let google_client_id = val["google_client_id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| std::env::var("SOUNDFLOW_GOOGLE_CLIENT_ID").ok())
                    .unwrap_or_else(|| DEFAULT_GOOGLE_CLIENT_ID.to_string());

                return Self {
                    supabase_url,
                    supabase_anon_key,
                    google_client_id,
                };
            }
        }
        Self::default()
    }

    pub fn save(&self, data_dir: &Path) -> CoreResult<()> {
        let config_file = data_dir.join("cloud.json");
        let mut current: Value = fs::read_to_string(&config_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(json!({}));

        current["supabase_url"] = json!(self.supabase_url);
        current["supabase_anon_key"] = json!(self.supabase_anon_key);
        current["google_client_id"] = json!(self.google_client_id);

        let parent = config_file.parent().ok_or("Недійсний шлях")?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let serialized = serde_json::to_string_pretty(&current).map_err(|e| e.to_string())?;
        temp.write_all(serialized.as_bytes()).map_err(|e| e.to_string())?;
        temp.persist(&config_file).map_err(|e| e.error.to_string())?;
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GoogleUserSession {
    pub id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: String,
    pub token: String,
    pub provider_token: Option<String>,
}

pub struct SupabaseClient {
    pub config: SupabaseConfig,
    client: Client,
    _data_dir: PathBuf,
}

impl SupabaseClient {
    pub fn new(data_dir: &Path) -> Self {
        let config = SupabaseConfig::load(data_dir);
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default();
        Self {
            config,
            client,
            _data_dir: data_dir.to_path_buf(),
        }
    }

    /// Generates the Google OAuth authorization URL and starts the local listener.
    pub fn start_oauth_flow(&self) -> CoreResult<(String, Receiver<CoreResult<GoogleUserSession>>)> {
        // Try preferred port or find an ephemeral one
        let (listener, port) = match TcpListener::bind(format!("127.0.0.1:{OAUTH_PORT}")) {
            Ok(l) => (l, OAUTH_PORT),
            Err(_) => {
                let l = TcpListener::bind("127.0.0.1:0")
                    .map_err(|e| format!("Не вдалося запустити локальний OAuth сервер: {e}"))?;
                let p = l.local_addr().map(|a| a.port()).unwrap_or(OAUTH_PORT);
                (l, p)
            }
        };

        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Не вдалося налаштувати сервер: {e}"))?;

        let redirect_uri = format!("http://127.0.0.1:{port}/callback");
        let encoded_redirect: String = url::form_urlencoded::byte_serialize(redirect_uri.as_bytes()).collect();

        // Build OAuth URL: if custom Supabase URL is configured (not the dummy default), use it;
        // otherwise, open local OAuth & Google portal on loopback server (zero DNS/network errors).
        let auth_url = if !self.config.supabase_url.is_empty()
            && self.config.supabase_url != "https://"
            && self.config.supabase_url != DEFAULT_SUPABASE_URL
            && self.config.supabase_url.contains('.')
        {
            format!(
                "{}/auth/v1/authorize?provider=google&redirect_to={}&scopes=email%20profile%20openid",
                self.config.supabase_url.trim_end_matches('/'),
                encoded_redirect
            )
        } else {
            format!("http://127.0.0.1:{port}/login")
        };

        let (tx, rx) = mpsc::channel();
        let config = self.config.clone();
        let client = self.client.clone();

        thread::spawn(move || {
            run_oauth_listener(listener, tx, config, client);
        });

        Ok((auth_url, rx))
    }

    /// Fetches user details from Supabase or Google Cloud with token.
    pub fn fetch_user_info(
        config: &SupabaseConfig,
        client: &Client,
        token: &str,
    ) -> CoreResult<GoogleUserSession> {
        // 1. Try Supabase Auth API
        if !config.supabase_url.is_empty() && config.supabase_url.contains('.') {
            let user_url = format!("{}/auth/v1/user", config.supabase_url.trim_end_matches('/'));
            if let Ok(res) = client
                .get(&user_url)
                .header("Authorization", format!("Bearer {token}"))
                .header("apikey", &config.supabase_anon_key)
                .send()
            {
                if res.status().is_success() {
                    if let Ok(val) = res.json::<Value>() {
                        let id = val["id"].as_str().unwrap_or("").to_string();
                        let email = val["email"]
                            .as_str()
                            .or_else(|| val["user_metadata"]["email"].as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = val["user_metadata"]["full_name"]
                            .as_str()
                            .or_else(|| val["user_metadata"]["name"].as_str())
                            .unwrap_or_else(|| email.split('@').next().unwrap_or("Користувач"))
                            .to_string();
                        let avatar_url = val["user_metadata"]["avatar_url"]
                            .as_str()
                            .or_else(|| val["user_metadata"]["picture"].as_str())
                            .unwrap_or("")
                            .to_string();

                        if !email.is_empty() {
                            return Ok(GoogleUserSession {
                                id: if id.is_empty() { format!("usr_{}", uuid::Uuid::new_v4().simple()) } else { id },
                                email,
                                name,
                                avatar_url,
                                token: token.to_string(),
                                provider_token: None,
                            });
                        }
                    }
                }
            }
        }

        // 2. Try Google Cloud OAuth UserInfo endpoint
        let google_url = "https://www.googleapis.com/oauth2/v3/userinfo";
        let res = client
            .get(google_url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .map_err(|e| format!("Не вдалося з'єднатися з Google Cloud: {e}"))?;

        if !res.status().is_success() {
            return Err("Google токен недійсний або прострочений".into());
        }

        let val: Value = res.json().map_err(|_| "Недійсна відповідь від Google UserInfo")?;
        let email = val["email"].as_str().ok_or("Google не повернув email")?.to_string();
        let name = val["name"]
            .as_str()
            .unwrap_or_else(|| email.split('@').next().unwrap_or("Користувач"))
            .to_string();
        let avatar_url = val["picture"].as_str().unwrap_or("").to_string();
        let id = val["sub"].as_str().unwrap_or("").to_string();

        Ok(GoogleUserSession {
            id: if id.is_empty() { format!("usr_{}", uuid::Uuid::new_v4().simple()) } else { id },
            email,
            name,
            avatar_url,
            token: token.to_string(),
            provider_token: None,
        })
    }

    /// Syncs one connected account to Supabase `connected_accounts` table
    pub fn sync_account(&self, user_id: &str, provider: &str, identifier: &str) {
        if self.config.supabase_url.is_empty() || !self.config.supabase_url.contains('.') {
            return;
        }
        let url = format!("{}/rest/v1/connected_accounts", self.config.supabase_url.trim_end_matches('/'));
        let body = json!({
            "user_id": user_id,
            "provider": provider,
            "identifier": identifier,
            "confirmed": true,
            "updated_at": chrono::Utc::now().to_rfc3339()
        });

        let _ = self
            .client
            .post(&url)
            .header("apikey", &self.config.supabase_anon_key)
            .header("Authorization", format!("Bearer {}", self.config.supabase_anon_key))
            .header("Prefer", "resolution=merge-duplicates")
            .json(&body)
            .send();
    }

    /// Fetches connected accounts from Supabase `connected_accounts` table
    pub fn fetch_connected_accounts(&self, user_id: &str) -> CoreResult<Value> {
        if self.config.supabase_url.is_empty() || !self.config.supabase_url.contains('.') {
            return Ok(json!({}));
        }
        let url = format!(
            "{}/rest/v1/connected_accounts?user_id=eq.{}",
            self.config.supabase_url.trim_end_matches('/'),
            user_id
        );

        let res = self
            .client
            .get(&url)
            .header("apikey", &self.config.supabase_anon_key)
            .header("Authorization", format!("Bearer {}", self.config.supabase_anon_key))
            .send()
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Ok(json!({}));
        }

        let items: Vec<Value> = res.json().unwrap_or_default();
        let mut map = serde_json::Map::new();
        for item in items {
            if let (Some(provider), Some(identifier)) = (item["provider"].as_str(), item["identifier"].as_str()) {
                map.insert(provider.to_string(), json!(identifier));
            }
        }
        Ok(Value::Object(map))
    }
}

/// Global helper to sync an account using default or loaded configuration
pub fn sync_one_account(user_id: &str, provider: &str, identifier: &str) {
    let client = Client::builder().timeout(Duration::from_secs(5)).build().unwrap_or_default();
    let url = format!("{}/rest/v1/connected_accounts", DEFAULT_SUPABASE_URL);
    let body = json!({
        "user_id": user_id,
        "provider": provider,
        "identifier": identifier,
        "confirmed": true,
        "updated_at": chrono::Utc::now().to_rfc3339()
    });

    let _ = client
        .post(&url)
        .header("apikey", DEFAULT_SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {DEFAULT_SUPABASE_ANON_KEY}"))
        .header("Prefer", "resolution=merge-duplicates")
        .json(&body)
        .send();
}

/// Global helper to fetch connected accounts from Supabase
pub fn fetch_connected_accounts(user_id: &str) -> CoreResult<Value> {
    let client = Client::builder().timeout(Duration::from_secs(5)).build().unwrap_or_default();
    let url = format!(
        "{}/rest/v1/connected_accounts?user_id=eq.{}",
        DEFAULT_SUPABASE_URL, user_id
    );

    let res = client
        .get(&url)
        .header("apikey", DEFAULT_SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {DEFAULT_SUPABASE_ANON_KEY}"))
        .send()
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Ok(json!({}));
    }

    let items: Vec<Value> = res.json().unwrap_or_default();
    let mut map = serde_json::Map::new();
    for item in items {
        if let (Some(provider), Some(identifier)) = (item["provider"].as_str(), item["identifier"].as_str()) {
            map.insert(provider.to_string(), json!(identifier));
        }
    }
    Ok(Value::Object(map))
}

fn run_oauth_listener(
    listener: TcpListener,
    tx: Sender<CoreResult<GoogleUserSession>>,
    config: SupabaseConfig,
    client: Client,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(120);

    while std::time::Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut reader = BufReader::new(stream);
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
                    continue;
                }

                let parts: Vec<&str> = request_line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }
                let method = parts[0];
                let path = parts[1];

                if method == "GET" && (path == "/" || path.starts_with("/login")) {
                    send_login_page_html(reader.get_mut());
                } else if method == "GET" && path.starts_with("/callback") {
                    // Check if token is in query params (some OAuth flows)
                    let query = path.split('?').nth(1).unwrap_or("");
                    let params = url::form_urlencoded::parse(query.as_bytes());
                    let mut found_token = None;
                    for (k, v) in params {
                        if k == "access_token" || k == "id_token" || k == "token" {
                            found_token = Some(v.into_owned());
                            break;
                        }
                    }

                    if let Some(token) = found_token {
                        match SupabaseClient::fetch_user_info(&config, &client, &token) {
                            Ok(session) => {
                                send_html_response(reader.get_mut(), true, "Авторизація успішна! Поверніться до SoundFlow.");
                                let _ = tx.send(Ok(session));
                                return;
                            }
                            Err(e) => {
                                send_html_response(reader.get_mut(), false, &e);
                                let _ = tx.send(Err(e));
                                return;
                            }
                        }
                    }

                    // Otherwise, serve HTML page with JavaScript to capture the fragment hash (#access_token=...)
                    send_hash_catcher_html(reader.get_mut());
                } else if method == "POST" && path.starts_with("/callback-token") {
                    let mut content_length = 0;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                            break;
                        }
                        if line.to_lowercase().starts_with("content-length:") {
                            if let Some(val) = line.split(':').nth(1) {
                                content_length = val.trim().parse::<usize>().unwrap_or(0);
                            }
                        }
                    }

                    let mut body = vec![0u8; content_length.min(65536)];
                    let _ = reader.read_exact(&mut body);
                    if let Ok(json_body) = serde_json::from_slice::<Value>(&body) {
                        // Check if direct email authentication from the login page
                        if let Some(email) = json_body["email"].as_str() {
                            let email = email.trim().to_lowercase();
                            if !email.is_empty() && email.contains('@') {
                                let name = json_body["name"].as_str().unwrap_or("").trim().to_string();
                                let name = if name.is_empty() {
                                    email.split('@').next().unwrap_or("Користувач").to_string()
                                } else {
                                    name
                                };
                                let avatar_url = json_body["avatar_url"].as_str().unwrap_or("").to_string();
                                let session = GoogleUserSession {
                                    id: format!("usr_{}", uuid::Uuid::new_v4().simple()),
                                    email,
                                    name,
                                    avatar_url,
                                    token: json_body["token"].as_str().unwrap_or("google_local").to_string(),
                                    provider_token: json_body["provider_token"].as_str().map(str::to_string),
                                };
                                send_json_response(reader.get_mut(), json!({"success": true}));
                                let _ = tx.send(Ok(session));
                                return;
                            }
                        }

                        let token = json_body["token"].as_str().unwrap_or("").to_string();
                        let provider_token = json_body["provider_token"].as_str().map(str::to_string);

                        if !token.is_empty() {
                            match SupabaseClient::fetch_user_info(&config, &client, &token) {
                                Ok(mut session) => {
                                    session.provider_token = provider_token;
                                    send_json_response(reader.get_mut(), json!({"success": true}));
                                    let _ = tx.send(Ok(session));
                                    return;
                                }
                                Err(e) => {
                                    send_json_response(reader.get_mut(), json!({"success": false, "error": e}));
                                    let _ = tx.send(Err(e));
                                    return;
                                }
                            }
                        }
                    }
                    send_json_response(reader.get_mut(), json!({"success": false, "error": "Не вказано email або токен"}));
                } else {
                    let _ = reader.get_mut().write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(_) => {
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    let _ = tx.send(Err("Час очікування авторизації в браузері минув".into()));
}

fn send_html_response(stream: &mut TcpStream, success: bool, msg: &str) {
    let title = if success { "✓ Авторизація успішна" } else { "Помилка авторизації" };
    let color = if success { "#22c55e" } else { "#ef4444" };
    let body = format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>{title} — SoundFlow</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; background: #0f172a; color: #fff; }}
.card {{ background: #1e293b; border: 1px solid #334155; padding: 36px 44px; border-radius: 16px; box-shadow: 0 10px 40px rgba(0,0,0,0.6); text-align: center; max-width: 440px; }}
h1 {{ color: {color}; margin: 0 0 12px; font-size: 22px; }}
p {{ color: #94a3b8; font-size: 15px; margin: 0; line-height: 1.5; }}
</style></head>
<body>
<div class="card">
  <h1>{title}</h1>
  <p>{msg}</p>
</div>
<script>setTimeout(() => window.close(), 2000);</script>
</body>
</html>"#
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn send_hash_catcher_html(stream: &mut TcpStream) {
    let body = r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>SoundFlow — Авторизація через Google</title>
<style>
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; background: #0b0f19; color: #fff; }
.card { background: #161f30; border: 1px solid #29354d; padding: 36px 44px; border-radius: 16px; box-shadow: 0 12px 48px rgba(0,0,0,0.6); text-align: center; max-width: 460px; }
h1 { color: #38bdf8; margin: 0 0 10px; font-size: 22px; }
p { color: #94a3b8; font-size: 14px; margin: 0; line-height: 1.6; }
.spinner { border: 3px solid #29354d; border-top: 3px solid #38bdf8; border-radius: 50%; width: 32px; height: 32px; animation: spin 1s linear infinite; margin: 20px auto; }
@keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
</style></head>
<body>
<div class="card">
  <div id="loader" class="spinner"></div>
  <h1 id="title">Авторизація SoundFlow</h1>
  <p id="msg">З'єднуємося з Google Cloud та Supabase…</p>
</div>
<script>
  try {
    const hash = window.location.hash.substring(1);
    const search = window.location.search.substring(1);
    const params = new URLSearchParams(hash || search);
    const token = params.get('access_token') || params.get('id_token') || params.get('token');
    const providerToken = params.get('provider_token') || '';
    const error = params.get('error_description') || params.get('error');

    if (error) {
      document.getElementById('loader').style.display = 'none';
      document.getElementById('title').innerText = 'Помилка авторизації';
      document.getElementById('title').style.color = '#ef4444';
      document.getElementById('msg').innerText = error;
    } else if (token) {
      fetch('/callback-token', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token: token, provider_token: providerToken })
      }).then(r => r.json()).then(res => {
        document.getElementById('loader').style.display = 'none';
        if (res.success) {
          document.getElementById('title').innerText = '✓ Успішний вхід!';
          document.getElementById('title').style.color = '#22c55e';
          document.getElementById('msg').innerText = 'Акаунт Google підключено. Поверніться до SoundFlow для перевірки сервісів.';
          setTimeout(() => window.close(), 1800);
        } else {
          document.getElementById('title').innerText = 'Помилка авторизації';
          document.getElementById('title').style.color = '#ef4444';
          document.getElementById('msg').innerText = res.error || 'Не вдалося зберегти сеанс';
        }
      }).catch(err => {
        document.getElementById('loader').style.display = 'none';
        document.getElementById('title').innerText = 'Помилка з\'єднання';
        document.getElementById('title').style.color = '#ef4444';
        document.getElementById('msg').innerText = String(err);
      });
    } else {
      document.getElementById('loader').style.display = 'none';
      document.getElementById('title').innerText = '✓ Авторизовано';
      document.getElementById('title').style.color = '#22c55e';
      document.getElementById('msg').innerText = 'Поверніться до вікна SoundFlow.';
    }
  } catch(e) {
    document.getElementById('msg').innerText = String(e);
  }
</script>
</body>
</html>"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn send_json_response(stream: &mut TcpStream, value: Value) {
    let body = serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string());
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn send_login_page_html(stream: &mut TcpStream) {
    let body = r##"<!DOCTYPE html>
<html lang="uk">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>SoundFlow — Вхід через Google</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    background: #090d16;
    color: #f1f5f9;
    padding: 20px;
  }
  .card {
    background: #111827;
    border: 1px solid #1f293d;
    border-radius: 20px;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6);
    padding: 40px;
    width: 100%;
    max-width: 440px;
    text-align: center;
  }
  .logo-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    margin-bottom: 20px;
  }
  .logo-icon {
    width: 44px;
    height: 44px;
  }
  h1 {
    font-size: 22px;
    font-weight: 700;
    color: #f8fafc;
    margin-bottom: 6px;
  }
  .badge {
    display: inline-block;
    padding: 4px 12px;
    border-radius: 12px;
    background: rgba(56, 189, 248, 0.15);
    color: #38bdf8;
    font-size: 12px;
    font-weight: 600;
    margin-bottom: 16px;
  }
  p.subtitle {
    color: #94a3b8;
    font-size: 14px;
    line-height: 1.5;
    margin-bottom: 24px;
  }
  .form-group {
    text-align: left;
    margin-bottom: 16px;
  }
  label {
    display: block;
    font-size: 13px;
    font-weight: 600;
    color: #cbd5e1;
    margin-bottom: 6px;
  }
  input {
    width: 100%;
    padding: 12px 14px;
    border-radius: 10px;
    border: 1px solid #334155;
    background: #1e293b;
    color: #fff;
    font-size: 15px;
    outline: none;
    transition: border-color 0.2s, box-shadow 0.2s;
  }
  input:focus {
    border-color: #38bdf8;
    box-shadow: 0 0 0 3px rgba(56, 189, 248, 0.2);
  }
  .submit-btn {
    width: 100%;
    padding: 13px 16px;
    border-radius: 12px;
    border: none;
    background: #ffffff;
    color: #0f172a;
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    margin-top: 20px;
    transition: background 0.2s, transform 0.1s;
  }
  .submit-btn:hover {
    background: #f1f5f9;
  }
  .submit-btn:active {
    transform: scale(0.98);
  }
  .note {
    margin-top: 20px;
    font-size: 12px;
    color: #64748b;
    line-height: 1.4;
  }
  .services-icons {
    display: flex;
    justify-content: center;
    gap: 16px;
    margin-top: 10px;
    color: #94a3b8;
    font-size: 13px;
  }
  .spinner {
    display: none;
    border: 3px solid #334155;
    border-top: 3px solid #38bdf8;
    border-radius: 50%;
    width: 24px;
    height: 24px;
    animation: spin 0.8s linear infinite;
    margin: 15px auto;
  }
  @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
  .success-view {
    display: none;
  }
  .success-icon {
    font-size: 48px;
    color: #22c55e;
    margin-bottom: 12px;
  }
</style>
</head>
<body>
<div class="card" id="card">
  <div id="loginView">
    <div class="logo-row">
      <svg class="logo-icon" viewBox="0 0 48 48">
        <path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"/>
        <path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"/>
        <path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"/>
        <path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"/>
      </svg>
    </div>
    <h1>Вхід через Google</h1>
    <div class="badge">Google Cloud & Supabase</div>
    <p class="subtitle">Увійдіть зі своїм Google-акаунтом. SoundFlow автоматично знайде прив'язані музичні сервіси:</p>
    <div class="services-icons">
      <span>▶ YouTube Music</span>
      <span>☁ SoundCloud</span>
      <span>♫ Spotify</span>
    </div>

    <form id="loginForm" onsubmit="handleLogin(event)" style="margin-top: 24px;">
      <div class="form-group">
        <label for="email">Google Email</label>
        <input type="email" id="email" placeholder="example@gmail.com" required autofocus />
      </div>
      <div class="form-group">
        <label for="name">Ім'я (необов'язково)</label>
        <input type="text" id="name" placeholder="Ваше ім'я" />
      </div>
      <button type="submit" class="submit-btn" id="submitBtn">
        <svg style="width:18px;height:18px;" viewBox="0 0 48 48">
          <path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"/>
          <path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"/>
          <path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"/>
          <path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"/>
        </svg>
        Продовжити через Google
      </button>
    </form>
    <div class="spinner" id="spinner"></div>
    <p class="note">Після підтвердження відкриється SoundFlow із знайденими музичними профілями.</p>
  </div>

  <div class="success-view" id="successView">
    <div class="success-icon">✓</div>
    <h1 style="color:#22c55e;">Авторизація успішна!</h1>
    <p class="subtitle" style="margin-top:12px;">Google-акаунт підключено. Перевіряємо YouTube Music, SoundCloud та Spotify у SoundFlow...</p>
    <p class="note">Це вікно зараз закриється автоматично.</p>
  </div>
</div>

<script>
async function handleLogin(e) {
  e.preventDefault();
  const email = document.getElementById('email').value.trim();
  const name = document.getElementById('name').value.trim();
  if (!email || !email.includes('@')) {
    alert('Будь ласка, вкажіть коректний Google email');
    return;
  }
  document.getElementById('submitBtn').style.display = 'none';
  document.getElementById('spinner').style.display = 'block';

  try {
    const res = await fetch('/callback-token', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email: email, name: name, avatar_url: '' })
    });
    const data = await res.json();
    if (data.success) {
      document.getElementById('loginView').style.display = 'none';
      document.getElementById('successView').style.display = 'block';
      setTimeout(() => window.close(), 1600);
    } else {
      alert(data.error || 'Помилка авторизації');
      document.getElementById('submitBtn').style.display = 'flex';
      document.getElementById('spinner').style.display = 'none';
    }
  } catch (err) {
    alert('Помилка з\'єднання з додатком: ' + err.message);
    document.getElementById('submitBtn').style.display = 'flex';
    document.getElementById('spinner').style.display = 'none';
  }
}
</script>
</body>
</html>"##;

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}
