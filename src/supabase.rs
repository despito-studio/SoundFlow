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
pub const DEFAULT_GOOGLE_CLIENT_ID: &str = "764086051850-6qr4p6gpi6hn506pt8ejuq83di341hur.apps.googleusercontent.com";
pub const DEFAULT_GOOGLE_CLIENT_SECRET: &str = "d-FL95Q19q7MQmFpd7hHD0Ty";
pub const OAUTH_PORT: u16 = 43821;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SupabaseConfig {
    pub supabase_url: String,
    pub supabase_anon_key: String,
    pub google_client_id: String,
    pub google_client_secret: String,
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
            google_client_secret: std::env::var("SOUNDFLOW_GOOGLE_CLIENT_SECRET")
                .unwrap_or_else(|_| DEFAULT_GOOGLE_CLIENT_SECRET.to_string()),
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

                let google_client_secret = val["google_client_secret"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| std::env::var("SOUNDFLOW_GOOGLE_CLIENT_SECRET").ok())
                    .unwrap_or_else(|| DEFAULT_GOOGLE_CLIENT_SECRET.to_string());

                return Self {
                    supabase_url,
                    supabase_anon_key,
                    google_client_id,
                    google_client_secret,
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
        current["google_client_secret"] = json!(self.google_client_secret);

        let parent = config_file.parent().ok_or("Недійсний шлях")?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let serialized = serde_json::to_string_pretty(&current).map_err(|e| e.to_string())?;
        temp.write_all(serialized.as_bytes()).map_err(|e| e.to_string())?;
        temp.persist(&config_file).map_err(|e| e.error.to_string())?;
        Ok(())
    }

    pub fn build_google_oauth_url(&self, redirect_uri: &str, state: &str) -> String {
        let client_id = if !self.google_client_id.trim().is_empty() {
            self.google_client_id.trim()
        } else {
            DEFAULT_GOOGLE_CLIENT_ID
        };
        let encoded_client_id: String = url::form_urlencoded::byte_serialize(client_id.as_bytes()).collect();
        let encoded_redirect: String = url::form_urlencoded::byte_serialize(redirect_uri.as_bytes()).collect();
        let encoded_state: String = url::form_urlencoded::byte_serialize(state.as_bytes()).collect();

        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&access_type=offline&prompt=select_account&state={}",
            encoded_client_id,
            encoded_redirect,
            encoded_state
        )
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
    data_dir: PathBuf,
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
            data_dir: data_dir.to_path_buf(),
        }
    }

    /// Directly generates the Google OAuth 2.0 authorization URL and starts the local loopback listener.
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
        let state = uuid::Uuid::new_v4().simple().to_string();

        // ALWAYS open Google's actual OAuth 2.0 authorization screen directly!
        let auth_url = self.config.build_google_oauth_url(&redirect_uri, &state);

        let (tx, rx) = mpsc::channel();
        let config = self.config.clone();
        let client = self.client.clone();
        let data_dir = self.data_dir.clone();

        thread::spawn(move || {
            run_oauth_listener(listener, tx, config, client, port, redirect_uri, data_dir);
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
            id: if id.is_empty() { format!("usr_{}", uuid::Uuid::new_v4().simple()) } else { format!("usr_google_{id}") },
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

/// Exchanges a Google OAuth 2.0 authorization code for tokens and user profile
pub fn exchange_google_auth_code(
    client: &Client,
    config: &SupabaseConfig,
    code: &str,
    redirect_uri: &str,
) -> CoreResult<GoogleUserSession> {
    let token_endpoint = "https://oauth2.googleapis.com/token";
    let client_id = if !config.google_client_id.trim().is_empty() {
        config.google_client_id.trim()
    } else {
        DEFAULT_GOOGLE_CLIENT_ID
    };
    let client_secret = if !config.google_client_secret.trim().is_empty() {
        config.google_client_secret.trim()
    } else {
        DEFAULT_GOOGLE_CLIENT_SECRET
    };

    let params = vec![
        ("code", code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ];

    let res = client
        .post(token_endpoint)
        .form(&params)
        .send()
        .map_err(|e| format!("Не вдалося надіслати запит обміну токена: {e}"))?;

    let status = res.status();
    let body: Value = res.json().map_err(|_| "Недійсна JSON відповідь від Google Token API")?;

    if !status.is_success() {
        let err_desc = body["error_description"]
            .as_str()
            .or_else(|| body["error"].as_str())
            .unwrap_or("Не вдалося обміняти OAuth код");
        return Err(format!("Google OAuth помилка: {err_desc}"));
    }

    let access_token = body["access_token"].as_str().unwrap_or("");
    let id_token = body["id_token"].as_str().unwrap_or("");

    if !id_token.is_empty() {
        if let Ok(mut session) = verify_google_id_token(client, id_token) {
            session.token = if access_token.is_empty() { id_token.to_string() } else { access_token.to_string() };
            session.provider_token = Some(id_token.to_string());
            return Ok(session);
        }
    }

    if !access_token.is_empty() {
        return SupabaseClient::fetch_user_info(config, client, access_token);
    }

    Err("Google не повернув необхідних токенів авторизації".into())
}

/// Verifies a Google ID token (JWT) via Google's tokeninfo API or internal payload decoding
pub fn verify_google_id_token(client: &Client, id_token: &str) -> CoreResult<GoogleUserSession> {
    // 1. Try Google TokenInfo endpoint
    let tokeninfo_url = format!("https://oauth2.googleapis.com/tokeninfo?id_token={}", id_token);
    if let Ok(res) = client.get(&tokeninfo_url).send() {
        if res.status().is_success() {
            if let Ok(val) = res.json::<Value>() {
                if let Some(email) = val["email"].as_str() {
                    let name = val["name"]
                        .as_str()
                        .unwrap_or_else(|| email.split('@').next().unwrap_or("Користувач"))
                        .to_string();
                    let avatar_url = val["picture"].as_str().unwrap_or("").to_string();
                    let id = val["sub"].as_str().unwrap_or("").to_string();
                    return Ok(GoogleUserSession {
                        id: if id.is_empty() { format!("usr_{}", uuid::Uuid::new_v4().simple()) } else { format!("usr_google_{id}") },
                        email: email.to_string(),
                        name,
                        avatar_url,
                        token: id_token.to_string(),
                        provider_token: Some(id_token.to_string()),
                    });
                }
            }
        }
    }

    // 2. Fallback: decode JWT claims payload
    decode_jwt_claims(id_token)
}

/// Decodes the payload claims of a JWT without network requests
pub fn decode_jwt_claims(token: &str) -> CoreResult<GoogleUserSession> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return Err("Недійсний формат JWT токена".into());
    }
    let payload_b64 = parts[1];
    let decoded_bytes = base64url_decode(payload_b64)
        .map_err(|e| format!("Помилка декодування JWT: {e}"))?;
    let val: Value = serde_json::from_slice(&decoded_bytes)
        .map_err(|e| format!("Недійсний JSON у payload токена: {e}"))?;

    let email = val["email"].as_str().ok_or("Email відсутній у токені Google")?;
    let name = val["name"]
        .as_str()
        .unwrap_or_else(|| email.split('@').next().unwrap_or("Користувач"))
        .to_string();
    let avatar_url = val["picture"].as_str().unwrap_or("").to_string();
    let id = val["sub"].as_str().unwrap_or("").to_string();

    Ok(GoogleUserSession {
        id: if id.is_empty() { format!("usr_{}", uuid::Uuid::new_v4().simple()) } else { format!("usr_google_{id}") },
        email: email.to_string(),
        name,
        avatar_url,
        token: token.to_string(),
        provider_token: Some(token.to_string()),
    })
}

fn base64url_decode(input: &str) -> Result<Vec<u8>, &'static str> {
    let mut standard = input.replace('-', "+").replace('_', "/");
    while standard.len() % 4 != 0 {
        standard.push('=');
    }
    let chars: Vec<u8> = standard.bytes().collect();
    let mut out = Vec::with_capacity(chars.len() * 3 / 4);
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == b'=' {
            break;
        }
        let c1 = b64_val(chars[i])?;
        let c2 = if i + 1 < chars.len() && chars[i + 1] != b'=' { b64_val(chars[i + 1])? } else { 0 };
        let c3 = if i + 2 < chars.len() && chars[i + 2] != b'=' { b64_val(chars[i + 2])? } else { 0 };
        let c4 = if i + 3 < chars.len() && chars[i + 3] != b'=' { b64_val(chars[i + 3])? } else { 0 };

        let b0 = (c1 << 2) | (c2 >> 4);
        out.push(b0);
        if i + 2 < chars.len() && chars[i + 2] != b'=' {
            let b1 = ((c2 & 0x0F) << 4) | (c3 >> 2);
            out.push(b1);
        }
        if i + 3 < chars.len() && chars[i + 3] != b'=' {
            let b2 = ((c3 & 0x03) << 6) | c4;
            out.push(b2);
        }
        i += 4;
    }
    Ok(out)
}

