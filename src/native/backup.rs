//! Library backup. A backup is one JSON file holding the library, the playtime history and (optionally) the custom
//! cover images, so it can be kept in a OneDrive / Dropbox / Google Drive folder or copied to another PC.
//! Aether Hub has no server: "cloud" here means whatever folder the player chooses, synced by their own provider.
use crate::{models::Library, playtime, storage, util};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, Mutex}, time::Duration};

pub const FORMAT: &str = "aether-hub-backup";
const VERSION: u32 = 1;
const PREFIX: &str = "aether-backup-";
const BEFORE_RESTORE: &str = "-before-restore";
const AUTO_EVERY_SECS: u64 = 30 * 60;
const STARTUP_IF_OLDER_SECS: u64 = 12 * 3600;
const MAX_BUNDLE_BYTES: u64 = 192 * 1024 * 1024;
const MAX_EMBED_FILE: u64 = 3 * 1024 * 1024;
const MAX_EMBED_TOTAL: u64 = 64 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    pub format: String,
    pub version: u32,
    pub created_at: u64,
    pub app_version: String,
    pub library: Library,
    pub playtime: playtime::Store,
    /// Cover file name -> base64 data. Present only when the backup was made with "include cover images".
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub covers: HashMap<String, String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo { pub name: String, pub path: String, pub size_bytes: u64, pub created_at: u64 }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview { pub dir: String, pub backups: Vec<BackupInfo>, pub last_error: String }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Made { pub info: BackupInfo, pub warnings: Vec<String> }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Restored { pub library: Library, pub playtime: playtime::Store, pub games: u32, pub restored_covers: u32, pub missing_covers: u32 }

static LAST_ERROR: Mutex<String> = Mutex::new(String::new());
static PENDING: AtomicBool = AtomicBool::new(false);
static WORKER: AtomicBool = AtomicBool::new(false);
fn set_error(message: &str) { if let Ok(mut e) = LAST_ERROR.lock() { *e = message.to_owned(); } }

