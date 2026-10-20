//! Перевірка нових релізів SoundFlow на GitHub.
use serde_json::Value;
use std::time::Duration;

const RELEASES: &str = "https://api.github.com/repos/despito-studio/SoundFlow/releases/latest";

fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let clean = text.trim().trim_start_matches('v').trim_start_matches('V');
    let mut numbers = [0u64; 3];
    for (index, part) in clean.split('.').take(3).enumerate() {
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        numbers[index] = digits.parse().ok()?;
    }
    Some((numbers[0], numbers[1], numbers[2]))
}

/// Повертає тег і посилання на реліз, якщо на GitHub є версія новіша за поточну.
pub fn check() -> Option<(String, String)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(format!("SoundFlow/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;
    let response = client
        .get(RELEASES)
        .header("Accept", "application/vnd.github+json")
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body: Value = response.json().ok()?;
    if body["draft"].as_bool() == Some(true) || body["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = body["tag_name"].as_str()?.trim().to_string();
    let url = body["html_url"]
        .as_str()
        .unwrap_or("https://github.com/despito-studio/SoundFlow/releases")
        .to_string();
    let latest = parse_version(&tag)?;
    let current = parse_version(env!("CARGO_PKG_VERSION"))?;
    (latest > current).then_some((tag, url))
}

#[cfg(test)]
mod tests {
    use super::parse_version;

    #[test]
    fn versions_are_compared_numerically() {
        assert!(parse_version("v3.4.2") > parse_version("3.4.1"));
        assert!(parse_version("3.10.0") > parse_version("3.9.9"));
        assert_eq!(parse_version("v3.4.1"), parse_version("3.4.1"));
        assert_eq!(parse_version("3.4"), Some((3, 4, 0)));
        assert_eq!(parse_version("nightly"), None);
    }
}
