//! Локалізовані рядки для нативного шару (те, що формується в Rust, а не в Slint).
use slint::SharedString;

/// Повертає український або англійський варіант залежно від мови інтерфейсу.
pub fn text(language: &str, uk: &str, en: &str) -> SharedString {
    if language == "en" {
        en.into()
    } else {
        uk.into()
    }
}