/// Only plain image file names are ever read from or written into the covers folder.
fn safe_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (1..=100).contains(&name.len()) && !name.contains("..")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        && ["webp", "png", "jpg", "jpeg"].iter().any(|e| lower.ends_with(&format!(".{e}")))
}
/// Every artwork reference in a library: covers, banners and the wallpaper.
fn references(library: &mut Library) -> impl Iterator<Item = &mut String> + '_ {
    library.games.iter_mut().flat_map(|g| [&mut g.cover, &mut g.banner]).chain(std::iter::once(&mut library.settings.wallpaper))
}
/// Rewrite references into the covers folder as `covers/<file>` (portable between PCs). Returns the file names.
fn to_portable(library: &mut Library, covers: &Path) -> Vec<String> {
    let mut names = vec![];
    for value in references(library) {
        if value.is_empty() || value.starts_with("https:") || !util::path_starts_with(Path::new(value.as_str()), covers) { continue; }
        if let Some(name) = Path::new(value.as_str()).file_name().and_then(|n| n.to_str()).map(str::to_owned) { *value = format!("covers/{name}"); names.push(name); }
    }
    names
}
/// Resolve `covers/<file>` back to this PC. A reference to an image that is not here is cleared (the interface then
/// falls back to Steam art or the title card) rather than failing the whole restore. Returns how many were cleared.
fn from_portable(library: &mut Library, covers: &Path) -> u32 {
    let mut cleared = 0;
    for value in references(library) {
        if value.is_empty() || value.starts_with("https:") || storage::WALLPAPER_PRESETS.contains(&value.as_str()) { continue; }
        let resolved = match value.strip_prefix("covers/") { Some(name) => Some(name).filter(|n| safe_name(n)).map(|n| covers.join(n)), None => Some(PathBuf::from(value.as_str())) };
        match resolved {
            Some(path) if path.is_file() && util::path_starts_with(&path, covers) => *value = path.to_string_lossy().into_owned(),
            _ => { value.clear(); cleared += 1; }
        }
    }
    cleared
}
pub fn create_bundle(include_covers: bool) -> Result<(Bundle, Vec<String>), String> {
    let mut library = storage::load()?;
    library.settings.steam_api_key.clear(); // a key is a credential: it is never written into a backup
    let covers_dir = storage::root()?.join("covers");
    let names = to_portable(&mut library, &covers_dir);
    let (mut covers, mut warnings) = (HashMap::new(), vec![]);
    if include_covers {
        let (mut total, mut seen) = (0u64, HashSet::new());
        for name in names.into_iter().filter(|n| seen.insert(n.clone())) {
            if !safe_name(&name) { continue; }
            let path = covers_dir.join(&name);
            let Ok(meta) = fs::metadata(&path) else { warnings.push(format!("A cover image is missing: {name}")); continue; };
            if meta.len() > MAX_EMBED_FILE || total + meta.len() > MAX_EMBED_TOTAL { warnings.push(format!("{name} was left out to keep the backup small")); continue; }
            match fs::read(&path) { Ok(bytes) => { total += meta.len(); covers.insert(name, B64.encode(bytes)); } Err(e) => warnings.push(format!("{name} could not be read: {e}")) }
        }
    }
    if warnings.len() > 5 { let more = warnings.len() - 5; warnings.truncate(5); warnings.push(format!("…and {more} more")); }
    let bundle = Bundle { format: FORMAT.into(), version: VERSION, created_at: util::now_secs(), app_version: env!("CARGO_PKG_VERSION").into(), library, playtime: playtime::load()?, covers };
    Ok((bundle, warnings))
}
fn resolve_dir(configured: &str) -> Result<PathBuf, String> {
    let dir = if configured.trim().is_empty() { storage::root()?.join("backups") } else { PathBuf::from(configured.trim()) };
    if !dir.is_absolute() { return Err("Choose an absolute backup folder".into()); }
    fs::create_dir_all(&dir).map_err(|e| format!("The backup folder is not usable: {e}"))?;
    Ok(dir)
}
/// Backup file names in `dir`, oldest first (the timestamp in the name sorts chronologically).
fn names_in(dir: &Path, snapshots: bool) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(PREFIX) && n.ends_with(".json") && (snapshots || !n.contains(BEFORE_RESTORE))).collect();
    names.sort(); names
}
fn info(path: &Path) -> Option<BackupInfo> {
    let meta = fs::metadata(path).ok()?;
    let created = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    Some(BackupInfo { name: path.file_name()?.to_string_lossy().into_owned(), path: util::clean_path(path), size_bytes: meta.len(), created_at: created })
}
fn write_bundle(dir: &Path, bundle: &Bundle, suffix: &str) -> Result<PathBuf, String> {
    let bytes = serde_json::to_vec_pretty(bundle).map_err(|e| e.to_string())?;
    let stamp = util::utc_stamp(bundle.created_at);
    for n in 0..100 {
        let name = if n == 0 { format!("{PREFIX}{stamp}{suffix}.json") } else { format!("{PREFIX}{stamp}{suffix}-{n}.json") };
        let path = dir.join(name);
        if !path.exists() { storage::write_atomic(&path, &bytes)?; return Ok(path); }
    }
    Err("Too many backups were made in the same second".into())
}
/// Keep the newest `keep` backups; safety snapshots taken before a restore are kept separately (newest 3).
fn rotate(dir: &Path, keep: usize) {
    let snapshots: Vec<String> = names_in(dir, true).into_iter().filter(|n| n.contains(BEFORE_RESTORE)).collect();
    for (names, limit) in [(names_in(dir, false), keep), (snapshots, 3)] {
        for name in names.iter().take(names.len().saturating_sub(limit)) { let _ = fs::remove_file(dir.join(name)); }
    }
}
pub fn backup_with(dir_setting: &str, keep: u32, include_covers: bool) -> Result<Made, String> {
    let dir = resolve_dir(dir_setting)?;
    let (bundle, warnings) = create_bundle(include_covers)?;
    let path = write_bundle(&dir, &bundle, "")?;
    rotate(&dir, keep.clamp(1, 50) as usize);
    set_error("");
    Ok(Made { info: info(&path).ok_or("The backup could not be read back")?, warnings })
}
pub fn overview(dir_setting: &str) -> Result<Overview, String> {
    let dir = resolve_dir(dir_setting)?;
    let mut backups: Vec<BackupInfo> = names_in(&dir, true).iter().filter_map(|n| info(&dir.join(n))).collect();
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.name.cmp(&a.name)));
    Ok(Overview { dir: util::clean_path(&dir), backups, last_error: LAST_ERROR.lock().map(|e| e.clone()).unwrap_or_default() })
}
/// Write a self-contained copy (with cover images) wherever the player chose.
pub fn export_to(path: &Path) -> Result<Vec<String>, String> {
    let (bundle, warnings) = create_bundle(true)?;
    storage::write_atomic(path, &serde_json::to_vec_pretty(&bundle).map_err(|e| e.to_string())?)?;
    Ok(warnings)
}
/// Is `path` one of the backups in the configured backup folder? (Used before restoring a file picked from the list.)
pub fn is_listed(dir_setting: &str, path: &Path) -> bool {
    resolve_dir(dir_setting).is_ok_and(|dir| util::path_starts_with(path, &dir) && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(PREFIX) && n.ends_with(".json")))
}
pub fn restore(path: &Path) -> Result<Restored, String> {
    let size = fs::metadata(path).map_err(|e| format!("Cannot read the backup: {e}"))?.len();
    if size > MAX_BUNDLE_BYTES { return Err("This file is too large to be an Aether Hub backup".into()); }
    let bundle: Bundle = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|_| "This file is not an Aether Hub backup".to_string())?;
    if bundle.format != FORMAT { return Err("This file is not an Aether Hub backup".into()); }
    if bundle.version > VERSION { return Err("This backup was made by a newer version of Aether Hub. Update the app to restore it.".into()); }
    // Safety net: what is about to be replaced is saved first, so a mistaken restore can be undone from the list.
    let current = storage::load()?;
    let dir = resolve_dir(&current.settings.backup_dir)?;
    let (snapshot, _) = create_bundle(false)?;
    write_bundle(&dir, &snapshot, BEFORE_RESTORE)?; rotate(&dir, 50);
    let covers_dir = storage::root()?.join("covers");
    let mut restored_covers = 0;
    for (name, data) in &bundle.covers {
        let target = covers_dir.join(name);
        if !safe_name(name) || target.exists() { continue; }
        if let Ok(bytes) = B64.decode(data) { if bytes.len() as u64 <= MAX_EMBED_FILE && storage::write_atomic(&target, &bytes).is_ok() { restored_covers += 1; } }
    }
    let mut library = bundle.library;
    let missing_covers = from_portable(&mut library, &covers_dir);
    // Machine-specific values stay as they are on this PC.
    library.settings.steam_api_key = current.settings.steam_api_key;
    library.settings.backup_dir = current.settings.backup_dir;
    storage::save(&library)?;
    playtime::replace(bundle.playtime)?;
    Ok(Restored { games: library.games.len() as u32, library, playtime: playtime::load()?, restored_covers, missing_covers })
}

