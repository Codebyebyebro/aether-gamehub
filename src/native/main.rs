#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod models;
mod util;
mod storage;
mod scanner;
mod scan_control;
mod artwork;
mod metadata;
mod launch;
mod shortcuts;
mod playtime;
mod session;
mod discord;
mod steam_sync;
mod backup;
use models::{Library, ScanResult};
use tauri::{Emitter, Manager};
use std::{path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, Mutex}, time::Duration};
static LAUNCHING: AtomicBool = AtomicBool::new(false);
static ART_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(3);
static SCAN_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
static SCANS: scan_control::Registry = scan_control::Registry::new();
/// Something the player should be told about once the interface is up (for example a failed data migration).
static STARTUP_NOTICE: Mutex<Option<String>> = Mutex::new(None);

#[tauri::command]
async fn load_library() -> Result<Library, String> {
    tauri::async_runtime::spawn_blocking(storage::load).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn save_library(library: Library) -> Result<(), String> {
    let (discord_on, discord_id) = (library.settings.discord_enabled, library.settings.discord_app_id.clone());
    tauri::async_runtime::spawn_blocking(move || storage::save(&library)).await.map_err(|e| e.to_string())??;
    discord::configure(discord_on, &discord_id);
    backup::note_change();
    Ok(())
}
#[tauri::command]
async fn startup_notice() -> Option<String> { STARTUP_NOTICE.lock().ok().and_then(|mut n| n.take()) }
#[tauri::command]
async fn load_playtime() -> Result<playtime::Store, String> {
    tauri::async_runtime::spawn_blocking(playtime::load).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn pick_file(kind: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = rfd::FileDialog::new();
        let path = match kind.as_str() {
            "exe" => dialog.set_title("Choose a game executable").add_filter("Windows executable", &["exe"]).pick_file(),
            "image" => dialog.set_title("Choose artwork").add_filter("Artwork", &["png", "jpg", "jpeg", "webp"]).pick_file(),
            "folder" => dialog.set_title("Choose a folder").pick_folder(),
            _ => return Err("Unsupported picker type".into()),
        };
        Ok(path.map(|p| util::clean_path(&p)))
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn import_artwork(path: String) -> Result<String, String> {
    let _permit = ART_JOBS.acquire().await.map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || artwork::import(&path)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn resolve_artwork(url: String) -> Result<String, String> {
    let _permit = ART_JOBS.acquire().await.map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || artwork::resolve(&url)).await.map_err(|e| e.to_string())?
}
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanProgressEvent { scan_id: String, step: u32, steps: u32, checked: u64, found: u32, folder: String }
/// Runs one scan, reporting `aether:scan-progress` events tagged with `scan_id` until it finishes or is cancelled.
#[tauri::command]
async fn scan_games(app: tauri::AppHandle, folders: Vec<String>, scan_id: String) -> Result<ScanResult, String> {
    uuid::Uuid::parse_str(&scan_id).map_err(|_| "Invalid scan identifier")?;
    let cancel = SCANS.begin(&scan_id);
    // A cancelled scan stops at its next file, but one slow folder read can hold it a moment: wait instead of refusing.
    let Ok(Ok(_permit)) = tokio::time::timeout(Duration::from_secs(5), SCAN_JOBS.acquire()).await else {
        SCANS.finish(&scan_id);
        return Err("A scan is already running. Wait for it to finish before starting another.".into());
    };
    let id = scan_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut watch = scan_control::Watch::new(cancel, Duration::from_millis(150), |p: &scan_control::Progress| {
            let _ = app.emit("aether:scan-progress", ScanProgressEvent { scan_id: id.clone(), step: p.step, steps: p.steps, checked: p.checked, found: p.found, folder: p.folder.clone() });
        });
        scanner::scan(folders, &mut watch)
    }).await.map_err(|e| e.to_string());
    SCANS.finish(&scan_id);
    result?
}
/// Asks the scan with this id to stop. It returns a result marked `cancelled` shortly afterwards.
#[tauri::command]
async fn cancel_scan(scan_id: String) -> Result<(), String> {
    uuid::Uuid::parse_str(&scan_id).map_err(|_| "Invalid scan identifier")?;
    SCANS.cancel(&scan_id);
    Ok(())
}
#[tauri::command]
async fn launch_game(app: tauri::AppHandle, id: String) -> Result<(), String> {
    uuid::Uuid::parse_str(&id).map_err(|_| "Invalid game identifier")?;
    if LAUNCHING.swap(true, Ordering::SeqCst) { return Err("A game launch is already in progress".into()); }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let library = storage::load()?;
        let game = library.games.iter().find(|g| g.id == id).ok_or("Game is no longer in your library")?.clone();
        let handoff = launch::handoff(&game)?;
        Ok::<_, String>((library.settings, game, handoff))
    }).await.map_err(|e| e.to_string()).and_then(|r| r);
    LAUNCHING.store(false, Ordering::SeqCst);
    let (settings, game, handoff) = result?;
    match settings.after_launch.as_str() {
        // Nothing of the launcher stays running, so there is nothing left to track or to show on Discord.
        "close" => app.exit(0),
        mode => {
            session::start(app.clone(), game, handoff, &settings);
            if mode == "minimize" { if let Some(window) = app.get_webview_window("main") { window.minimize().map_err(|e| e.to_string())?; } }
        }
    }
    Ok(())
}
#[tauri::command]
async fn export_shortcuts() -> Result<Option<usize>, String> {
    tauri::async_runtime::spawn_blocking(shortcuts::export).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn grid_artwork(title: String, key: String) -> Result<Vec<metadata::Match>, String> {
    tauri::async_runtime::spawn_blocking(move || metadata::search(title, key)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn backup_overview(dir: String) -> Result<backup::Overview, String> {
    tauri::async_runtime::spawn_blocking(move || backup::overview(&dir)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn backup_now(include_covers: bool, dir: String, keep: u32) -> Result<backup::Made, String> {
    tauri::async_runtime::spawn_blocking(move || backup::backup_with(&dir, keep, include_covers)).await.map_err(|e| e.to_string())?
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Exported { path: String, warnings: Vec<String> }
#[tauri::command]
async fn backup_export() -> Result<Option<Exported>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let name = format!("aether-hub-backup-{}.json", util::utc_stamp(util::now_secs()));
        let Some(path) = rfd::FileDialog::new().set_title("Export Aether Hub backup").set_file_name(name).add_filter("Aether Hub backup", &["json"]).save_file() else { return Ok(None); };
        let warnings = backup::export_to(&path)?;
        Ok(Some(Exported { path: util::clean_path(&path), warnings }))
    }).await.map_err(|e| e.to_string())?
}
/// Restore from a backup listed in the backup folder (`path`), or let the player pick any backup file (`None`).
#[tauri::command]
async fn backup_restore(path: Option<String>) -> Result<Option<backup::Restored>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let chosen = match path {
            Some(listed) => {
                let listed = PathBuf::from(listed);
                if !backup::is_listed(&storage::load()?.settings.backup_dir, &listed) { return Err("Choose a backup from the list, or use “Restore from file…”".to_string()); }
                listed
            }
            None => match rfd::FileDialog::new().set_title("Restore an Aether Hub backup").add_filter("Aether Hub backup", &["json"]).pick_file() { Some(file) => file, None => return Ok(None) },
        };
        backup::restore(&chosen).map(Some)
    }).await.map_err(|e| e.to_string())?
}
fn open_folder(path: &Path) -> Result<(), String> {
    #[cfg(windows)] let program = "explorer.exe";
    #[cfg(not(windows))] let program = "xdg-open";
    std::process::Command::new(program).arg(path).spawn().map(|_| ()).map_err(|e| format!("Could not open the folder: {e}"))
}
#[tauri::command]
async fn open_backup_folder(dir: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_folder(Path::new(&backup::overview(&dir)?.dir))).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn discord_status() -> discord::Status { discord::status() }
#[tauri::command]
async fn discord_test(app_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || discord::test(&app_id)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn steam_sync_now(app: tauri::AppHandle, profile: String, api_key: String) -> Result<steam_sync::Summary, String> {
    let summary = tauri::async_runtime::spawn_blocking(move || steam_sync::sync_now(&profile, &api_key)).await.map_err(|e| e.to_string())?;
    let _ = app.emit("aether:playtime-reload", ()); // refresh the interface on success and failure alike (the failure is recorded)
    summary
}

fn main() {
    // First start after the app identifier changed: carry the previous library over before anything reads it.
    match storage::migrate_legacy() {
        Ok(_) => {}
        Err(e) => { if let Ok(mut notice) = STARTUP_NOTICE.lock() { *notice = Some(format!("Your previous library could not be carried over automatically: {e}. It has not been changed or deleted.")); } }
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--launch") {
        let result = (|| -> Result<(), String> {
            if args.len() != 2 { return Err("Shortcut contains invalid launch arguments".into()); }
            uuid::Uuid::parse_str(&args[1]).map_err(|_| "Shortcut game identifier is invalid")?;
            let library = storage::load()?;
            let game = library.games.iter().find(|g| g.id == args[1]).ok_or("This game was removed from Aether Hub. Create a new shortcut after adding it again.")?;
            launch::handoff(game).map(|_| ())
        })();
        if let Err(error) = result { rfd::MessageDialog::new().set_title("Aether Hub — Could not launch").set_description(&error).set_level(rfd::MessageLevel::Error).show(); std::process::exit(1); }
        return;
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") { let _ = window.unminimize(); let _ = window.show(); let _ = window.set_focus(); }
        }))
        .setup(|app| {
            if let Ok(library) = storage::load() { discord::configure(library.settings.discord_enabled, &library.settings.discord_app_id); }
            steam_sync::spawn_scheduler(app.handle().clone());
            backup::on_startup();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![load_library, save_library, pick_file, import_artwork, resolve_artwork, scan_games, cancel_scan, launch_game, export_shortcuts, grid_artwork, startup_notice, load_playtime, backup_overview, backup_now, backup_export, backup_restore, open_backup_folder, discord_status, discord_test, steam_sync_now])
        .build(tauri::generate_context!())
        .expect("Aether Hub could not initialize");
    app.run(|handle, event| {
        // Quitting while a game is still running must not lose that session or leave a stale status on Discord.
        if let tauri::RunEvent::Exit = event { session::flush_all(handle); discord::shutdown(); }
    });
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    fn registered() -> BTreeSet<String> {
        let source = include_str!("main.rs");
        let marker = concat!("generate_handler", "![");
        let start = source.find(marker).expect("the handler list") + marker.len();
        source[start..start + source[start..].find(']').unwrap()].split(',').map(|c| c.trim().to_owned()).filter(|c| !c.is_empty()).collect()
    }
    fn allowed() -> BTreeSet<String> {
        let toml = include_str!("../../src-tauri/permissions/library.toml");
        let start = toml.find("commands.allow").and_then(|i| toml[i..].find('[').map(|j| i + j + 1)).expect("commands.allow list");
        toml[start..start + toml[start..].find(']').unwrap()].split(',').map(|c| c.trim().trim_matches('"').to_owned()).filter(|c| !c.is_empty()).collect()
    }
    /// A command missing from `permissions/library.toml` compiles and works under `tauri dev`, but the release build
    /// rejects it at runtime ("not allowed by ACL"). This keeps the two lists identical.
    #[test] fn the_permission_set_lists_exactly_the_registered_commands() {
        let (registered, allowed) = (registered(), allowed());
        assert!(registered.len() >= 19, "{registered:?}");
        assert_eq!(registered.difference(&allowed).collect::<Vec<_>>(), Vec::<&String>::new(), "registered but not allowed");
        assert_eq!(allowed.difference(&registered).collect::<Vec<_>>(), Vec::<&String>::new(), "allowed but not registered");
    }
    #[test] fn the_window_capability_grants_the_permission_set_and_nothing_broad() {
        let capability = include_str!("../../src-tauri/capabilities/main.json");
        assert!(capability.contains("\"library-commands\"") && capability.contains("\"windows\": [\"main\"]"));
        for broad in ["fs:", "shell:", "http:", "dialog:", "opener:"] { assert!(!capability.contains(broad), "{broad}"); }
    }
}
