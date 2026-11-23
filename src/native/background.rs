//! Keeping Aether Hub running in the background: the tray icon, the global shortcut, starting with Windows, and
//! coming forward in Big Box when a controller connects. Every one of these is off until the player switches it on.
//!
//! One rule runs through all of it: the window is only ever hidden while the tray icon exists to bring it back.
use crate::{autostart, behavior::{self, Wanted}, diag, hotkey, models::Settings, padwatch, session, storage};
use serde::Serialize;
use std::sync::{atomic::{AtomicBool, Ordering}, Mutex, MutexGuard};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Window, WindowEvent,
};

const TRAY_ID: &str = "aether-hub";
/// The tray icon exists, so closing the window may hide it.
static TRAY_UP: AtomicBool = AtomicBool::new(false);
static PAD_WAKE: AtomicBool = AtomicBool::new(false);
/// Set when something asked for Big Box, cleared when the interface collects it. The interface may still be loading
/// (a quiet start at sign-in) when a controller connects, so the request is kept rather than only sent as an event.
static WAKE_PENDING: AtomicBool = AtomicBool::new(false);
static APPLIED: Mutex<Option<Wanted>> = Mutex::new(None);
static ERRORS: Mutex<Errors> = Mutex::new(Errors { tray: String::new(), shortcut: String::new(), startup: String::new(), pad: String::new() });

struct Errors { tray: String, shortcut: String, startup: String, pad: String }
fn errors() -> MutexGuard<'static, Errors> { ERRORS.lock().unwrap_or_else(|e| e.into_inner()) }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub tray_active: bool, pub tray_error: String,
    pub shortcut_active: bool, pub shortcut_label: String, pub shortcut_error: String,
    pub startup_error: String,
    pub pad_watching: bool, pub pad_error: String,
}
pub fn status() -> Status {
    let e = errors();
    Status {
        tray_active: TRAY_UP.load(Ordering::SeqCst), tray_error: e.tray.clone(),
        shortcut_active: hotkey::active(), shortcut_label: hotkey::LABEL.into(), shortcut_error: e.shortcut.clone(),
        startup_error: e.startup.clone(),
        pad_watching: padwatch::running(), pad_error: e.pad.clone(),
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") { let _ = window.unminimize(); let _ = window.show(); let _ = window.set_focus(); }
}
/// Brings the launcher forward and asks it to open Big Box.
pub fn request_big_box(app: &AppHandle) {
    WAKE_PENDING.store(true, Ordering::SeqCst);
    show_main(app);
    let _ = app.emit("aether:open-big-box", ());
}
pub fn take_wake_request() -> bool { WAKE_PENDING.swap(false, Ordering::SeqCst) }

fn build_tray(app: &AppHandle) -> Result<(), String> {
    if app.tray_by_id(TRAY_ID).is_some() { return Ok(()); }
    let icon = app.default_window_icon().cloned().ok_or("Aether Hub has no icon to show in the tray.")?;
    let open = MenuItem::with_id(app, "open", "Open Aether Hub", true, None::<&str>).map_err(|e| e.to_string())?;
    let big = MenuItem::with_id(app, "bigbox", "Open Big Box", true, None::<&str>).map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(app, "quit", "Quit Aether Hub", true, None::<&str>).map_err(|e| e.to_string())?;
    let menu = Menu::with_items(app, &[&open, &big, &quit]).map_err(|e| e.to_string())?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon).tooltip("Aether Hub").menu(&menu).show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "bigbox" => request_big_box(app),
            // Exit runs the same shutdown as closing normally: running sessions are saved and the Discord status is cleared.
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event { show_main(tray.app_handle()); }
        })
        .build(app).map(|_| ()).map_err(|e| e.to_string())
}
fn set_tray(app: &AppHandle, on: bool) {
    if on {
        match build_tray(app) {
            Ok(()) => { TRAY_UP.store(true, Ordering::SeqCst); errors().tray.clear(); }
            Err(e) => { TRAY_UP.store(false, Ordering::SeqCst); diag::event("tray", &e.to_string()); errors().tray = format!("The tray icon could not be created, so closing the window will still quit. {e}"); }
        }
    } else {
        // Cleared first, so a close during the removal quits instead of hiding into nothing.
        TRAY_UP.store(false, Ordering::SeqCst);
        let _ = app.remove_tray_by_id(TRAY_ID);
        errors().tray.clear();
        show_main(app); // a window hidden in the tray must not be left with no way back
    }
}
fn set_shortcut(app: &AppHandle, on: bool) {
    if on {
        let handle = app.clone();
        match hotkey::start(move || show_main(&handle)) { Ok(()) => errors().shortcut.clear(), Err(e) => { diag::event("shortcut", &e); errors().shortcut = e } }
    } else { hotkey::stop(); errors().shortcut.clear(); }
}
fn set_startup(on: bool) { match autostart::apply(on) { Ok(()) => errors().startup.clear(), Err(e) => { diag::event("startup", &e); errors().startup = e } } }
fn watch_pads(app: &AppHandle) {
    let handle = app.clone();
    // A game that is running keeps the launcher out of the way: the controller belongs to the game then.
    let started = padwatch::start(move || { if PAD_WAKE.load(Ordering::SeqCst) && !session::any_running() { request_big_box(&handle); } });
    match started { Ok(()) => errors().pad.clear(), Err(e) => { diag::event("pad", &e); errors().pad = e } }
}

/// Makes the background behaviour match the settings. Called on every settings save, so unchanged switches are left alone.
pub fn apply(app: &AppHandle, settings: &Settings) {
    let next = Wanted { tray: settings.keep_in_tray, shortcut: settings.shortcut, autostart: settings.start_with_windows, pad: settings.pad_wake };
    let plan = { let mut applied = APPLIED.lock().unwrap_or_else(|e| e.into_inner()); let plan = behavior::plan(*applied, next); *applied = Some(next); plan };
    PAD_WAKE.store(next.pad, Ordering::SeqCst);
    if let Some(on) = plan.tray { set_tray(app, on); }
    if let Some(on) = plan.shortcut { set_shortcut(app, on); }
    if let Some(on) = plan.autostart { set_startup(on); }
    if plan.pad == Some(true) { watch_pads(app); }
}

/// At start-up: apply the saved settings, then honour a quiet start. A quiet start (`--tray`, used by "start with
/// Windows") only hides the window when the tray icon is really there; otherwise the window opens as usual.
pub fn init(app: &AppHandle) {
    let settings = storage::load().map(|library| library.settings).unwrap_or_default();
    apply(app, &settings);
    let quiet = std::env::args().skip(1).any(|a| a == autostart::ARG);
    if quiet && TRAY_UP.load(Ordering::SeqCst) {
        if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); }
    }
}
/// A second launch while Aether Hub is already running (for example from the Start menu) shows the existing window.
/// The sign-in launch is the exception: it must not pop a window the player has put away.
pub fn second_launch(app: &AppHandle, args: &[String]) {
    if !args.iter().any(|a| a == autostart::ARG) { show_main(app); }
}
/// Closing the main window hides it when the tray is up; otherwise it closes (and the program quits) as before.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == "main" && TRAY_UP.load(Ordering::SeqCst) { api.prevent_close(); let _ = window.hide(); }
    }
}