fn newest_age(dir: &Path) -> Option<u64> {
    let newest = names_in(dir, false).into_iter().filter_map(|n| info(&dir.join(n))).map(|i| i.created_at).max()?;
    Some(util::now_secs().saturating_sub(newest))
}
/// Note that something worth backing up changed. Bursts of edits are coalesced into one backup, and automatic
/// backups are at most one every 30 minutes, so a busy session cannot push older history out of the rotation.
pub fn note_change() {
    PENDING.store(true, Ordering::SeqCst);
    if WORKER.swap(true, Ordering::SeqCst) { return; }
    tauri::async_runtime::spawn(async {
        loop {
            let wait = tauri::async_runtime::spawn_blocking(wait_secs).await.unwrap_or(60);
            tokio::time::sleep(Duration::from_secs(wait)).await;
            let _ = tauri::async_runtime::spawn_blocking(run_pending).await;
            WORKER.store(false, Ordering::SeqCst);
            if PENDING.load(Ordering::SeqCst) && !WORKER.swap(true, Ordering::SeqCst) { continue; }
            break;
        }
    });
}
fn wait_secs() -> u64 {
    let setting = storage::load().map(|l| l.settings.backup_dir).unwrap_or_default();
    match resolve_dir(&setting).ok().and_then(|d| newest_age(&d)) { Some(age) if age < AUTO_EVERY_SECS => AUTO_EVERY_SECS - age + 5, _ => 45 }
}
fn run_pending() {
    if !PENDING.swap(false, Ordering::SeqCst) { return; }
    let Ok(library) = storage::load() else { return; };
    if !library.settings.backup_enabled { return; }
    if let Err(e) = backup_with(&library.settings.backup_dir, library.settings.backup_keep, false) { set_error(&e); }
}
/// One backup shortly after start-up if the newest is old, so a library is protected even before its first edit.
pub fn on_startup() {
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let _ = tauri::async_runtime::spawn_blocking(|| {
            let Ok(library) = storage::load() else { return; };
            let s = &library.settings;
            if !s.backup_enabled || library.games.is_empty() { return; }
            let Ok(dir) = resolve_dir(&s.backup_dir) else { return; };
            if newest_age(&dir).is_none_or(|age| age >= STARTUP_IF_OLDER_SECS) {
                if let Err(e) = backup_with(&s.backup_dir, s.backup_keep, false) { set_error(&e); }
            }
        }).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Game;
    const KEY: &str = "0123456789abcdef0123456789abcdef";
    /// A saved library with a managed cover, a banner from Steam, a preset wallpaper and a secret.
    fn seed(sandbox: &storage::testutil::Sandbox) -> (Library, PathBuf) {
        let covers = storage::root().unwrap().join("covers");
        let cover = covers.join("aaa.webp"); fs::write(&cover, b"RIFF-fake-webp").unwrap();
        let mut hades = Game::new("Hades".into(), "Custom", "/games/hades.exe".into());
        hades.cover = cover.to_string_lossy().into_owned();
        let mut elden = Game::new("ELDEN RING".into(), "Steam", "steam://rungameid/1245620".into()); elden.steam_art("1245620");
        let mut library = Library { games: vec![hades.clone(), elden], settings: Default::default() };
        library.settings.wallpaper = "/orbit-wallpaper.svg".into();
        library.settings.steam_api_key = KEY.into();
        library.settings.backup_dir = sandbox.dir.join("my-backups").to_string_lossy().into_owned();
        storage::save(&library).unwrap();
        playtime::add_session(&hades.id, 3600, 1_700_000_000).unwrap();
        (library, cover)
    }
    #[test] fn backups_are_portable_complete_and_keep_secrets_out() {
        let sandbox = storage::testutil::Sandbox::new();
        let (library, _) = seed(&sandbox);
        let (bundle, warnings) = create_bundle(true).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(bundle.library.games[0].cover, "covers/aaa.webp", "references are relative, so they survive a move to another PC");
        assert!(bundle.library.games[1].cover.starts_with("https://") && bundle.library.settings.wallpaper == "/orbit-wallpaper.svg");
        assert_eq!(B64.decode(&bundle.covers["aaa.webp"]).unwrap(), b"RIFF-fake-webp");
        assert_eq!(bundle.playtime.games[&library.games[0].id].seconds, 3600);
        let text = serde_json::to_string(&bundle).unwrap();
        assert!(!text.contains(KEY) && bundle.library.settings.steam_api_key.is_empty(), "the Steam key must never be exported");
        assert!(create_bundle(false).unwrap().0.covers.is_empty(), "automatic backups carry references only");
    }
    #[test] fn restore_rebuilds_the_library_covers_and_playtime_and_keeps_a_way_back() {
        let sandbox = storage::testutil::Sandbox::new();
        let (library, cover) = seed(&sandbox);
        let file = sandbox.dir.join("export.json");
        export_to(&file).unwrap();
        // Lose everything, and use a different key and folder on this "new PC".
        fs::remove_file(&cover).unwrap();
        let mut empty = Library::default();
        empty.settings.steam_api_key = "ffffffffffffffffffffffffffffffff".into();
        empty.settings.backup_dir = sandbox.dir.join("other-folder").to_string_lossy().into_owned();
        storage::save(&empty).unwrap();
        playtime::replace(playtime::Store::default()).unwrap();
        let restored = restore(&file).unwrap();
        assert_eq!((restored.games, restored.restored_covers, restored.missing_covers), (2, 1, 0));
        let hades = &restored.library.games[0];
        assert!(Path::new(&hades.cover).is_file() && Path::new(&hades.cover).starts_with(storage::root().unwrap().join("covers")));
        assert_eq!(restored.playtime.games[&library.games[0].id].seconds, 3600);
        assert_eq!(restored.library.settings.steam_api_key, "ffffffffffffffffffffffffffffffff", "this PC's key is kept");
        assert!(restored.library.settings.backup_dir.ends_with("other-folder"), "this PC's backup folder is kept");
        assert_eq!(storage::load().unwrap().games.len(), 2);
        let snapshots = names_in(&sandbox.dir.join("other-folder"), true);
        assert!(snapshots.iter().any(|n| n.contains(BEFORE_RESTORE)), "the state that was replaced is saved first: {snapshots:?}");
    }
    #[test] fn a_missing_cover_is_cleared_instead_of_failing_the_restore() {
        let sandbox = storage::testutil::Sandbox::new();
        seed(&sandbox);
        let file = sandbox.dir.join("references-only.json");
        let (bundle, _) = create_bundle(false).unwrap();
        fs::write(&file, serde_json::to_vec(&bundle).unwrap()).unwrap();
        fs::remove_file(storage::root().unwrap().join("covers/aaa.webp")).unwrap();
        let restored = restore(&file).unwrap();
        assert_eq!(restored.missing_covers, 1);
        assert!(restored.library.games[0].cover.is_empty());
    }
    #[test] fn files_that_are_not_backups_are_refused_and_nothing_changes() {
        let sandbox = storage::testutil::Sandbox::new();
        let (library, _) = seed(&sandbox);
        for (name, body) in [("a.json", "not json".to_string()), ("b.json", r#"{"hello":1}"#.to_string())] {
            let file = sandbox.dir.join(name); fs::write(&file, body).unwrap();
            assert!(restore(&file).is_err(), "{name}");
        }
        let (mut bundle, _) = create_bundle(false).unwrap();
        bundle.version = 99; let newer = sandbox.dir.join("c.json"); fs::write(&newer, serde_json::to_vec(&bundle).unwrap()).unwrap();
        assert!(restore(&newer).err().expect("a newer backup is refused").contains("newer version"));
        bundle.version = 1; bundle.format = "something-else".into(); fs::write(&newer, serde_json::to_vec(&bundle).unwrap()).unwrap();
        assert!(restore(&newer).is_err());
        assert_eq!(storage::load().unwrap().games.len(), library.games.len());
    }
    #[test] fn a_hostile_backup_cannot_write_outside_the_covers_folder() {
        let sandbox = storage::testutil::Sandbox::new();
        seed(&sandbox);
        let (mut bundle, _) = create_bundle(false).unwrap();
        for name in ["../escape.webp", "..\\escape.webp", "sub/dir.webp", "evil.exe", "", "a..b.webp"] { bundle.covers.insert(name.into(), B64.encode(b"x")); }
        bundle.covers.insert("fine.webp".into(), B64.encode(b"ok"));
        let file = sandbox.dir.join("hostile.json"); fs::write(&file, serde_json::to_vec(&bundle).unwrap()).unwrap();
        let restored = restore(&file).unwrap();
        assert_eq!(restored.restored_covers, 1);
        assert!(!sandbox.dir.join("escape.webp").exists() && !storage::root().unwrap().join("escape.webp").exists());
    }
    #[test] fn rotation_keeps_the_newest_and_leaves_other_files_alone() {
        let sandbox = storage::testutil::Sandbox::new();
        let dir = sandbox.dir.join("rotation"); fs::create_dir_all(&dir).unwrap();
        for stamp in ["20260101-000000", "20260102-000000", "20260103-000000", "20260104-000000"] { fs::write(dir.join(format!("{PREFIX}{stamp}.json")), "{}").unwrap(); }
        for stamp in ["20260101-000001", "20260102-000001", "20260103-000001", "20260104-000001", "20260105-000001"] { fs::write(dir.join(format!("{PREFIX}{stamp}{BEFORE_RESTORE}.json")), "{}").unwrap(); }
        fs::write(dir.join("notes.txt"), "mine").unwrap(); fs::write(dir.join("photo.json"), "{}").unwrap();
        rotate(&dir, 2);
        assert_eq!(names_in(&dir, false), [format!("{PREFIX}20260103-000000.json"), format!("{PREFIX}20260104-000000.json")]);
        assert_eq!(names_in(&dir, true).iter().filter(|n| n.contains(BEFORE_RESTORE)).count(), 3);
        assert!(dir.join("notes.txt").exists() && dir.join("photo.json").exists());
    }
    #[test] fn only_backups_inside_the_configured_folder_can_be_restored_from_the_list() {
        let sandbox = storage::testutil::Sandbox::new();
        let dir = sandbox.dir.join("b"); fs::create_dir_all(&dir).unwrap();
        let setting = dir.to_string_lossy().into_owned();
        assert!(is_listed(&setting, &dir.join(format!("{PREFIX}20260101-000000.json"))));
        assert!(!is_listed(&setting, &dir.join("passwd")) && !is_listed(&setting, &sandbox.dir.join(format!("{PREFIX}x.json"))));
    }
    #[test] fn backup_names_sort_chronologically_and_never_collide() {
        let sandbox = storage::testutil::Sandbox::new();
        seed(&sandbox);
        let setting = sandbox.dir.join("n").to_string_lossy().into_owned();
        let first = backup_with(&setting, 10, false).unwrap().info; let second = backup_with(&setting, 10, false).unwrap().info;
        assert_ne!(first.path, second.path);
        assert_eq!(overview(&setting).unwrap().backups.len(), 2);
    }
}
