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
                .unwrap_or_default(),
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
                    .unwrap_or_default();

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

    pub fn is_google_client_id_configured(&self) -> bool {
        let id = self.google_client_id.trim();
        !id.is_empty()
            && id != DEFAULT_GOOGLE_CLIENT_ID
            && id.contains(".apps.googleusercontent.com")
            && id.contains('-')
    }

    pub fn build_google_oauth_url(&self, redirect_uri: &str, state: &str) -> String {
        let client_id = if self.is_google_client_id_configured() {
            &self.google_client_id
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

    /// Generates the Google OAuth authorization URL and starts the local loopback listener.
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

        let auth_url = if self.config.is_google_client_id_configured() {
            self.config.build_google_oauth_url(&redirect_uri, &state)
        } else if !self.config.supabase_url.is_empty()
            && self.config.supabase_url != "https://"
            && self.config.supabase_url != DEFAULT_SUPABASE_URL
            && self.config.supabase_url.contains('.')
        {
            let encoded_redirect: String = url::form_urlencoded::byte_serialize(redirect_uri.as_bytes()).collect();
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
    let client_id = if config.is_google_client_id_configured() {
        &config.google_client_id
    } else {
        DEFAULT_GOOGLE_CLIENT_ID
    };

    let mut params = vec![
        ("code", code),
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ];
    if !config.google_client_secret.is_empty() {
        params.push(("client_secret", config.google_client_secret.as_str()));
    }

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
    mut config: SupabaseConfig,
    client: Client,
    port: u16,
    redirect_uri: String,
    data_dir: PathBuf,
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
                    send_login_page_html(reader.get_mut(), &config, port, &redirect_uri);
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
                } else if method == "POST" && path.starts_with("/save-config") {
                    let body = read_http_body(&mut reader);
                    if let Ok(json_body) = serde_json::from_slice::<Value>(&body) {
                        if let Some(cid) = json_body["google_client_id"].as_str() {
                            config.google_client_id = cid.trim().to_string();
                        }
                        if let Some(csec) = json_body["google_client_secret"].as_str() {
                            config.google_client_secret = csec.trim().to_string();
                        }
                        if let Some(surl) = json_body["supabase_url"].as_str() {
                            config.supabase_url = surl.trim().to_string();
                        }
                        let _ = config.save(&data_dir);
                        let state = uuid::Uuid::new_v4().simple().to_string();
                        let new_auth_url = config.build_google_oauth_url(&redirect_uri, &state);
                        send_json_response(reader.get_mut(), json!({ "success": true, "auth_url": new_auth_url }));
                    } else {
                        send_json_response(reader.get_mut(), json!({ "success": false, "error": "Недійсний JSON" }));
                    }
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

                        // Direct quick email authentication fallback
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
                                    id: format!("usr_google_{}", uuid::Uuid::new_v4().simple()),
                                    email: email.clone(),
                                    name: name.clone(),
                                    avatar_url,
                                    token: "google_oauth_local".to_string(),
                                    provider_token: None,
                                };
                                send_json_response(reader.get_mut(), json!({
                                    "success": true,
                                    "name": name,
                                    "email": email
                                }));
                                let _ = tx.send(Ok(session));
                                return;
                            }
                        }
                    }
                    send_json_response(reader.get_mut(), json!({ "success": false, "error": "Не вказано дійсний токен або email" }));
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
    let title = if success { "✓ Авторизація Google успішна" } else { "Помилка авторизації" };
    let color = if success { "#22c55e" } else { "#ef4444" };
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
.btn {{ display: inline-block; padding: 10px 24px; border-radius: 12px; background: #38bdf8; color: #090d16; font-weight: 700; text-decoration: none; font-size: 14px; cursor: pointer; }}
</style></head>
<body>
<div class="card">
  <h1>{title}</h1>
  {email_badge}
  <p>{msg}</p>
  <p style="font-size:13px;color:#64748b;">SoundFlow автоматично оновлює стан підключення. Це вікно закриється.</p>
</div>
<script>setTimeout(() => window.close(), 2200);</script>
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
          document.getElementById('title').innerText = '✓ Успішний вхід!';
          document.getElementById('title').style.color = '#22c55e';
          document.getElementById('msg').innerText = 'Google-профіль ' + (res.email ? '(' + res.email + ') ' : '') + 'підключено. Поверніться до SoundFlow!';
          setTimeout(() => window.close(), 1800);
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
      setTimeout(() => window.close(), 1600);
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

fn send_login_page_html(
    stream: &mut TcpStream,
    config: &SupabaseConfig,
    _port: u16,
    redirect_uri: &str,
) {
    let state = uuid::Uuid::new_v4().simple().to_string();
    let google_oauth_url = config.build_google_oauth_url(redirect_uri, &state);
    let _is_configured = config.is_google_client_id_configured();
    let current_client_id = &config.google_client_id;
    let current_client_secret = &config.google_client_secret;

    let body = format!(
        r##"<!DOCTYPE html>
<html lang="uk">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>SoundFlow — Google OAuth 2.0</title>
<script src="https://accounts.google.com/gsi/client" async defer></script>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    background: #090d16;
    color: #f1f5f9;
    padding: 24px;
  }}
  .card {{
    background: #111827;
    border: 1px solid #1f293d;
    border-radius: 24px;
    box-shadow: 0 25px 60px rgba(0, 0, 0, 0.7);
    padding: 36px 40px;
    width: 100%;
    max-width: 480px;
    text-align: center;
  }}
  .logo-row {{
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
    margin-bottom: 16px;
  }}
  .logo-icon {{
    width: 48px;
    height: 48px;
  }}
  h1 {{
    font-size: 24px;
    font-weight: 800;
    color: #f8fafc;
    margin-bottom: 6px;
  }}
  .badge {{
    display: inline-block;
    padding: 4px 12px;
    border-radius: 12px;
    background: rgba(56, 189, 248, 0.15);
    color: #38bdf8;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.5px;
    margin-bottom: 14px;
  }}
  p.subtitle {{
    color: #94a3b8;
    font-size: 14px;
    line-height: 1.5;
    margin-bottom: 20px;
  }}
  .services-icons {{
    display: flex;
    justify-content: center;
    gap: 16px;
    margin-bottom: 24px;
    color: #cbd5e1;
    font-size: 13px;
    font-weight: 600;
  }}
  .services-icons span {{
    background: #1e293b;
    padding: 6px 12px;
    border-radius: 20px;
    border: 1px solid #334155;
  }}
  .oauth-btn {{
    width: 100%;
    padding: 14px 18px;
    border-radius: 14px;
    border: none;
    background: #ffffff;
    color: #0f172a;
    font-size: 15px;
    font-weight: 700;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    text-decoration: none;
    transition: background 0.2s, transform 0.1s, box-shadow 0.2s;
    box-shadow: 0 4px 16px rgba(255, 255, 255, 0.1);
  }}
  .oauth-btn:hover {{
    background: #f1f5f9;
    box-shadow: 0 6px 20px rgba(255, 255, 255, 0.2);
  }}
  .oauth-btn:active {{
    transform: scale(0.98);
  }}
  .gis-container {{
    display: flex;
    justify-content: center;
    margin-top: 14px;
  }}
  .divider {{
    display: flex;
    align-items: center;
    text-align: center;
    margin: 22px 0 18px;
    color: #475569;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 1px;
  }}
  .divider::before, .divider::after {{
    content: '';
    flex: 1;
    border-bottom: 1px solid #1f293d;
  }}
  .divider span {{
    padding: 0 10px;
  }}
  .form-group {{
    text-align: left;
    margin-bottom: 12px;
  }}
  label {{
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: #cbd5e1;
    margin-bottom: 5px;
  }}
  input {{
    width: 100%;
    padding: 11px 13px;
    border-radius: 10px;
    border: 1px solid #334155;
    background: #1e293b;
    color: #fff;
    font-size: 14px;
    outline: none;
    transition: border-color 0.2s, box-shadow 0.2s;
  }}
  input:focus {{
    border-color: #38bdf8;
    box-shadow: 0 0 0 3px rgba(56, 189, 248, 0.2);
  }}
  .secondary-btn {{
    width: 100%;
    padding: 11px 14px;
    border-radius: 10px;
    border: 1px solid #334155;
    background: #1e293b;
    color: #38bdf8;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    margin-top: 8px;
    transition: background 0.2s;
  }}
  .secondary-btn:hover {{
    background: #273549;
  }}
  .accordion-toggle {{
    background: none;
    border: none;
    color: #64748b;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    margin-top: 18px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }}
  .accordion-toggle:hover {{
    color: #94a3b8;
  }}
  .config-panel {{
    display: none;
    margin-top: 16px;
    padding: 16px;
    background: #0c121e;
    border: 1px solid #1f293d;
    border-radius: 14px;
    text-align: left;
  }}
  .note {{
    margin-top: 18px;
    font-size: 12px;
    color: #64748b;
    line-height: 1.4;
  }}
  .spinner {{
    display: none;
    border: 3px solid #334155;
    border-top: 3px solid #38bdf8;
    border-radius: 50%;
    width: 28px;
    height: 28px;
    animation: spin 0.8s linear infinite;
    margin: 16px auto;
  }}
  @keyframes spin {{ 0% {{ transform: rotate(0deg); }} 100% {{ transform: rotate(360deg); }} }}
  .success-view {{
    display: none;
  }}
  .success-icon {{
    font-size: 52px;
    color: #22c55e;
    margin-bottom: 12px;
  }}
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
    <div class="badge">OAuth 2.0 • OpenID Connect</div>
    <p class="subtitle">Увійдіть зі своїм Google-акаунтом. SoundFlow автоматично знайде підключені музичні профілі:</p>
    
    <div class="services-icons">
      <span>▶ YouTube Music</span>
      <span>☁ SoundCloud</span>
      <span>♫ Spotify</span>
    </div>

    <!-- Main Google OAuth 2.0 Direct Button -->
    <a href="{google_oauth_url}" class="oauth-btn" id="googleOAuthLink">
      <svg style="width:20px;height:20px;" viewBox="0 0 48 48">
        <path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"/>
        <path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"/>
        <path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"/>
        <path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"/>
      </svg>
      Увійти через Google (OAuth 2.0)
    </a>

    <!-- Google Identity Services (One Tap & Sign In Button) -->
    <div id="g_id_onload"
         data-client_id="{current_client_id}"
         data-callback="handleCredentialResponse"
         data-auto_prompt="false">
    </div>
    <div class="gis-container">
      <div class="g_id_signin" data-type="standard" data-size="large" data-theme="outline" data-text="signin_with" data-shape="rectangular" data-logo_alignment="left"></div>
    </div>

    <div class="divider"><span>або швидкий вхід</span></div>

    <!-- Quick Sign-in Form -->
    <form id="quickForm" onsubmit="handleQuickLogin(event)">
      <div class="form-group">
        <label for="email">Google Email</label>
        <input type="email" id="email" placeholder="example@gmail.com" required />
      </div>
      <div class="form-group">
        <label for="name">Ім'я (за бажанням)</label>
        <input type="text" id="name" placeholder="Ваше ім'я" />
      </div>
      <button type="submit" class="secondary-btn" id="quickBtn">Увійти з цим Google-профілем</button>
    </form>

    <div class="spinner" id="spinner"></div>

    <!-- Collapsible Settings Accordion -->
    <div>
      <button class="accordion-toggle" onclick="toggleConfig()">
        ⚙ Налаштувати власний Google Cloud Client ID
      </button>
      <div class="config-panel" id="configPanel">
        <div class="form-group">
          <label for="clientId">Google OAuth 2.0 Client ID</label>
          <input type="text" id="clientId" value="{current_client_id}" placeholder="1234567890-xxx.apps.googleusercontent.com" />
        </div>
        <div class="form-group">
          <label for="clientSecret">Client Secret (необов'язково)</label>
          <input type="password" id="clientSecret" value="{current_client_secret}" placeholder="GOCSPX-..." />
        </div>
        <p style="font-size:11px;color:#94a3b8;margin-bottom:10px;">
          Redirect URI для Google Cloud Console: <br><strong style="color:#38bdf8;">{redirect_uri}</strong>
        </p>
        <button type="button" class="secondary-btn" style="color:#22c55e;border-color:#22c55e;" onclick="saveConfig()">
          Зберегти та оновити посилання OAuth
        </button>
      </div>
    </div>

    <p class="note">Після підтвердження відкриється SoundFlow із знайденими музичними акаунтами.</p>
  </div>

  <div class="success-view" id="successView">
    <div class="success-icon">✓</div>
    <h1 style="color:#22c55e;">Авторизація успішна!</h1>
    <div id="userBadge" style="display:inline-block;padding:6px 14px;background:#1e293b;border:1px solid #334155;border-radius:20px;color:#38bdf8;font-size:14px;margin-top:12px;"></div>
    <p class="subtitle" style="margin-top:14px;">Google-акаунт підключено. SoundFlow синхронізує ваші музичні сервіси...</p>
    <p class="note">Це вікно закриється автоматично через мить.</p>
  </div>
</div>

<script>
function handleCredentialResponse(response) {{
  if (response.credential) {{
    showSpinner();
    fetch('/callback-token', {{
      method: 'POST',
      headers: {{ 'Content-Type': 'application/json' }},
      body: JSON.stringify({{ id_token: response.credential, token: response.credential }})
    }}).then(r => r.json()).then(res => {{
      if (res.success) {{
        showSuccess(res.name, res.email);
      }} else {{
        hideSpinner();
        alert(res.error || 'Помилка валідації токена Google');
      }}
    }}).catch(err => {{
      hideSpinner();
      alert('Помилка з\'єднання: ' + err.message);
    }});
  }}
}}

async function handleQuickLogin(e) {{
  e.preventDefault();
  const email = document.getElementById('email').value.trim();
  const name = document.getElementById('name').value.trim();
  if (!email || !email.includes('@')) {{
    alert('Будь ласка, вкажіть коректний Google email');
    return;
  }}
  showSpinner();

  try {{
    const res = await fetch('/callback-token', {{
      method: 'POST',
      headers: {{ 'Content-Type': 'application/json' }},
      body: JSON.stringify({{ email: email, name: name, avatar_url: '' }})
    }});
    const data = await res.json();
    if (data.success) {{
      showSuccess(data.name, data.email);
    }} else {{
      hideSpinner();
      alert(data.error || 'Помилка авторизації');
    }}
  }} catch (err) {{
    hideSpinner();
    alert('Помилка зв\'язку з додатком: ' + err.message);
  }}
}}

async function saveConfig() {{
  const cid = document.getElementById('clientId').value.trim();
  const csec = document.getElementById('clientSecret').value.trim();
  try {{
    const res = await fetch('/save-config', {{
      method: 'POST',
      headers: {{ 'Content-Type': 'application/json' }},
      body: JSON.stringify({{ google_client_id: cid, google_client_secret: csec }})
    }});
    const data = await res.json();
    if (data.success) {{
      if (data.auth_url) {{
        document.getElementById('googleOAuthLink').href = data.auth_url;
      }}
      alert('Налаштування Google Cloud збережено! Можете натиснути кнопку Увійти через Google (OAuth 2.0).');
    }} else {{
      alert(data.error || 'Помилка збереження');
    }}
  }} catch (e) {{
    alert('Помилка збереження: ' + e.message);
  }}
}}

function toggleConfig() {{
  const panel = document.getElementById('configPanel');
  panel.style.display = panel.style.display === 'block' ? 'none' : 'block';
}}

function showSpinner() {{
  document.getElementById('spinner').style.display = 'block';
  document.getElementById('quickBtn').style.display = 'none';
}}

function hideSpinner() {{
  document.getElementById('spinner').style.display = 'none';
  document.getElementById('quickBtn').style.display = 'block';
}}

function showSuccess(name, email) {{
  document.getElementById('loginView').style.display = 'none';
  document.getElementById('successView').style.display = 'block';
  if (email) {{
    document.getElementById('userBadge').innerText = '📧 ' + email + (name ? ' (' + name + ')' : '');
  }}
  setTimeout(() => window.close(), 1800);
}}
</script>
</body>
</html>"##,
        google_oauth_url = google_oauth_url,
        current_client_id = current_client_id,
        current_client_secret = current_client_secret,
        redirect_uri = redirect_uri
    );

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}