fn b64_val(c: u8) -> Result<u8, &'static str> {
    match c {
        b'A'..=b'Z' => Ok(c - b'A'),
        b'a'..=b'z' => Ok(c - b'a' + 26),
        b'0'..=b'9' => Ok(c - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err("Invalid base64 character"),
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

fn read_http_body(reader: &mut BufReader<TcpStream>) -> Vec<u8> {
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
    let mut body = vec![0u8; content_length.min(131072)];
    let _ = reader.read_exact(&mut body);
    body
}

fn run_oauth_listener(
    listener: TcpListener,
    tx: Sender<CoreResult<GoogleUserSession>>,
    config: SupabaseConfig,
    client: Client,
    _port: u16,
    redirect_uri: String,
    _data_dir: PathBuf,
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
                    // Forward immediately to Google's authorization endpoint
                    let auth_url = config.build_google_oauth_url(&redirect_uri, &uuid::Uuid::new_v4().simple().to_string());
                    let response = format!(
                        "HTTP/1.1 302 Found\r\nLocation: {}\r\nConnection: close\r\n\r\n",
                        auth_url
                    );
                    let _ = reader.get_mut().write_all(response.as_bytes());
                } else if method == "GET" && path.starts_with("/callback") {
                    let query = path.split('?').nth(1).unwrap_or("");
                    let params: std::collections::HashMap<String, String> =
                        url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();

                    if let Some(err) = params.get("error_description").or_else(|| params.get("error")) {
                        send_html_response(reader.get_mut(), false, &format!("Помилка авторизації Google: {err}"), None);
                        let _ = tx.send(Err(err.clone()));
                        return;
                    }

                    if let Some(code) = params.get("code") {
                        match exchange_google_auth_code(&client, &config, code, &redirect_uri) {
                            Ok(session) => {
                                send_html_response(
                                    reader.get_mut(),
                                    true,
                                    &format!("Вітаємо, {}! Google-акаунт успішно підключено.", session.name),
                                    Some(&session.email),
                                );
                                let _ = tx.send(Ok(session));
                                return;
                            }
                            Err(e) => {
                                send_html_response(reader.get_mut(), false, &e, None);
                                let _ = tx.send(Err(e));
                                return;
                            }
                        }
                    }

                    if let Some(token) = params.get("id_token").or_else(|| params.get("access_token")).or_else(|| params.get("token")) {
                        match verify_google_id_token(&client, token).or_else(|_| SupabaseClient::fetch_user_info(&config, &client, token)) {
                            Ok(session) => {
                                send_html_response(
                                    reader.get_mut(),
                                    true,
                                    &format!("Вітаємо, {}! Google-акаунт успішно підключено.", session.name),
                                    Some(&session.email),
                                );
                                let _ = tx.send(Ok(session));
                                return;
                            }
                            Err(e) => {
                                send_html_response(reader.get_mut(), false, &e, None);
                                let _ = tx.send(Err(e));
                                return;
                            }
                        }
                    }

                    // Otherwise, serve HTML page with JavaScript to capture the fragment hash (#access_token=... / #id_token=...)
                    send_hash_catcher_html(reader.get_mut());
                } else if method == "POST" && path.starts_with("/callback-token") {
                    let body = read_http_body(&mut reader);
                    if let Ok(json_body) = serde_json::from_slice::<Value>(&body) {
                        // Check if OAuth code is passed for exchange
                        if let Some(code) = json_body["code"].as_str() {
                            if !code.trim().is_empty() {
                                match exchange_google_auth_code(&client, &config, code.trim(), &redirect_uri) {
                                    Ok(session) => {
                                        send_json_response(reader.get_mut(), json!({
                                            "success": true,
                                            "name": session.name,
                                            "email": session.email
                                        }));
                                        let _ = tx.send(Ok(session));
                                        return;
                                    }
                                    Err(e) => {
                                        send_json_response(reader.get_mut(), json!({ "success": false, "error": e }));
                                        let _ = tx.send(Err(e));
                                        return;
                                    }
                                }
                            }
                        }

                        // Check ID token or access token
                        let id_token = json_body["id_token"].as_str().unwrap_or("");
                        let token = json_body["token"].as_str().unwrap_or(id_token);

                        if !token.is_empty() {
                            match verify_google_id_token(&client, token).or_else(|_| SupabaseClient::fetch_user_info(&config, &client, token)) {
                                Ok(mut session) => {
                                    if let Some(provider_token) = json_body["provider_token"].as_str() {
                                        session.provider_token = Some(provider_token.to_string());
                                    }
                                    send_json_response(reader.get_mut(), json!({
                                        "success": true,
                                        "name": session.name,
                                        "email": session.email
                                    }));
                                    let _ = tx.send(Ok(session));
                                    return;
                                }
                                Err(e) => {
                                    send_json_response(reader.get_mut(), json!({ "success": false, "error": e }));
                                    let _ = tx.send(Err(e));
                                    return;
                                }
                            }
                        }
                    }
                    send_json_response(reader.get_mut(), json!({ "success": false, "error": "Не вказано дійсний токен авторизації" }));
                } else {
                    let _ = reader.get_mut().write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(80));
            }
            Err(_) => {
                thread::sleep(Duration::from_millis(80));
            }
        }
    }

    let _ = tx.send(Err("Час очікування авторизації в браузері минув (120 секунд)".into()));
}

