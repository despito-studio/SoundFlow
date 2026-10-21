pub mod accounts;
pub mod application;
mod cloud;
pub mod core;
pub mod desktop;
pub mod discord;
pub mod i18n;
pub mod library;
pub mod listening;
mod media;
mod media_keys;
pub mod platform;
pub mod playback;
pub mod shortcut;
pub mod soundcloud;
pub mod supabase;
pub mod supabase_auth;
pub mod supabase_sync;
pub mod update;
pub mod verification;

slint::include_modules!();

/// Коротка форма для локалізованого рядка, що залежить від мови інтерфейсу.
#[macro_export]
macro_rules! tr {
    ($ui:expr, $uk:expr, $en:expr) => {
        $crate::i18n::text($ui.get_language().as_str(), $uk, $en)
    };
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    application::run()
}
