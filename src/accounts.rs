use crate::library::{now, CoreResult};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SubscriptionState {
    pub active: bool,
    pub plan: String,
    pub plan_name: String,
    pub tier: String,
    pub expires_at: f64,
    pub expires_formatted: String,
    #[serde(default)]
    pub promo_code: String,
    #[serde(default)]
    pub granted_by: String,
}

impl Default for SubscriptionState {
    fn default() -> Self {
        Self {
            active: false,
            plan: "soundflow_plus".into(),
            plan_name: "SoundFlow Plus".into(),
            tier: "free".into(),
            expires_at: 0.0,
            expires_formatted: "Неактивна".into(),
            promo_code: String::new(),
            granted_by: String::new(),
        }
    }
}

impl SubscriptionState {
    pub fn is_valid(&self) -> bool {
        if !self.active {
            return false;
        }
        if self.expires_at == 0.0 || self.tier == "lifetime" {
            return true;
        }
        self.expires_at > now()
    }

    pub fn to_value(&self) -> Value {
        let valid = self.is_valid();
        json!({
            "available": true,
            "active": valid,
            "plan": self.plan,
            "plan_name": self.plan_name,
            "tier": self.tier,
            "expires_at": self.expires_at,
            "expires_formatted": if valid {
                self.expires_formatted.clone()
            } else if self.active {
                "Термін підписки минув".into()
            } else {
                "Підписка неактивна".into()
            },
            "promo_code": self.promo_code,
            "granted_by": self.granted_by,
            "message": if valid {
                format!("SoundFlow Plus активна ⭐ ({})", self.expires_formatted)
            } else {
                "Підписка неактивна".into()
            }
        })
    }
}

pub fn format_expiry(expires_at: f64, tier: &str) -> String {
    if expires_at == 0.0 || tier == "lifetime" {
        "Безстрокова (Lifetime) ⭐".into()
    } else if let Some(dt) = DateTime::from_timestamp(expires_at as i64, 0) {
        format!("до {}", dt.format("%d.%m.%Y"))
    } else {
        "Активна".into()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AccountEntry {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub avatar_url: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "now")]
    pub created_at: f64,
    #[serde(default)]
    pub subscription: SubscriptionState,
}

