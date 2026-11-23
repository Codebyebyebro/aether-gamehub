//! "Is there a newer Aether Hub?" One request to GitHub's public releases API, made only when the player asks for it
//! (the button) or has switched the start-up check on. Nothing is downloaded or installed here: the player is sent
//! to the releases page and installs the new version themselves, which is also the only honest option for an
//! unsigned installer.
use crate::version;
use serde::Serialize;
use std::time::Duration;

/// Where releases are published. One place, so a renamed or moved repository is a one-line change.
pub const REPO: &str = "Codebyebyebro/aether-gamehub";
pub fn page() -> String { format!("https://github.com/{REPO}/releases/latest") }
pub fn current() -> &'static str { env!("CARGO_PKG_VERSION") }

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Update { pub current: String, pub latest: Option<String>, pub available: bool, pub note: String }

/// Turns GitHub's answer into something the interface can show. Only the version number is taken from it, and only
/// after it has been read as numbers, so nothing the network says is ever shown or opened as it came.
pub fn read(current: &str, status: u16, body: &str) -> Result<Update, String> {
    let known = |latest: Option<String>, available: bool, note: &str| Ok(Update { current: current.to_owned(), latest, available, note: note.to_owned() });
    match status {
        404 => known(None, false, "No release has been published yet."),
        200 => {
            let json: serde_json::Value = serde_json::from_str(body).map_err(|_| "GitHub's answer could not be read.".to_string())?;
            let tag = json.get("tag_name").and_then(|t| t.as_str()).ok_or("GitHub's answer did not name a version.")?;
            let latest = version::show(version::parse(tag).ok_or("The newest release has a version number Aether Hub cannot read.")?);
            let available = version::is_newer(current, &latest);
            known(Some(latest), available, if available { "A newer version is available." } else { "You have the latest version." })
        }
        403 | 429 => Err("GitHub is limiting requests from this connection right now. Try again later.".into()),
        other => Err(format!("GitHub answered with status {other}.")),
    }
}

pub fn check() -> Result<Update, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10)).redirect(reqwest::redirect::Policy::none())
        .user_agent(format!("AetherHub/{}", current()))
        .build().map_err(|e| e.to_string())?;
    let response = client.get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .send().map_err(|_| "Could not reach GitHub. Check your internet connection.".to_string())?;
    let status = response.status().as_u16();
    let bytes = response.bytes().map_err(|_| "GitHub's answer was cut off.".to_string())?;
    if bytes.len() > 2_000_000 { return Err("GitHub's answer was larger than expected.".into()); }
    read(current(), status, &String::from_utf8_lossy(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    const LATEST: &str = r#"{"tag_name":"v0.2.0","name":"Aether Hub 0.2.0","draft":false,"prerelease":false,"assets":[]}"#;

    #[test] fn a_newer_release_is_reported_with_a_clean_version_number() {
        let u = read("0.1.0", 200, LATEST).unwrap();
        assert_eq!(u, Update { current: "0.1.0".into(), latest: Some("0.2.0".into()), available: true, note: "A newer version is available.".into() });
    }
    #[test] fn the_same_version_is_not_an_update() {
        let u = read("0.2.0", 200, LATEST).unwrap();
        assert!(!u.available && u.latest.as_deref() == Some("0.2.0"));
    }
    #[test] fn a_repository_with_no_release_yet_is_not_an_error() {
        let u = read("0.1.0", 404, r#"{"message":"Not Found"}"#).unwrap();
        assert!(!u.available && u.latest.is_none() && u.note.contains("No release"));
    }
    #[test] fn a_rate_limit_and_other_failures_say_so() {
        assert!(read("0.1.0", 403, "{}").unwrap_err().contains("limiting"));
        assert!(read("0.1.0", 429, "{}").unwrap_err().contains("limiting"));
        assert!(read("0.1.0", 500, "").unwrap_err().contains("500"));
        assert!(read("0.1.0", 301, "").unwrap_err().contains("301"), "a redirect is not followed");
    }
    #[test] fn answers_that_are_not_what_was_expected_are_errors_not_updates() {
        assert!(read("0.1.0", 200, "<html>").is_err());
        assert!(read("0.1.0", 200, r#"{"name":"no tag"}"#).is_err());
        assert!(read("0.1.0", 200, r#"{"tag_name":"nightly"}"#).is_err());
        assert!(read("0.1.0", 200, r#"{"tag_name":7}"#).is_err());
    }
    #[test] fn nothing_from_the_network_is_shown_as_it_came() {
        let u = read("0.1.0", 200, r#"{"tag_name":"v9.9.9-<script>alert(1)</script>"}"#).unwrap();
        assert_eq!(u.latest.as_deref(), Some("9.9.9"), "only the numbers survive");
        assert!(!u.note.contains('<'));
    }
    #[test] fn the_page_is_a_fixed_github_address() {
        assert_eq!(page(), format!("https://github.com/{REPO}/releases/latest"));
        assert!(page().starts_with("https://github.com/"));
    }
    /// The number the check compares against comes from Cargo.toml; the number on the installer comes from tauri.conf.json.
    #[test] fn the_installer_and_the_program_report_the_same_version() {
        let conf = include_str!("../../src-tauri/tauri.conf.json");
        let at = conf.find("\"version\"").expect("a version in tauri.conf.json");
        let value = conf[at..].split('"').nth(3).expect("its value");
        assert_eq!(value, current(), "bump both together, or every install will think it is out of date");
    }
}