fn send_html_response(stream: &mut TcpStream, success: bool, msg: &str, email: Option<&str>) {
    // Успішний вхід не показує жодної сторінки: вкладка браузера закривається сама,
    // тож користувач одразу повертається у SoundFlow.
    if success {
        send_oauth_done_page(stream);
        return;
    }
    let title = "Помилка авторизації";
    let color = "#ef4444";
    let email_badge = if let Some(e) = email {
        format!(r#"<div style="display:inline-block;padding:6px 14px;background:#1e293b;border:1px solid #334155;border-radius:20px;color:#38bdf8;font-size:14px;margin-bottom:16px;">📧 {}</div>"#, e)
    } else {
        "".to_string()
    };

    let body = format!(
        r#"<!DOCTYPE html>
<html lang="uk">
<head><meta charset="utf-8"><title>{title} — SoundFlow</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; background: #090d16; color: #fff; }}
.card {{ background: #111827; border: 1px solid #1f293d; padding: 40px 48px; border-radius: 20px; box-shadow: 0 20px 50px rgba(0,0,0,0.7); text-align: center; max-width: 460px; }}
h1 {{ color: {color}; margin: 0 0 14px; font-size: 22px; font-weight: 700; }}
p {{ color: #94a3b8; font-size: 15px; margin: 0 0 20px; line-height: 1.6; }}
</style></head>
<body>
<div class="card">
  <h1>{title}</h1>
  {email_badge}
  <p>{msg}</p>
  <p style="font-size:13px;color:#64748b;">Поверніться у SoundFlow і спробуйте ще раз. Цю вкладку можна закрити.</p>
</div>
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

/// Мінімальна службова сторінка після успішного входу: вкладка браузера
/// закривається одразу, без вікна "Авторизація успішна" та без зайвих кліків.
fn send_oauth_done_page(stream: &mut TcpStream) {
    let body = r#"<!DOCTYPE html>
<html lang="uk">
<head><meta charset="utf-8"><meta name="color-scheme" content="dark"><title>SoundFlow</title>
<style>
html, body { margin: 0; height: 100%; background: #090d16; color: #64748b; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }
body { display: flex; align-items: center; justify-content: center; }
#hint { font-size: 14px; opacity: 0; transition: opacity .25s ease; }
body[data-stay="1"] #hint { opacity: 1; }
</style></head>
<body><div id="hint">Готово. Цю вкладку можна закрити.</div>
<script>
(function () {
  var tries = 0;
  function closeTab() {
    try { window.close(); } catch (e) {}
    try { if (window.opener) { window.opener.focus(); } } catch (e) {}
    if (++tries < 3) { setTimeout(closeTab, 120); }
    else { setTimeout(function () { document.body.setAttribute('data-stay', '1'); }, 300); }
  }
  closeTab();
})();
</script>
</body>
</html>"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn send_hash_catcher_html(stream: &mut TcpStream) {
    let body = r#"<!DOCTYPE html>
<html lang="uk">
<head><meta charset="utf-8"><title>SoundFlow — Google OAuth 2.0</title>
<style>
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; background: #090d16; color: #fff; }
.card { background: #111827; border: 1px solid #1f293d; padding: 40px 48px; border-radius: 20px; box-shadow: 0 20px 50px rgba(0,0,0,0.7); text-align: center; max-width: 480px; }
h1 { color: #38bdf8; margin: 0 0 12px; font-size: 22px; }
p { color: #94a3b8; font-size: 14px; margin: 0; line-height: 1.6; }
.spinner { border: 3px solid #1f293d; border-top: 3px solid #38bdf8; border-radius: 50%; width: 36px; height: 36px; animation: spin 0.8s linear infinite; margin: 20px auto; }
@keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
</style></head>
<body>
<div class="card">
  <div id="loader" class="spinner"></div>
  <h1 id="title">Обробка Google OAuth 2.0...</h1>
  <p id="msg">Зчитуємо токен безпеки та перевіряємо обліковий запис у Google Cloud…</p>
</div>
<script>
  try {
    const hash = window.location.hash.substring(1);
    const search = window.location.search.substring(1);
    const params = new URLSearchParams(hash || search);
    const token = params.get('access_token') || params.get('id_token') || params.get('token');
    const code = params.get('code');
    const providerToken = params.get('provider_token') || '';
    const error = params.get('error_description') || params.get('error');

    if (error) {
      document.getElementById('loader').style.display = 'none';
      document.getElementById('title').innerText = 'Помилка авторизації';
      document.getElementById('title').style.color = '#ef4444';
      document.getElementById('msg').innerText = error;
    } else if (token || code) {
      fetch('/callback-token', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token: token, id_token: token, code: code, provider_token: providerToken })
      }).then(r => r.json()).then(res => {
        document.getElementById('loader').style.display = 'none';
        if (res.success) {
          document.getElementById('loader').style.display = 'none';
          document.getElementById('title').innerText = 'Готово';
          document.getElementById('msg').innerText = 'Цю вкладку можна закрити.';
          window.close();
          setTimeout(() => window.close(), 120);
          setTimeout(() => window.close(), 400);
        } else {
          document.getElementById('title').innerText = 'Помилка валідації токена';
          document.getElementById('title').style.color = '#ef4444';
          document.getElementById('msg').innerText = res.error || 'Не вдалося зберегти сеанс Google';
        }
      }).catch(err => {
        document.getElementById('loader').style.display = 'none';
        document.getElementById('title').innerText = 'Помилка зв\'язку з додатком';
        document.getElementById('title').style.color = '#ef4444';
        document.getElementById('msg').innerText = String(err);
      });
    } else {
      document.getElementById('loader').style.display = 'none';
      document.getElementById('title').innerText = '✓ Авторизовано';
      document.getElementById('title').style.color = '#22c55e';
      document.getElementById('msg').innerText = 'Поверніться до вікна SoundFlow.';
      setTimeout(() => window.close(), 1500);
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
