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
    /// Closing the window hides Aether Hub in the system tray instead of quitting. Off means closing quits, as before.
    pub keep_in_tray: bool,
    /// A global shortcut that brings the launcher back from anywhere (the keys are fixed, see `hotkey.rs`).
    pub shortcut: bool,
    /// Start with Windows. With `keep_in_tray` on it starts hidden in the tray; otherwise the window opens as usual.
    pub start_with_windows: bool,
    /// Open Big Box when a controller connects, unless a game is running. Needs Aether Hub to be running (tray or minimized).
    pub pad_wake: bool,
    /// Ask GitHub once per start whether a newer release exists. Off by default: Aether Hub makes no network request for this unless asked.
    pub check_updates: bool,
    /// "Living colors": the interface takes on the colours of the game in focus. On by default; Performance mode turns it off.
    pub living_colors: bool,
    /// Console mode: with a controller connected, Aether Hub starts in Big Box and comes back to Big Box when a game
    /// ends. Nothing changes when no controller is connected. On by default.
    pub console_mode: bool,
    /// A short rumble on the controller when a game is launched with it. On by default.
    pub rumble: bool,
    /// The PS / Xbox button brings Aether Hub forward (a quick menu while a game runs). It listens to the controller in
    /// the background, so like the other background behaviours it is off until the player switches it on.
    pub home_button: bool,
}
impl Default for Settings {
    fn default() -> Self { Self { theme: "cream".into(), accent: "#5b4bd6".into(), view: "poster".into(),
        size: 170, gap: 22, radius: 14, titles: true, performance: false, sounds: true, wallpaper: String::new(),
        blur: 0, dim: 30, after_launch: "minimize".into(), folders: vec![], onboarded: false,
        backup_enabled: true, backup_dir: String::new(), backup_keep: 10,
        discord_enabled: false, discord_app_id: String::new(),
        steam_sync_enabled: false, steam_profile: String::new(), steam_api_key: String::new(), steam_sync_hours: 6,
        keep_in_tray: false, shortcut: false, start_with_windows: false, pad_wake: false, check_updates: false, living_colors: true, console_mode: true, rumble: true, home_button: false } }
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
    #[test] fn background_behaviour_is_off_until_the_player_turns_it_on() {
        let old: Settings = serde_json::from_str(r#"{"theme":"navy"}"#).unwrap();
        assert!(!old.keep_in_tray && !old.shortcut && !old.start_with_windows && !old.pad_wake && !old.check_updates,
            "closing must still quit, nothing may register a key or start with Windows, and no update request may be made, until asked");
    }
    #[test] fn living_colors_is_on_for_libraries_saved_before_it_existed() {
        let old: Settings = serde_json::from_str(r#"{"theme":"navy"}"#).unwrap();
        assert!(old.living_colors);
        assert!(serde_json::to_string(&Settings::default()).unwrap().contains(r#""livingColors":true"#));
        assert!(!serde_json::from_str::<Settings>(r#"{"livingColors":false}"#).unwrap().living_colors);
    }
    #[test] fn console_mode_is_on_for_libraries_saved_before_it_existed() {
        assert!(serde_json::from_str::<Settings>(r#"{"theme":"navy"}"#).unwrap().console_mode);
        assert!(serde_json::to_string(&Settings::default()).unwrap().contains(r#""consoleMode":true"#));
        assert!(!serde_json::from_str::<Settings>(r#"{"consoleMode":false}"#).unwrap().console_mode);
    }
    #[test] fn rumble_is_on_and_the_home_button_is_off_until_asked() {
        let old: Settings = serde_json::from_str(r#"{"theme":"navy"}"#).unwrap();
        assert!(old.rumble && !old.home_button, "the background listener never starts on its own");
        let json = serde_json::to_string(&Settings::default()).unwrap();
        assert!(json.contains(r#""rumble":true"#) && json.contains(r#""homeButton":false"#));
        assert!(serde_json::from_str::<Settings>(r#"{"homeButton":true}"#).unwrap().home_button);
    }
    #[test] fn background_settings_use_their_camel_case_names() {
        let json = serde_json::to_string(&Settings::default()).unwrap();
        for name in ["keepInTray", "shortcut", "startWithWindows", "padWake", "checkUpdates"] { assert!(json.contains(&format!("\"{name}\":false")), "{name} in {json}"); }
        let on: Settings = serde_json::from_str(r#"{"keepInTray":true,"padWake":true}"#).unwrap();
        assert!(on.keep_in_tray && on.pad_wake && !on.shortcut);
    }
}
