pub mod accounts;
pub mod application;
mod cloud;
pub mod core;
pub mod desktop;
pub mod discord;
pub mod library;
pub mod listening;
mod media;
mod media_keys;
pub mod platform;
pub mod playback;
pub mod verification;

slint::include_modules!();

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    application::run()
}
