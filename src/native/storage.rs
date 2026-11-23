use crate::{models::Library, util};
use std::{collections::HashSet, fs::{self, File}, io::Write, path::{Path, PathBuf}, sync::Mutex};

/// Must equal `identifier` in `src-tauri/tauri.conf.json`: Tauri's `$APPDATA` (used by the asset-protocol
/// scope for covers) resolves to `<roaming app data>/<identifier>`.
pub const APP_ID: &str = "com.aetherhub.launcher";
/// Earlier releases stored data under these folders. They are copied, never moved or deleted.
pub const LEGACY_APP_IDS: &[&str] = &["com.aetherhub.desktop"];
pub static WRITE_LOCK: Mutex<()> = Mutex::new(());
/// Built-in wallpapers shipped with the interface; they are not files in the covers folder.
pub const WALLPAPER_PRESETS: &[&str] = &["/orbit-wallpaper.svg", "/orbit-navy-wallpaper.svg", "/orbit-cyan-wallpaper.svg"];

fn data_base() -> Result<PathBuf, String> {
    // `AETHER_DATA_DIR` lets tests and portable setups relocate everything without touching Windows folders.
    if let Some(dir) = std::env::var_os("AETHER_DATA_DIR") { return Ok(PathBuf::from(dir)); }
    dirs::data_dir().ok_or_else(|| "Windows application data directory is unavailable".to_string())
}
pub fn root() -> Result<PathBuf, String> {
    let p = data_base()?.join(APP_ID);
    fs::create_dir_all(p.join("covers")).map_err(|e| e.to_string())?;
    Ok(p)
}
pub fn load() -> Result<Library, String> {
    let path = root()?.join("library.json");
    if !path.exists() { return Ok(Library::default()); }
    let bytes = fs::read(&path).map_err(|e| format!("Cannot read library: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Cannot parse library: {e}. Your library and backup files have not been overwritten."))
}
/// Launch-target identity used everywhere duplicates are detected (Rust and the TypeScript adapter agree).
pub fn identity(target: &str, args: &str) -> String {
    format!("{}|{}", util::strip_verbatim(target).replace('/', "\\").to_lowercase(), args.trim())
}
pub fn validate(library: &Library) -> Result<(), String> {
    if library.games.len() > 20000 { return Err("The library exceeds the 20,000-entry safety limit".into()); }
    let mut ids = HashSet::new(); let mut targets = HashSet::new();
    for game in &library.games {
        uuid::Uuid::parse_str(&game.id).map_err(|_| "Invalid game identifier")?;
        if game.title.trim().is_empty() || game.title.len() > 640 || game.target.is_empty() || game.target.len() > 32760 || game.args.len() > 16000 || game.target.contains('\0') || game.args.contains('\0') || game.working_dir.contains('\0') { return Err("Invalid title, launch target, or launch arguments".into()); }
        if !["Steam", "Epic", "Riot", "Custom"].contains(&game.platform.as_str()) { return Err("Unsupported platform".into()); }
        if !ids.insert(&game.id) || !targets.insert(identity(&game.target, &game.args)) { return Err("Duplicate game identifier or launch target".into()); }
        crate::launch::validate_target(&game.target)?;
        if game.target.contains("://") && !game.args.trim().is_empty() { return Err("Configure Steam/Epic launch arguments in the storefront client, or use a direct executable target".into()); }
        for art in [&game.cover, &game.banner] { crate::artwork::validate_reference(art)?; }
    }
    let s = &library.settings;
    if !(130..=280).contains(&s.size) || !(8..=40).contains(&s.gap) || s.radius > 30 || s.blur > 20 || s.dim > 85 || s.folders.len() > 100 || !["cream", "navy", "cyan"].contains(&s.theme.as_str()) || !["poster", "banner", "list", "carousel"].contains(&s.view.as_str()) || !["keep", "minimize", "close"].contains(&s.after_launch.as_str()) { return Err("Invalid layout or launch settings".into()); }
    if s.accent.len() != 7 || !s.accent.starts_with('#') || !s.accent[1..].bytes().all(|b| b.is_ascii_hexdigit()) { return Err("Invalid accent color".into()); }
    if !s.wallpaper.is_empty() && !WALLPAPER_PRESETS.contains(&s.wallpaper.as_str()) { crate::artwork::validate_reference(&s.wallpaper)?; }
    validate_integrations(s)
}
fn validate_integrations(s: &crate::models::Settings) -> Result<(), String> {
    if !(1..=50).contains(&s.backup_keep) { return Err("Keep between 1 and 50 backups".into()); }
    if s.backup_dir.len() > 1000 || s.backup_dir.contains('\0') || (!s.backup_dir.is_empty() && !Path::new(&s.backup_dir).is_absolute()) { return Err("Choose an absolute backup folder".into()); }
    if !s.discord_app_id.is_empty() && !((17..=20).contains(&s.discord_app_id.len()) && s.discord_app_id.bytes().all(|b| b.is_ascii_digit())) { return Err("A Discord Application ID is 17 to 20 digits".into()); }
    if s.steam_profile.len() > 200 || s.steam_profile.chars().any(char::is_control) { return Err("Enter a Steam profile link, SteamID64, or custom name".into()); }
    if !s.steam_api_key.is_empty() && !(s.steam_api_key.len() == 32 && s.steam_api_key.bytes().all(|b| b.is_ascii_hexdigit())) { return Err("A Steam Web API key is 32 hexadecimal characters".into()); }
    if !(1..=168).contains(&s.steam_sync_hours) { return Err("Choose a Steam sync interval between 1 hour and 7 days".into()); }
    Ok(())
}
pub fn save(library: &Library) -> Result<(), String> {
    let _guard = WRITE_LOCK.lock().map_err(|_| "Library lock unavailable")?;
    validate(library)?;
    let dir = root()?; let path = dir.join("library.json");
    let bytes = serde_json::to_vec(library).map_err(|e| e.to_string())?;
    if path.exists() { fs::copy(&path, dir.join("library.backup.json")).map_err(|e| format!("Cannot create library backup: {e}"))?; }
    write_atomic(&path, &bytes)
}
/// Write beside the destination, flush to disk, then swap into place so a crash never leaves half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path.parent().ok_or("Invalid destination folder")?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let temp = dir.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4().simple()));
    let result = (|| -> Result<(), String> {
        let mut file = File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?; file.sync_all().map_err(|e| e.to_string())?; drop(file);
        replace(&temp, path)
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result
}
#[cfg(windows)]
fn replace(from: &Path, to: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{core::PCWSTR, Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH}};
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe { MoveFileExW(PCWSTR(from.as_ptr()), PCWSTR(to.as_ptr()), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) }.map_err(|e| e.to_string())
}
#[cfg(not(windows))]
fn replace(from: &Path, to: &Path) -> Result<(), String> { fs::rename(from, to).map_err(|e| e.to_string()) }

