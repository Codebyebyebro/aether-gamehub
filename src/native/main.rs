#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod models;
mod storage;
mod scanner;
mod artwork;
mod metadata;
mod launch;
mod shortcuts;
use models::{Library, ScanResult};
use tauri::Manager;
use std::sync::atomic::{AtomicBool, Ordering};
static LAUNCHING: AtomicBool = AtomicBool::new(false);
static ART_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(3);
static SCAN_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

#[tauri::command]
async fn load_library() -> Result<Library, String> {
    tauri::async_runtime::spawn_blocking(storage::load).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn save_library(library: Library) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || storage::save(&library)).await.map_err(|e| e.to_string())?
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
        Ok(path.map(|p| p.to_string_lossy().into_owned()))
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
#[tauri::command]
async fn scan_games(folders: Vec<String>) -> Result<ScanResult, String> {
    let _permit = SCAN_JOBS.try_acquire().map_err(|_| "A scan is already running. Wait for it to finish before starting another.".to_string())?;
    tauri::async_runtime::spawn_blocking(move || scanner::scan(folders)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn launch_game(app: tauri::AppHandle, id: String) -> Result<(), String> {
    uuid::Uuid::parse_str(&id).map_err(|_| "Invalid game identifier")?;
    if LAUNCHING.swap(true, Ordering::SeqCst) { return Err("A game launch is already in progress".into()); }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let library = storage::load()?;
        let game = library.games.iter().find(|g| g.id == id).ok_or("Game is no longer in your library")?;
        launch::handoff(game)?;
        Ok::<String, String>(library.settings.after_launch)
    }).await.map_err(|e| e.to_string()).and_then(|r| r);
    LAUNCHING.store(false, Ordering::SeqCst);
    match result?.as_str() {
        "minimize" => if let Some(window) = app.get_webview_window("main") { window.minimize().map_err(|e| e.to_string())?; },
        "close" => app.exit(0),
        _ => {},
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
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--launch") {
        let result = (|| -> Result<(), String> {
            if args.len() != 2 { return Err("Shortcut contains invalid launch arguments".into()); }
            uuid::Uuid::parse_str(&args[1]).map_err(|_| "Shortcut game identifier is invalid")?;
            let library = storage::load()?;
            let game = library.games.iter().find(|g| g.id == args[1]).ok_or("This game was removed from Aether Hub. Create a new shortcut after adding it again.")?;
            launch::handoff(game)
        })();
        if let Err(error) = result { rfd::MessageDialog::new().set_title("Aether Hub — Could not launch").set_description(&error).set_level(rfd::MessageLevel::Error).show(); std::process::exit(1); }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") { let _ = window.unminimize(); let _ = window.show(); let _ = window.set_focus(); }
        }))
        .invoke_handler(tauri::generate_handler![load_library, save_library, pick_file, import_artwork, resolve_artwork, scan_games, launch_game, export_shortcuts, grid_artwork])
        .run(tauri::generate_context!())
        .expect("Aether Hub could not initialize");
}
