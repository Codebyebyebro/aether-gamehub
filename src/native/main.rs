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
mod behavior;
mod version;
mod updates;
mod autostart;
mod hotkey;
mod padwatch;
mod background;
mod diag;
mod machine;
mod presence;
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
    diag::logged("library", tauri::async_runtime::spawn_blocking(storage::load).await.map_err(|e| e.to_string())?)
}
#[tauri::command]
async fn save_library(app: tauri::AppHandle, library: Library) -> Result<(), String> {
    let (discord_on, discord_id) = (library.settings.discord_enabled, library.settings.discord_app_id.clone());
    let settings = library.settings.clone();
    diag::logged("save", tauri::async_runtime::spawn_blocking(move || storage::save(&library)).await.map_err(|e| e.to_string())?)?;
    discord::configure(discord_on, &discord_id);
    backup::note_change();
    // The tray, shortcut, start-with-Windows and controller wake-up follow the saved settings. Only a switch that moved is acted on.
    tauri::async_runtime::spawn_blocking(move || background::apply(&app, &settings)).await.map_err(|e| e.to_string())?;
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
    diag::logged("scan", result?)
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
    let (settings, game, handoff) = diag::logged("launch", result)?;
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
    diag::logged("backup", tauri::async_runtime::spawn_blocking(move || backup::backup_with(&dir, keep, include_covers)).await.map_err(|e| e.to_string())?)
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
    diag::logged("restore", tauri::async_runtime::spawn_blocking(move || {
        let chosen = match path {
            Some(listed) => {
                let listed = PathBuf::from(listed);
                if !backup::is_listed(&storage::load()?.settings.backup_dir, &listed) { return Err("Choose a backup from the list, or use “Restore from file…”".to_string()); }
                listed
            }
            None => match rfd::FileDialog::new().set_title("Restore an Aether Hub backup").add_filter("Aether Hub backup", &["json"]).pick_file() { Some(file) => file, None => return Ok(None) },
        };
        backup::restore(&chosen).map(Some)
    }).await.map_err(|e| e.to_string())?)
}
fn open_folder(path: &Path) -> Result<(), String> {
    #[cfg(windows)] let program = "explorer.exe";
    #[cfg(not(windows))] let program = "xdg-open";
    std::process::Command::new(program).arg(path).spawn().map(|_| ()).map_err(|e| format!("Could not open the folder: {e}"))
}
/// Opens the releases page in the default browser. The address is fixed in `updates.rs`; the interface cannot supply one.
fn open_releases() -> Result<(), String> {
    #[cfg(windows)] let program = "explorer.exe";
    #[cfg(not(windows))] let program = "xdg-open";
    std::process::Command::new(program).arg(updates::page()).spawn().map(|_| ()).map_err(|e| format!("Could not open the browser: {e}"))
}
#[tauri::command]
async fn open_backup_folder(dir: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_folder(Path::new(&backup::overview(&dir)?.dir))).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn background_status() -> background::Status { background::status() }
/// The text the player copies from Settings when something does not work. It is built here, never by the interface.
#[tauri::command]
async fn diagnostics() -> String {
    tauri::async_runtime::spawn_blocking(build_diagnostics).await.unwrap_or_else(|e| format!("The diagnostics could not be put together: {e}"))
}
/// An error that happened in the interface (a crash in a page, a promise nobody caught), for the local log.
#[tauri::command]
async fn log_interface_error(message: String) { diag::event("interface", &message); }
/// Games whose file can no longer be found, or whose drive is not connected. Only games that point at an .exe on a
/// drive are judged; store links are never reported. Waits at most four seconds, so a slow drive cannot freeze the library.
#[tauri::command]
async fn missing_games() -> Result<Vec<presence::Absent>, String> {
    diag::logged("missing", tauri::async_runtime::spawn_blocking(|| -> Result<Vec<presence::Absent>, String> {
        let games = storage::load()?.games.into_iter().map(|g| (g.id, g.target)).collect();
        Ok(presence::check_all(games, |root| Path::new(root).exists(), |file| Path::new(file).is_file(), Duration::from_secs(4)))
    }).await.map_err(|e| e.to_string())?)
}
/// True once if something (a controller connecting, the tray menu) asked for Big Box since the interface last looked.
#[tauri::command]
async fn take_wake_request() -> bool { background::take_wake_request() }
#[tauri::command]
async fn check_update() -> Result<updates::Update, String> {
    tauri::async_runtime::spawn_blocking(updates::check).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn open_releases_page() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(open_releases).await.map_err(|e| e.to_string())?
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

fn build_diagnostics() -> String {
    let home = dirs::home_dir().map(|h| h.display().to_string());
    let (library, settings) = match storage::load() {
        Ok(l) => (Ok((l.games.len(), diag::settings_lines(&l.settings))), l.settings),
        Err(e) => (Err(e), models::Settings::default()),
    };
    let status = background::status();
    let facts = diag::Facts {
        version: env!("CARGO_PKG_VERSION").into(),
        webview: tauri::webview_version().unwrap_or_else(|_| "unknown".into()),
        windows: machine::windows(),
        now: util::now_secs(),
        data_dir: storage::root().map(|p| p.display().to_string()).unwrap_or_else(|e| format!("unavailable ({e})")),
        background: vec![
            ("Tray".into(), diag::feature(settings.keep_in_tray, status.tray_active, &status.tray_error)),
            (format!("Global shortcut ({})", status.shortcut_label), diag::feature(settings.shortcut, status.shortcut_active, &status.shortcut_error)),
            ("Start with Windows".into(), diag::feature(settings.start_with_windows, status.startup_error.is_empty(), &status.startup_error)),
            ("Controller wake-up".into(), diag::feature(settings.pad_wake, status.pad_watching, &status.pad_error)),
        ],
        library, memory: machine::footprint(), log: diag::recent(),
    };
    diag::report(&facts, home.as_deref())
}

fn main() {
    diag::install_panic_hook();
    if let Ok(root) = storage::root() { diag::init(root, dirs::home_dir().map(|h| h.display().to_string())); }
    // First start after the app identifier changed: carry the previous library over before anything reads it.
    match storage::migrate_legacy() {
        Ok(_) => {}
        Err(e) => { diag::event("migrate", &e); if let Ok(mut notice) = STARTUP_NOTICE.lock() { *notice = Some(format!("Your previous library could not be carried over automatically: {e}. It has not been changed or deleted.")); } }
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    diag::event("start", &format!("Aether Hub {} started{}", env!("CARGO_PKG_VERSION"), if args.iter().any(|a| a == "--tray") { " (quiet start)" } else if args.first().is_some_and(|a| a == "--launch") { " (from an exported shortcut)" } else { "" }));
    if args.first().is_some_and(|s| s == "--launch") {
        let result = (|| -> Result<(), String> {
            if args.len() != 2 { return Err("Shortcut contains invalid launch arguments".into()); }
            uuid::Uuid::parse_str(&args[1]).map_err(|_| "Shortcut game identifier is invalid")?;
            let library = storage::load()?;
            let game = library.games.iter().find(|g| g.id == args[1]).ok_or("This game was removed from Aether Hub. Create a new shortcut after adding it again.")?;
            launch::handoff(game).map(|_| ())
        })();
        if let Err(error) = result { diag::event("shortcut", &error); rfd::MessageDialog::new().set_title("Aether Hub — Could not launch").set_description(&error).set_level(rfd::MessageLevel::Error).show(); std::process::exit(1); }
        return;
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| background::second_launch(app, &args)))
        .on_window_event(background::on_window_event)
        .setup(|app| {
            if let Ok(library) = storage::load() { discord::configure(library.settings.discord_enabled, &library.settings.discord_app_id); }
            steam_sync::spawn_scheduler(app.handle().clone());
            backup::on_startup();
            background::init(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![load_library, save_library, pick_file, import_artwork, resolve_artwork, scan_games, cancel_scan, launch_game, export_shortcuts, grid_artwork, startup_notice, load_playtime, backup_overview, backup_now, backup_export, backup_restore, open_backup_folder, discord_status, discord_test, steam_sync_now, background_status, take_wake_request, check_update, open_releases_page, diagnostics, log_interface_error, missing_games])
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
    /// The interface reads `{ id, reason: "file" | "drive" }`; a renamed field or variant would silently show nothing.
    #[test] fn the_missing_game_answer_has_the_names_the_interface_reads() {
        use crate::presence::{Absence, Absent};
        let json = serde_json::to_string(&vec![Absent { id: "a".into(), reason: Absence::File }, Absent { id: "b".into(), reason: Absence::Drive }]).unwrap();
        assert_eq!(json, r#"[{"id":"a","reason":"file"},{"id":"b","reason":"drive"}]"#);
    }
    #[test] fn the_window_capability_grants_the_permission_set_and_nothing_broad() {
        let capability = include_str!("../../src-tauri/capabilities/main.json");
        assert!(capability.contains("\"library-commands\"") && capability.contains("\"windows\": [\"main\"]"));
        for broad in ["fs:", "shell:", "http:", "dialog:", "opener:"] { assert!(!capability.contains(broad), "{broad}"); }
    }
}