/// If `value` points inside `from`, return the same file inside `to`.
pub fn rebase_reference(value: &str, from: &Path, to: &Path) -> Option<String> {
    if value.is_empty() || value.starts_with("https:") || !util::path_starts_with(Path::new(value), from) { return None; }
    let clean = util::strip_verbatim(value);
    let prefix = util::strip_verbatim(&from.to_string_lossy());
    let rest = clean.get(prefix.trim_end_matches(['\\', '/']).len()..)?.trim_start_matches(['\\', '/']);
    Some(format!("{}{}{rest}", util::strip_verbatim(&to.to_string_lossy()).trim_end_matches(['\\', '/']), std::path::MAIN_SEPARATOR))
}
/// One-time copy of a library created by an earlier release (different identifier) into the current data
/// folder. Only the covers the library actually references are copied; nothing is moved or deleted.
pub fn migrate_legacy() -> Result<Option<usize>, String> {
    let base = data_base()?;
    let new_root = base.join(APP_ID);
    if new_root.join("library.json").exists() { return Ok(None); }
    for legacy in LEGACY_APP_IDS {
        let old_root = base.join(legacy);
        let old_library = old_root.join("library.json");
        if !old_library.is_file() { continue; }
        let _guard = WRITE_LOCK.lock().map_err(|_| "Library lock unavailable")?;
        fs::create_dir_all(new_root.join("covers")).map_err(|e| e.to_string())?;
        let bytes = fs::read(&old_library).map_err(|e| format!("Cannot read the previous library: {e}"))?;
        let mut library: Library = serde_json::from_slice(&bytes).map_err(|e| format!("The previous library could not be read, so it was not migrated: {e}"))?;
        let (old_covers, new_covers) = (old_root.join("covers"), new_root.join("covers"));
        let mut copied = 0;
        let mut carry = |value: &mut String| {
            if let Some(next) = rebase_reference(value, &old_covers, &new_covers) {
                if let (Some(name), true) = (Path::new(value.as_str()).file_name(), Path::new(value.as_str()).is_file()) {
                    if fs::copy(value.as_str(), new_covers.join(name)).is_ok() { copied += 1; *value = next; return; }
                }
                value.clear(); // the original file is gone; fall back to Steam art or the title card
            }
        };
        for game in &mut library.games { carry(&mut game.cover); carry(&mut game.banner); }
        carry(&mut library.settings.wallpaper);
        let backup = old_root.join("library.backup.json");
        if backup.is_file() { let _ = fs::copy(&backup, new_root.join("library.backup.json")); }
        write_atomic(&new_root.join("library.json"), &serde_json::to_vec(&library).map_err(|e| e.to_string())?)?;
        return Ok(Some(copied));
    }
    Ok(None)
}

