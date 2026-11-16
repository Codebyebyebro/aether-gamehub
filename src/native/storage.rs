use crate::models::Library;
use std::{collections::HashSet, fs::{self, File}, io::Write, path::PathBuf, sync::Mutex};
pub const APP_ID: &str = "com.aetherhub.desktop";
pub static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn root() -> Result<PathBuf, String> {
    let p = dirs::data_dir().ok_or("Windows application data directory is unavailable")?.join(APP_ID);
    fs::create_dir_all(p.join("covers")).map_err(|e| e.to_string())?;
    Ok(p)
}
pub fn load() -> Result<Library, String> {
    let path = root()?.join("library.json");
    if !path.exists() { return Ok(Library::default()); }
    let bytes = fs::read(&path).map_err(|e| format!("Cannot read library: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Cannot parse library: {e}. Your library and backup files have not been overwritten."))
}
pub fn save(library: &Library) -> Result<(), String> {
    let _guard = WRITE_LOCK.lock().map_err(|_| "Library lock unavailable")?;
    if library.games.len() > 20000 { return Err("The library exceeds the 20,000-entry safety limit".into()); }
    let mut ids = HashSet::new(); let mut targets = HashSet::new();
    for game in &library.games {
        uuid::Uuid::parse_str(&game.id).map_err(|_| "Invalid game identifier")?;
        if game.title.trim().is_empty() || game.title.len() > 640 || game.target.is_empty() || game.target.len() > 32760 || game.args.len() > 16000 || game.target.contains('\0') || game.args.contains('\0') || game.working_dir.contains('\0') { return Err("Invalid title, launch target, or launch arguments".into()); }
        if !["Steam", "Epic", "Riot", "Custom"].contains(&game.platform.as_str()) { return Err("Unsupported platform".into()); }
        if !ids.insert(&game.id) || !targets.insert(format!("{}|{}", game.target.replace('/', "\\").to_lowercase(), game.args.trim())) { return Err("Duplicate game identifier or launch target".into()); }
        crate::launch::validate_target(&game.target)?;
        if game.target.contains("://") && !game.args.trim().is_empty() { return Err("Configure Steam/Epic launch arguments in the storefront client, or use a direct executable target".into()); }
        for art in [&game.cover, &game.banner] { crate::artwork::validate_reference(art)?; }
    }
    let s = &library.settings;
    if !(130..=280).contains(&s.size) || !(8..=40).contains(&s.gap) || s.radius > 30 || s.blur > 20 || s.dim > 85 || s.folders.len() > 100 || !["cream", "navy", "cyan"].contains(&s.theme.as_str()) || !["poster", "banner", "list", "carousel"].contains(&s.view.as_str()) || !["keep", "minimize", "close"].contains(&s.after_launch.as_str()) { return Err("Invalid layout or launch settings".into()); }
    if s.accent.len() != 7 || !s.accent.starts_with('#') || !s.accent[1..].bytes().all(|b| b.is_ascii_hexdigit()) { return Err("Invalid accent color".into()); }
    if !s.wallpaper.is_empty() && !["/orbit-wallpaper.svg", "/orbit-navy-wallpaper.svg", "/orbit-cyan-wallpaper.svg"].contains(&s.wallpaper.as_str()) { crate::artwork::validate_reference(&s.wallpaper)?; }
    let dir = root()?; let path = dir.join("library.json"); let temp = dir.join("library.next.json");
    let bytes = serde_json::to_vec(library).map_err(|e| e.to_string())?;
    let mut file = File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?; file.sync_all().map_err(|e| e.to_string())?; drop(file);
    if path.exists() { fs::copy(&path, dir.join("library.backup.json")).map_err(|e| format!("Cannot create library backup: {e}"))?; }
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        use windows::{core::PCWSTR, Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH}};
        let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe { MoveFileExW(PCWSTR(from.as_ptr()), PCWSTR(to.as_ptr()), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) }.map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))] fs::rename(temp, path).map_err(|e| e.to_string())?;
    Ok(())
}