fn default_provider() -> String {
    "local".into()
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AccountsConfig {
    #[serde(default = "default_active_id")]
    pub active_id: String,
    #[serde(default)]
    pub accounts: Vec<AccountEntry>,
    #[serde(default)]
    pub guest_subscription: SubscriptionState,
}

fn default_active_id() -> String {
    "guest".into()
}

pub struct AccountsStore {
    path: PathBuf,
    pub config: AccountsConfig,
}

impl AccountsStore {
    pub fn open(data_dir: &Path) -> CoreResult<Self> {
        let path = data_dir.join("accounts.json");
        let mut config: Option<AccountsConfig> = if path.is_file() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
        } else {
            None
        };

        if config.is_none() {
            // First run or migration from existing user libraries & session.json
            let mut accounts = Vec::new();
            let mut active_id = "guest".to_string();

            // Check session.json for existing user ID
            let session_file = data_dir.join("session.json");
            let last_user = fs::read_to_string(&session_file)
                .ok()
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .and_then(|v| v["last_user_id"].as_str().map(str::to_owned));

            // Scan data_dir for existing library_*.json files
            if let Ok(entries) = fs::read_dir(data_dir) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name().to_string_lossy().into_owned();
                    if let Some(user_part) = file_name.strip_prefix("library_").and_then(|s| s.strip_suffix(".json")) {
                        if user_part != "guest" {
                            let mut name = user_part.to_string();
                            let mut email = format!("{user_part}@soundflow.local");
                            let mut role = "listener".to_string();
                            if let Ok(content) = fs::read_to_string(entry.path()) {
                                if let Ok(val) = serde_json::from_str::<Value>(&content) {
                                    if let Some(p) = val.get("profile") {
                                        if let Some(n) = p["name"].as_str().filter(|s| !s.is_empty()) {
                                            name = n.to_string();
                                        }
                                        if let Some(e) = p["email"].as_str().filter(|s| !s.is_empty()) {
                                            email = e.to_string();
                                        }
                                        if let Some(r) = p["role"].as_str() {
                                            role = r.to_string();
                                        }
                                    }
                                }
                            }
                            let is_admin = role == "admin";
                            let subscription = if is_admin {
                                SubscriptionState {
                                    active: true,
                                    plan: "soundflow_plus".into(),
                                    plan_name: "SoundFlow Plus ⭐ (Admin)".into(),
                                    tier: "lifetime".into(),
                                    expires_at: 0.0,
                                    expires_formatted: "Безстрокова (Admin VIP) ⭐".into(),
                                    promo_code: "ADMIN".into(),
                                    granted_by: "admin".into(),
                                }
                            } else {
                                SubscriptionState {
                                    active: true,
                                    plan: "soundflow_plus".into(),
                                    plan_name: "SoundFlow Plus ⭐".into(),
                                    tier: "lifetime".into(),
                                    expires_at: 0.0,
                                    expires_formatted: "Безстрокова (Lifetime) ⭐".into(),
                                    promo_code: "SOUNDFLOW".into(),
                                    granted_by: "promo".into(),
                                }
                            };
                            accounts.push(AccountEntry {
                                id: user_part.to_string(),
                                name,
                                email,
                                avatar_url: String::new(),
                                provider: "local".into(),
                                created_at: now(),
                                subscription,
                            });
                        }
                    }
                }
            }

            if let Some(ref last) = last_user {
                if accounts.iter().any(|a| &a.id == last) {
                    active_id = last.clone();
                }
            } else if let Some(first) = accounts.first() {
                active_id = first.id.clone();
            }

            let initial_config = AccountsConfig {
                active_id,
                accounts,
                guest_subscription: SubscriptionState::default(),
            };
            config = Some(initial_config);
        }

        let store = Self {
            path,
            config: config.unwrap_or_else(|| AccountsConfig {
                active_id: "guest".into(),
                accounts: Vec::new(),
                guest_subscription: SubscriptionState::default(),
            }),
        };
        let _ = store.save();
        Ok(store)
    }

    pub fn save(&self) -> CoreResult<()> {
        let parent = self.path.parent().ok_or("Недійсна папка акаунтів")?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let serialized = serde_json::to_string_pretty(&self.config).map_err(|e| e.to_string())?;
        temp.write_all(serialized.as_bytes()).map_err(|e| e.to_string())?;
        temp.persist(&self.path).map_err(|e| e.error.to_string())?;
        Ok(())
    }

    pub fn active_account(&self) -> Option<&AccountEntry> {
        if self.config.active_id == "guest" {
            None
        } else {
            self.config.accounts.iter().find(|a| a.id == self.config.active_id)
        }
    }

    pub fn active_account_mut(&mut self) -> Option<&mut AccountEntry> {
        if self.config.active_id == "guest" {
            None
        } else {
            let id = self.config.active_id.clone();
            self.config.accounts.iter_mut().find(|a| a.id == id)
        }
    }

    pub fn active_id(&self) -> &str {
        &self.config.active_id
    }

    pub fn active_user_display(&self) -> (String, String) {
        if let Some(acc) = self.active_account() {
            (acc.name.clone(), acc.email.clone())
        } else {
            ("Гість".into(), "guest@soundflow.local".into())
        }
    }

    pub fn is_signed_in(&self) -> bool {
        self.config.active_id != "guest" && self.active_account().is_some()
    }

    pub fn get_subscription(&self) -> SubscriptionState {
        if let Some(acc) = self.active_account() {
            if acc.subscription.is_valid() {
                return acc.subscription.clone();
            }
        }
        if self.config.guest_subscription.is_valid() {
            return self.config.guest_subscription.clone();
        }
        SubscriptionState::default()
    }

    pub fn buy_subscription(&mut self, tier: &str) -> CoreResult<Value> {
        let tier = match tier.trim().to_lowercase().as_str() {
            "month" | "1m" | "m" => "month",
            "year" | "1y" | "y" | "12m" => "year",
            _ => "lifetime",
        };
        let (expires_at, plan_name, expires_formatted) = match tier {
            "month" => {
                let exp = now() + 30.0 * 86400.0;
                (exp, "SoundFlow Plus (Місячний)".into(), format_expiry(exp, "month"))
            }
            "year" => {
                let exp = now() + 365.0 * 86400.0;
                (exp, "SoundFlow Plus (Річний)".into(), format_expiry(exp, "year"))
            }
            _ => (0.0, "SoundFlow Plus ⭐ (Безстроковий)".into(), "Безстрокова (Lifetime) ⭐".into()),
        };

        let sub = SubscriptionState {
            active: true,
            plan: "soundflow_plus".into(),
            plan_name,
            tier: tier.into(),
            expires_at,
            expires_formatted,
            promo_code: String::new(),
            granted_by: "purchase".into(),
        };

        if self.is_signed_in() {
            if let Some(acc) = self.active_account_mut() {
                acc.subscription = sub.clone();
            }
        } else {
            self.config.guest_subscription = sub.clone();
        }
        self.save()?;

        Ok(json!({
            "ok": true,
            "message": "SoundFlow Plus успішно активовано! Дякуємо за підтримку!",
            "subscription": sub.to_value()
        }))
    }

    pub fn grant_subscription(&mut self, code_or_days: &str) -> CoreResult<Value> {
        let raw = code_or_days.trim();
        let upper = raw.to_uppercase();

        let (tier, expires_at, plan_name, formatted) = if matches!(
            upper.as_str(),
            "VIP" | "ADMIN" | "LIFETIME" | "FOREVER" | "SOUNDFLOW" | "PREMIUM" | "DEV" | "TEST" | "PRO" | "STAR"
        ) {
            ("lifetime", 0.0, "SoundFlow Plus ⭐ (Lifetime VIP)".into(), "Безстрокова (VIP) ⭐".into())
        } else if let Ok(days) = raw.parse::<f64>() {
            if days <= 0.0 {
                ("lifetime", 0.0, "SoundFlow Plus ⭐ (Lifetime)".into(), "Безстрокова (Lifetime) ⭐".into())
            } else {
                let exp = now() + days * 86400.0;
                ("custom", exp, format!("SoundFlow Plus ({days:.0} дн.)"), format_expiry(exp, "custom"))
            }
        } else if upper == "PLUS" || upper == "FREE" || upper == "MONTH" {
            let exp = now() + 30.0 * 86400.0;
            ("month", exp, "SoundFlow Plus (30 днів)".into(), format_expiry(exp, "month"))
        } else {
            // Any user-entered promo code is accepted and grants 1 year of SoundFlow Plus!
            let exp = now() + 365.0 * 86400.0;
            ("year", exp, format!("SoundFlow Plus ({upper}) ⭐"), format_expiry(exp, "year"))
        };

        let sub = SubscriptionState {
            active: true,
            plan: "soundflow_plus".into(),
            plan_name,
            tier: tier.into(),
            expires_at,
            expires_formatted: formatted,
            promo_code: upper,
            granted_by: "promo".into(),
        };

        if self.is_signed_in() {
            if let Some(acc) = self.active_account_mut() {
                acc.subscription = sub.clone();
            }
        } else {
            self.config.guest_subscription = sub.clone();
        }
        self.save()?;

        Ok(json!({
            "ok": true,
            "message": "Промокод успішно застосовано! SoundFlow Plus активовано.",
            "subscription": sub.to_value()
        }))
    }

    pub fn cancel_subscription(&mut self) -> CoreResult<Value> {
        let sub = SubscriptionState::default();
        if let Some(acc) = self.active_account_mut() {
            acc.subscription = sub.clone();
        }
        for acc in &mut self.config.accounts {
            acc.subscription = sub.clone();
        }
        self.config.guest_subscription = sub.clone();
        self.save()?;
        Ok(json!({
            "ok": true,
            "message": "Підписку SoundFlow Plus скасовано.",
            "subscription": sub.to_value()
        }))
    }

    pub fn github_sign_in(&mut self, handle: &str, name: &str) -> CoreResult<Value> {
        let handle = handle.trim();
        if handle.is_empty() {
            return Err("Вкажіть GitHub username або email".into());
        }
        let clean_handle = handle.trim_start_matches('@');
        let email = if clean_handle.contains('@') {
            clean_handle.to_lowercase()
        } else {
            format!("{}@users.noreply.github.com", clean_handle.to_lowercase())
        };
        let display_name = if !name.trim().is_empty() {
            name.trim().to_string()
        } else {
            clean_handle.to_string()
        };

        let existing_index = self
            .config
            .accounts
            .iter()
            .position(|a| a.email.to_lowercase() == email || a.name.to_lowercase() == clean_handle.to_lowercase());
        if let Some(idx) = existing_index {
            self.config.active_id = self.config.accounts[idx].id.clone();
            if !name.trim().is_empty() {
                self.config.accounts[idx].name = display_name;
            }
        } else {
            let id = format!("usr_gh_{}", &uuid::Uuid::new_v4().simple().to_string()[..10]);
            let mut sub = self.config.guest_subscription.clone();
            if !sub.is_valid() {
                sub = SubscriptionState {
                    active: true,
                    plan: "soundflow_plus".into(),
                    plan_name: "SoundFlow Plus ⭐".into(),
                    tier: "lifetime".into(),
                    expires_at: 0.0,
                    expires_formatted: "Безстрокова (GitHub) ⭐".into(),
                    promo_code: "GITHUB_SIGNIN".into(),
                    granted_by: "oauth".into(),
                };
            }
            let avatar = format!("https://github.com/{}.png", clean_handle.split('@').next().unwrap_or(clean_handle));
            let entry = AccountEntry {
                id: id.clone(),
                name: display_name,
                email,
                avatar_url: avatar,
                provider: "github".into(),
                created_at: now(),
                subscription: sub,
            };
            self.config.accounts.push(entry);
            self.config.active_id = id;
        }

        self.save()?;
        Ok(self.status())
    }

    pub fn google_sign_in(&mut self, email: &str, name: &str, avatar_url: &str) -> CoreResult<Value> {
        let email = email.trim().to_lowercase();
        if email.is_empty() || !email.contains('@') {
            return Err("Вкажіть коректний Google email".into());
        }
        let display_name = if !name.trim().is_empty() {
            name.trim().to_string()
        } else {
            email.split('@').next().unwrap_or("Користувач").to_string()
        };

        let existing_index = self.config.accounts.iter().position(|a| a.email.to_lowercase() == email);
        if let Some(idx) = existing_index {
            self.config.active_id = self.config.accounts[idx].id.clone();
            if !name.trim().is_empty() {
                self.config.accounts[idx].name = display_name;
            }
            if !avatar_url.trim().is_empty() {
                self.config.accounts[idx].avatar_url = avatar_url.trim().to_string();
            }
        } else {
            let id = format!("usr_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
            let mut sub = self.config.guest_subscription.clone();
            if !sub.is_valid() {
                sub = SubscriptionState {
                    active: true,
                    plan: "soundflow_plus".into(),
                    plan_name: "SoundFlow Plus ⭐".into(),
                    tier: "lifetime".into(),
                    expires_at: 0.0,
                    expires_formatted: "Безстрокова (Lifetime) ⭐".into(),
                    promo_code: "GOOGLE_SIGNIN".into(),
                    granted_by: "promo".into(),
                };
            }
            let entry = AccountEntry {
                id: id.clone(),
                name: display_name,
                email: email.clone(),
                avatar_url: avatar_url.trim().to_string(),
                provider: "google".into(),
                created_at: now(),
                subscription: sub,
            };
            self.config.accounts.push(entry);
            self.config.active_id = id;
        }

        self.save()?;
        Ok(self.status())
    }

    pub fn local_sign_in(&mut self, email: &str, password: &str) -> CoreResult<Value> {
        let email = email.trim().to_lowercase();
        if email.is_empty() || !email.contains('@') {
            return Err("Вкажіть коректний email".into());
        }
        let existing = self.config.accounts.iter().find(|a| a.email.to_lowercase() == email);
        if let Some(acc) = existing {
            self.config.active_id = acc.id.clone();
            self.save()?;
            Ok(self.status())
        } else {
            // Auto create account if not yet exists
            let name = email.split('@').next().unwrap_or("User");
            self.local_sign_up(name, &email, password)
        }
    }

    pub fn local_sign_up(&mut self, name: &str, email: &str, _password: &str) -> CoreResult<Value> {
        let email = email.trim().to_lowercase();
        if email.is_empty() || !email.contains('@') {
            return Err("Вкажіть коректний email".into());
        }
        let name = if name.trim().is_empty() {
            email.split('@').next().unwrap_or("User").to_string()
        } else {
            name.trim().to_string()
        };

        if let Some(existing) = self.config.accounts.iter().find(|a| a.email.to_lowercase() == email) {
            self.config.active_id = existing.id.clone();
            self.save()?;
            return Ok(self.status());
        }

        let id = format!("usr_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let mut sub = self.config.guest_subscription.clone();
        if !sub.is_valid() {
            sub = SubscriptionState {
                active: true,
                plan: "soundflow_plus".into(),
                plan_name: "SoundFlow Plus ⭐".into(),
                tier: "lifetime".into(),
                expires_at: 0.0,
                expires_formatted: "Безстрокова (Lifetime) ⭐".into(),
                promo_code: "WELCOME".into(),
                granted_by: "promo".into(),
            };
        }
        let entry = AccountEntry {
            id: id.clone(),
            name,
            email,
            avatar_url: String::new(),
            provider: "local".into(),
            created_at: now(),
            subscription: sub,
        };
        self.config.accounts.push(entry);
        self.config.active_id = id;
        self.save()?;
        Ok(self.status())
    }

    pub fn switch_account(&mut self, user_id: &str) -> CoreResult<Value> {
        let user_id = user_id.trim();
        if user_id == "guest" {
            self.config.active_id = "guest".into();
        } else if self.config.accounts.iter().any(|a| a.id == user_id) {
            self.config.active_id = user_id.to_string();
        } else {
            return Err("Акаунт не знайдено".into());
        }
        self.save()?;
        Ok(self.status())
    }

    pub fn sign_out(&mut self) -> CoreResult<Value> {
        self.config.active_id = "guest".into();
        self.save()?;
        Ok(self.status())
    }

    pub fn list_accounts(&self) -> Value {
        let list: Vec<Value> = self
            .config
            .accounts
            .iter()
            .map(|a| {
                json!({
                    "id": a.id,
                    "name": a.name,
                    "email": a.email,
                    "provider": a.provider,
                    "is_active": a.id == self.config.active_id,
                    "is_plus": a.subscription.is_valid(),
                    "expires_formatted": a.subscription.expires_formatted
                })
            })
            .collect();
        json!(list)
    }

    pub fn status(&self) -> Value {
        let is_signed_in = self.is_signed_in();
        let (user_id, name, email, avatar, provider) = if let Some(acc) = self.active_account() {
            (
                acc.id.clone(),
                acc.name.clone(),
                acc.email.clone(),
                acc.avatar_url.clone(),
                acc.provider.clone(),
            )
        } else {
            (
                "guest".into(),
                "Гість".into(),
                "".into(),
                "".into(),
                "guest".into(),
            )
        };

        json!({
            "signed_in": is_signed_in,
            "active_id": self.config.active_id,
            "user": {
                "id": user_id,
                "name": name,
                "email": email,
                "avatar_url": avatar,
                "provider": provider
            },
            "subscription": self.get_subscription().to_value(),
            "accounts": self.list_accounts()
        })
    }
}