#[cfg(test)]
pub mod testutil {
    use std::sync::{Mutex, MutexGuard};
    static ENV: Mutex<()> = Mutex::new(());
    /// Tests that touch the data folder run one at a time inside their own temporary directory.
    pub struct Sandbox { pub dir: std::path::PathBuf, _guard: MutexGuard<'static, ()> }
    impl Sandbox {
        pub fn new() -> Self {
            let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
            let dir = std::env::temp_dir().join(format!("aether-test-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&dir).unwrap();
            std::env::set_var("AETHER_DATA_DIR", &dir);
            Sandbox { dir, _guard: guard }
        }
    }
    impl Drop for Sandbox { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Game;
    #[test] fn identity_ignores_verbatim_prefix_case_and_slashes() {
        assert_eq!(identity(r"\\?\C:\Games\Hades.exe", " -x "), identity("c:/games/hades.exe", "-x"));
        assert_ne!(identity("a.exe", "-x"), identity("a.exe", "-y"));
    }
    #[test] fn references_are_rebased_only_inside_the_old_folder() {
        let (from, to) = (Path::new(r"C:\R\com.aetherhub.desktop\covers"), Path::new(r"C:\R\com.aetherhub.launcher\covers"));
        let got = rebase_reference(r"C:\R\com.aetherhub.desktop\covers\a.webp", from, to).unwrap();
        assert!(got.starts_with(r"C:\R\com.aetherhub.launcher\covers") && got.ends_with("a.webp"), "{got}");
        assert!(rebase_reference("https://cdn.akamai.steamstatic.com/x.jpg", from, to).is_none());
        assert!(rebase_reference(r"C:\Elsewhere\a.webp", from, to).is_none());
        assert!(rebase_reference("", from, to).is_none());
    }
    #[test] fn integration_settings_are_validated() {
        let mut s = crate::models::Settings::default();
        assert!(validate_integrations(&s).is_ok());
        s.discord_app_id = "123".into(); assert!(validate_integrations(&s).is_err());
        s.discord_app_id = "123456789012345678".into(); assert!(validate_integrations(&s).is_ok());
        s.steam_api_key = "not-a-key".into(); assert!(validate_integrations(&s).is_err());
        s.steam_api_key = "0123456789abcdef0123456789ABCDEF".into(); assert!(validate_integrations(&s).is_ok());
        s.backup_dir = "relative/path".into(); assert!(validate_integrations(&s).is_err());
        s.backup_dir = String::new(); s.backup_keep = 0; assert!(validate_integrations(&s).is_err());
        s.backup_keep = 10; s.steam_sync_hours = 0; assert!(validate_integrations(&s).is_err());
    }
    #[test] fn atomic_writes_replace_and_leave_no_temp_files() {
        let sandbox = testutil::Sandbox::new();
        let target = sandbox.dir.join("x.json");
        write_atomic(&target, b"one").unwrap(); write_atomic(&target, b"two").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"two");
        assert_eq!(fs::read_dir(&sandbox.dir).unwrap().count(), 1);
    }
    #[test] fn legacy_library_is_migrated_with_its_covers_and_original_is_kept() {
        let sandbox = testutil::Sandbox::new();
        let old = sandbox.dir.join("com.aetherhub.desktop");
        fs::create_dir_all(old.join("covers")).unwrap();
        let cover = old.join("covers").join("abc.webp"); fs::write(&cover, b"img").unwrap();
        let mut library = Library::default();
        let mut game = Game::new("Hades".into(), "Custom", r"C:\Games\Hades.exe".into());
        game.cover = cover.to_string_lossy().into_owned();
        let mut ghost = Game::new("Ghost".into(), "Custom", r"C:\Games\Ghost.exe".into());
        ghost.cover = old.join("covers").join("missing.webp").to_string_lossy().into_owned();
        library.games = vec![game, ghost];
        fs::write(old.join("library.json"), serde_json::to_vec(&library).unwrap()).unwrap();
        assert_eq!(migrate_legacy().unwrap(), Some(1));
        let migrated = load().unwrap();
        let new_covers = root().unwrap().join("covers");
        assert!(Path::new(&migrated.games[0].cover).starts_with(&new_covers) && Path::new(&migrated.games[0].cover).is_file());
        assert!(migrated.games[1].cover.is_empty(), "references to files that no longer exist are cleared");
        assert!(old.join("library.json").is_file() && cover.is_file(), "the legacy folder is left untouched");
        assert_eq!(migrate_legacy().unwrap(), None, "migration runs once");
    }
}
