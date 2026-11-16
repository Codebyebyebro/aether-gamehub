use serde::{Deserialize, Serialize};

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
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String, pub accent: String, pub view: String, pub size: u32,
    pub gap: u32, pub radius: u32, pub titles: bool, pub performance: bool,
    pub wallpaper: String, pub blur: u32, pub dim: u32, pub after_launch: String,
    pub folders: Vec<String>, pub onboarded: bool,
}
impl Default for Settings {
    fn default() -> Self { Self { theme: "cream".into(), accent: "#5b4bd6".into(), view: "poster".into(),
        size: 170, gap: 22, radius: 14, titles: true, performance: false, wallpaper: String::new(),
        blur: 0, dim: 30, after_launch: "minimize".into(), folders: vec![], onboarded: false } }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Library { pub games: Vec<Game>, pub settings: Settings }
#[derive(Serialize)]
pub struct ScanResult { pub games: Vec<Game>, pub warnings: Vec<String> }
