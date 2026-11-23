use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub target: String,
    pub args: String,
    pub working_dir: String,
    pub cover: String,
    pub banner: String,
    pub steam_id: String,
    pub favorite: bool,
}
impl Game {
    pub fn new(title: String, platform: &str, target: String) -> Self {
        Self { id: uuid::Uuid::new_v4().to_string(), title, platform: platform.into(), target,
            args: String::new(), working_dir: String::new(), cover: String::new(),
            banner: String::new(), steam_id: String::new(), favorite: false }
    }
    pub fn steam_art(&mut self, id: &str) {
        self.steam_id = id.to_owned();
        self.cover = format!("https://cdn.akamai.steamstatic.com/steam/apps/{id}/library_600x900.jpg");
        self.banner = format!("https://cdn.akamai.steamstatic.com/steam/apps/{id}/library_hero.jpg");
    }
    /// Numeric Steam app id, from the explicit field or from a `steam://rungameid/<id>` target.
    pub fn steam_app_id(&self) -> Option<String> {
        let digits = |s: &str| !s.is_empty() && s.len() <= 12 && s.bytes().all(|b| b.is_ascii_digit());
        if digits(&self.steam_id) { return Some(self.steam_id.clone()); }
        self.target.strip_prefix("steam://rungameid/").filter(|s| digits(s)).map(str::to_owned)
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String, pub accent: String, pub view: String, pub size: u32,
    pub gap: u32, pub radius: u32, pub titles: bool, pub performance: bool, pub sounds: bool,
    pub wallpaper: String, pub blur: u32, pub dim: u32, pub after_launch: String,
    pub folders: Vec<String>, pub onboarded: bool,
    /// Automatic library/playtime backups. An empty `backup_dir` means the app's own `backups` folder.
    pub backup_enabled: bool, pub backup_dir: String, pub backup_keep: u32,
    /// Discord Rich Presence needs the user's own Discord application id (a 17–20 digit snowflake).
    pub discord_enabled: bool, pub discord_app_id: String,
    /// Periodic Steam playtime import from a public profile; the API key is optional.
    pub steam_sync_enabled: bool, pub steam_profile: String, pub steam_api_key: String, pub steam_sync_hours: u32,
}
impl Default for Settings {
    fn default() -> Self { Self { theme: "cream".into(), accent: "#5b4bd6".into(), view: "poster".into(),
        size: 170, gap: 22, radius: 14, titles: true, performance: false, sounds: true, wallpaper: String::new(),
        blur: 0, dim: 30, after_launch: "minimize".into(), folders: vec![], onboarded: false,
        backup_enabled: true, backup_dir: String::new(), backup_keep: 10,
        discord_enabled: false, discord_app_id: String::new(),
        steam_sync_enabled: false, steam_profile: String::new(), steam_api_key: String::new(), steam_sync_hours: 6 } }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Library { pub games: Vec<Game>, pub settings: Settings }

/// What the scan review shows for one candidate. Kept out of `Game` so it is never persisted.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanDetail {
    /// Executable size, or the store-reported install size for Steam and Epic entries.
    pub size_bytes: Option<u64>,
    /// Clean, human-readable location (no `\\?\` prefix).
    pub path: String,
    /// "Steam", "Epic", "Riot" or "Folder".
    pub source: String,
}
/// A file the strict filters rejected, kept so a wrongly rejected game can be rescued from the review.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilteredItem { pub game: Game, pub detail: ScanDetail, pub reason: String }
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub executables_seen: u32,
    pub too_small: u32,
    pub blacklisted: u32,
    pub known_tools: u32,
    pub already_managed: u32,
    pub duplicates: u32,
    /// Files that look downloaded (a version number in the name, or loose in a Downloads folder) and have no game files.
    pub installers: u32,
    /// Programs under Program Files, ProgramData, AppData and similar with no game files next to them.
    pub programs: u32,
    /// Files in a Downloads folder with no game files next to them; these are listed under "Filtered out".
    pub no_game_files: u32,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub games: Vec<Game>,
    pub warnings: Vec<String>,
    pub details: HashMap<String, ScanDetail>,
    pub filtered: Vec<FilteredItem>,
    pub stats: ScanStats,
    /// The player stopped the scan; everything else is empty.
    pub cancelled: bool,
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test] fn settings_saved_before_navigation_sounds_existed_load_with_the_sound_on() {
        let old: Settings = serde_json::from_str(r#"{"theme":"navy","performance":true}"#).unwrap();
        assert!(old.sounds, "a missing field takes the default, which is on");
        assert!(old.performance && old.theme == "navy", "the fields that are present are kept");
    }
    #[test] fn the_navigation_sounds_setting_round_trips_under_its_camel_case_name() {
        assert!(serde_json::to_string(&Settings::default()).unwrap().contains(r#""sounds":true"#));
        let off: Settings = serde_json::from_str(r#"{"sounds":false}"#).unwrap();
        assert!(!off.sounds);
        assert!(!serde_json::from_str::<Settings>(&serde_json::to_string(&off).unwrap()).unwrap().sounds);
    }
}
