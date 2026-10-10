use crate::library::{now, CoreResult};
use reqwest::{blocking::Client, Method};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    time::Duration,
};
use url::Url;

pub struct CloudClient {
    base: String,
    config_path: PathBuf,
    client: Client,
    user: Option<Value>,
    token: Option<String>,
    revision: Option<String>,
}

fn validate_base(base: &str) -> CoreResult<Url> {
    let parsed = Url::parse(base).map_err(|_| "Вкажіть HTTPS адресу сервера")?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
        || parsed.port().is_some_and(|p| p != 443)
    {
        return Err("Сервер повинен використовувати HTTPS без облікових даних у URL".into());
    }
    Ok(parsed)
}
fn token_valid(token: &str) -> bool {
    !token.is_empty() && token.len() <= 16384 && !token.contains(['\r', '\n'])
}
fn sanitized_user(value: &Value) -> CoreResult<Value> {
    let id = value["id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 200)
        .ok_or("Сервер не повернув ID користувача")?;
    let mut user = json!({"id":id});
    for field in ["name", "email", "avatar_url"] {
        if let Some(text) = value[field].as_str() {
            user[field] = json!(text
                .chars()
                .take(if field == "avatar_url" { 2000 } else { 254 })
                .collect::<String>());
        }
    }
    Ok(user)
}
fn credential(base: &str) -> CoreResult<keyring::Entry> {
    keyring::Entry::new("SoundFlow", base)
        .map_err(|_| "Системне сховище облікових даних недоступне".into())
}

impl CloudClient {
    pub fn open(data_dir: &std::path::Path, offline: bool) -> CoreResult<Self> {
        let config_path = data_dir.join("cloud.json");
        let file: Value = fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null);
        let base = std::env::var("SOUNDFLOW_CLOUD_URL")
            .ok()
            .unwrap_or_else(|| file["base_url"].as_str().unwrap_or("").into())
            .trim_end_matches('/')
            .to_string();
        let base = if offline || validate_base(&base).is_err() {
            String::new()
        } else {
            base
        };
        let token = if base.is_empty() {
            None
        } else {
            credential(&base)
                .ok()
                .and_then(|e| e.get_password().ok())
                .filter(|t| token_valid(t))
        };
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base,
            config_path,
            client,
            user: None,
            token,
            revision: None,
        })
    }
    pub fn status(&self) -> Value {
        json!({"configured":!self.base.is_empty(),"base_url":self.base,"signed_in":self.user.is_some(),"user":self.user,"sync_revision":self.revision})
    }
    pub fn user_id(&self) -> Option<String> {
        self.user
            .as_ref()
            .and_then(|u| u["id"].as_str())
            .map(str::to_owned)
    }
    pub fn forget_revision(&mut self) {
        self.revision = None;
    }
    pub fn invalidate_session(&mut self) {
        self.user = None;
        self.token = None;
        self.revision = None;
    }
    pub fn configure(&mut self, payload: &Value) -> CoreResult<Value> {
        let base = payload["base_url"]
            .as_str()
            .unwrap_or("")
            .trim()
            .trim_end_matches('/')
            .to_string();
        if !base.is_empty() {
            validate_base(&base)?;
        }
        if self.base != base {
            self.user = None;
            self.token = None;
            self.revision = None;
        }
        let mut temp = tempfile::NamedTempFile::new_in(
            self.config_path
                .parent()
                .ok_or("Недійсний шлях конфігурації")?,
        )
        .map_err(|e| e.to_string())?;
        write!(temp, "{}", json!({"base_url":base})).map_err(|e| e.to_string())?;
        temp.persist(&self.config_path)
            .map_err(|e| e.error.to_string())?;
        self.base = base;
        if !self.base.is_empty() {
            self.token = credential(&self.base)
                .ok()
                .and_then(|e| e.get_password().ok())
                .filter(|t| token_valid(t));
        }
        Ok(self.status())
    }
    fn request(
        &self,
        method: Method,
        path: &str,
        payload: Option<&Value>,
        authenticated: bool,
        revision: Option<&str>,
    ) -> CoreResult<Value> {
        if self.base.is_empty() {
            return Err("Хмарний сервер SoundFlow ще не налаштовано".into());
        }
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base))
            .header("Accept", "application/json");
        if authenticated {
            let token = self
                .token
                .as_deref()
                .filter(|t| token_valid(t))
                .ok_or("Спочатку увійдіть у профіль")?;
            request = request.bearer_auth(token);
        }
        if let Some(value) = payload {
            request = request.json(value)
        }
        if let Some(revision) = revision {
            if revision.len() > 200 || revision.contains(['\r', '\n']) {
                return Err("Недійсна версія синхронізації".into());
            }
            request = request.header("If-Match", revision)
        }
        let response = request
            .send()
            .map_err(|_| "Не вдалося з'єднатися з сервером SoundFlow")?;
        if response.status().as_u16() == 409 || response.status().as_u16() == 412 {
            return Err("sync_conflict: Бібліотеку змінено на іншому пристрої. Локальну бібліотеку збережено без змін.".into());
        }
        if !response.status().is_success() {
            return Err(match response.status().as_u16() {
                401 => "Сеанс завершено. Увійдіть знову.",
                403 => "Недостатньо прав для цієї дії.",
                _ => "Сервер SoundFlow не зміг виконати запит.",
            }
            .into());
        }
        let mut bytes = vec![];
        response
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Не вдалося прочитати відповідь сервера")?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("Відповідь сервера перевищує 32 МБ".into());
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Недійсна відповідь сервера")?;
        if !value.is_object() {
            return Err("Недійсна відповідь сервера".into());
        }
        Ok(value)
    }
    pub fn sign_in(&mut self, payload: &Value, register: bool) -> CoreResult<Value> {
        let email = payload["email"].as_str().unwrap_or("").trim();
        let password = payload["password"].as_str().unwrap_or("");
        if email.len() > 254 || !email.contains('@') || password.len() < 8 || password.len() > 1024
        {
            return Err("Вкажіть email та пароль від 8 символів".into());
        }
        let path = if register {
            "/v1/auth/register"
        } else {
            "/v1/auth/login"
        };
        let result = self.request(Method::POST, path, Some(payload), false, None)?;
        let token = result["access_token"]
            .as_str()
            .filter(|t| token_valid(t))
            .ok_or("Сервер не повернув токен доступу")?;
        let user = sanitized_user(&result["user"])?;
        let expires = expiry(&result["expires_at"])?;
        if expires <= now() {
            return Err("Сервер повернув прострочений сеанс".into());
        }
        credential(&self.base)?
            .set_password(token)
            .map_err(|_| "Не вдалося зберегти сеанс у системному сховищі")?;
        self.token = Some(token.into());
        self.user = Some(user);
        self.revision = None;
        Ok(self.status())
    }
    pub fn restore(&mut self) -> CoreResult<Value> {
        if self.base.is_empty() || self.token.is_none() {
            return Ok(self.status());
        }
        let result = self.request(Method::GET, "/v1/auth/me", None, true, None)?;
        self.user = Some(sanitized_user(&result["user"])?);
        Ok(self.status())
    }
    pub fn sign_out(&mut self) -> CoreResult<Value> {
        if !self.base.is_empty() {
            match credential(&self.base)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(_) => return Err("Не вдалося видалити сеанс із системного сховища".into()),
            }
        }
        self.token = None;
        self.user = None;
        self.revision = None;
        Ok(self.status())
    }
    pub fn entitlement(&self) -> Value {
        let mut base = json!({"available":!self.base.is_empty(),"active":false,"price":1,"currency":"USD","interval":"month","plan":"soundflow_plus"});
        if self.base.is_empty() {
            base["reason"] = json!("billing_not_configured");
            base["message"] = json!("Підписка ще не доступна: сервер не налаштовано.");
            return base;
        }
        if self.user.is_none() || self.token.is_none() {
            base["reason"] = json!("sign_in_required");
            base["message"] = json!("Увійдіть, щоб перевірити підписку.");
            return base;
        }
        let result = self.request(Method::GET, "/v1/entitlement", None, true, None);
        if let Ok(result) = result {
            let expires = expiry(&result["expires_at"]);
            let user = self.user_id();
            if result["user_id"].as_str() == user.as_deref()
                && result["plan"] == "soundflow_plus"
                && result["active"] == true
                && expires.is_ok_and(|e| e > now() && e <= now() + 370.0 * 86400.0)
            {
                base["active"] = json!(true);
                base["reason"] = json!("active");
                base["expires_at"] = result["expires_at"].clone();
                base["message"] = json!("SoundFlow Plus активна.");
                return base;
            }
        }
        base["reason"] = json!("verification_failed");
        base["message"] = json!("Не вдалося підтвердити підписку.");
        base
    }
    pub fn checkout(&self) -> CoreResult<Value> {
        let result = self.request(
            Method::POST,
            "/v1/checkout",
            Some(&json!({"plan":"soundflow_plus"})),
            true,
            None,
        )?;
        if result["user_id"].as_str() != self.user_id().as_deref() {
            return Err("Профіль оплати не збігається".into());
        }
        let url = Url::parse(result["url"].as_str().unwrap_or(""))
            .map_err(|_| "Недійсна адреса оплати")?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || !matches!(
                url.host_str(),
                Some("checkout.stripe.com" | "billing.stripe.com")
            )
        {
            return Err("Недійсна адреса оплати".into());
        }
        Ok(json!({"ok":true,"url":url.as_str()}))
    }
    pub fn fetch_library(&mut self) -> CoreResult<Value> {
        let result = self.request(Method::GET, "/v1/library", None, true, None)?;
        if !result["library"].is_object() {
            return Err("Сервер повернув недійсну бібліотеку".into());
        }
        let revision = result["revision"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 200)
            .ok_or("Сервер не повернув версію бібліотеки")?;
        self.revision = Some(revision.into());
        Ok(result)
    }
    pub fn push_library(&mut self, library: &Value) -> CoreResult<Value> {
        let revision = self
            .revision
            .clone()
            .ok_or("Спочатку завантажте бібліотеку для перевірки її версії")?;
        let result = self.request(
            Method::PUT,
            "/v1/library",
            Some(&json!({"library":library,"revision":revision})),
            true,
            Some(&revision),
        )?;
        let new = result["revision"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 200)
            .ok_or("Сервер не повернув версію бібліотеки")?;
        self.revision = Some(new.into());
        Ok(json!({"ok":true,"revision":new}))
    }
}
fn expiry(value: &Value) -> CoreResult<f64> {
    if let Some(number) = value.as_f64() {
        if number.is_finite() {
            return Ok(number);
        }
    }
    if let Some(date) = value.as_str() {
        return chrono::DateTime::parse_from_rfc3339(date)
            .map(|d| d.timestamp() as f64)
            .map_err(|_| "Недійсна дата завершення".into());
    }
    Err("Недійсна дата завершення".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloud_is_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let mut cloud = CloudClient::open(dir.path(), true).unwrap();
        assert_eq!(cloud.entitlement()["active"], false);
        assert!(cloud
            .configure(&json!({"base_url":"http://example.com"}))
            .is_err());
        assert!(cloud
            .configure(&json!({"base_url":"https://user:password@example.com"}))
            .is_err());
        assert!(!fs::read_to_string(dir.path().join("cloud.json")).is_ok());
    }
}
