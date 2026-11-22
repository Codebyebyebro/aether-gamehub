//! Discord Rich Presence. The IPC client blocks, so it lives on one dedicated thread that is driven by messages and
//! sleeps (zero CPU) whenever there is nothing to show. Nothing here runs unless the user turned the feature on.
use discord_rich_presence::{activity::{Activity, ActivityType, Assets, StatusDisplayType, Timestamps}, DiscordIpc, DiscordIpcClient};
use serde::Serialize;
use std::{sync::{mpsc::{self, RecvTimeoutError, Sender}, Mutex, OnceLock}, thread, time::{Duration, Instant}};

const HEARTBEAT: Duration = Duration::from_secs(60);
const RETRY: Duration = Duration::from_secs(15);
const NOT_REACHABLE: &str = "Could not reach the Discord desktop app. Start Discord (the desktop app, not the web version) and try again.";

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status { pub enabled: bool, pub connected: bool, pub playing: String, pub error: String }

enum Cmd { Configure { enabled: bool, app_id: String }, Show { title: String, started_at: u64 }, Clear, Shutdown }
static TX: OnceLock<Sender<Cmd>> = OnceLock::new();
static STATUS: Mutex<Status> = Mutex::new(Status { enabled: false, connected: false, playing: String::new(), error: String::new() });

pub fn valid_id(id: &str) -> bool { (17..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit()) }
pub fn status() -> Status { STATUS.lock().map(|s| s.clone()).unwrap_or_default() }
fn set_status(change: impl FnOnce(&mut Status)) { if let Ok(mut s) = STATUS.lock() { change(&mut s); } }

fn sender() -> &'static Sender<Cmd> {
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let _ = thread::Builder::new().name("discord-presence".into()).spawn(move || run(rx));
        tx
    })
}
/// Apply the saved settings. Does nothing (and starts no thread) while the feature is off and was never on.
pub fn configure(enabled: bool, app_id: &str) {
    set_status(|s| { s.enabled = enabled; if !enabled { s.error.clear(); } });
    if !enabled && TX.get().is_none() { return; }
    let _ = sender().send(Cmd::Configure { enabled, app_id: app_id.trim().to_owned() });
}
pub fn show(title: &str, started_at: u64) { if status().enabled { let _ = sender().send(Cmd::Show { title: title.to_owned(), started_at }); } }
pub fn clear() { if let Some(tx) = TX.get() { let _ = tx.send(Cmd::Clear); } }
pub fn shutdown() { if let Some(tx) = TX.get() { let _ = tx.send(Cmd::Shutdown); } }

/// The activity friends see. Discord shows `details` in the member list; `name` replaces the application name on
/// clients that support it, so the game's title is visible either way.
pub fn build(title: &str, started_at: u64) -> Activity<'_> {
    Activity::new()
        .name(title).details(title).state("via Aether Hub")
        .activity_type(ActivityType::Playing).status_display_type(StatusDisplayType::Details)
        .timestamps(Timestamps::new().start(started_at as i64)) // Discord documents Unix seconds
        .assets(Assets::new().large_image("aether_hub").large_text("Aether Hub"))
}
fn push(client: &mut DiscordIpcClient, title: &str, started_at: u64) -> Result<(), String> {
    client.set_activity(build(title, started_at)).map_err(|e| e.to_string())?;
    // Read the acknowledgement so hours of heartbeats can never fill the pipe.
    client.recv().map(|_| ()).map_err(|e| e.to_string())
}
fn connect(app_id: &str) -> Result<DiscordIpcClient, String> {
    let mut client = DiscordIpcClient::new(app_id);
    client.connect().map_err(|_| NOT_REACHABLE.to_string())?;
    Ok(client)
}
fn close(client: &mut Option<DiscordIpcClient>) {
    if let Some(mut c) = client.take() { let _ = c.clear_activity(); let _ = c.close(); }
}
fn run(rx: mpsc::Receiver<Cmd>) {
    let (mut enabled, mut app_id) = (false, String::new());
    let mut desired: Option<(String, u64)> = None;
    let (mut client, mut client_id, mut retry_at) = (None::<DiscordIpcClient>, String::new(), Instant::now());
    let mut last_error = String::new(); // kept between retries so the settings panel keeps explaining what is wrong
    loop {
        let active = enabled && valid_id(&app_id) && desired.is_some();
        let wait = if !active { Duration::from_secs(3600) } else if client.is_some() { HEARTBEAT } else { retry_at.saturating_duration_since(Instant::now()) };
        match rx.recv_timeout(wait) {
            Ok(Cmd::Configure { enabled: e, app_id: a }) => { enabled = e; app_id = a; retry_at = Instant::now(); }
            Ok(Cmd::Show { title, started_at }) => { desired = Some((title, started_at)); retry_at = Instant::now(); }
            Ok(Cmd::Clear) => desired = None,
            Ok(Cmd::Shutdown) | Err(RecvTimeoutError::Disconnected) => { close(&mut client); break; }
            Err(RecvTimeoutError::Timeout) => {}
        }
        let active = enabled && valid_id(&app_id) && desired.is_some();
        if !active || client_id != app_id { close(&mut client); }
        if !active { last_error.clear(); }
        if active {
            if client.is_none() && Instant::now() >= retry_at {
                match connect(&app_id) {
                    Ok(c) => { client = Some(c); client_id = app_id.clone(); last_error.clear(); }
                    Err(e) => { last_error = e; retry_at = Instant::now() + RETRY; }
                }
            }
            if let (Some(c), Some((title, started_at))) = (client.as_mut(), desired.as_ref()) {
                if let Err(e) = push(c, title, *started_at) { last_error = e; client = None; retry_at = Instant::now() + Duration::from_secs(5); }
            }
        }
        let error = if enabled && !valid_id(&app_id) {
            if app_id.is_empty() { "Add your Discord Application ID".to_string() } else { "A Discord Application ID is 17 to 20 digits".to_string() }
        } else { last_error.clone() };
        let playing = desired.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
        set_status(|s| { s.enabled = enabled; s.connected = client.is_some(); s.playing = playing; s.error = error; });
    }
}
/// Connect, show a short test activity, confirm Discord accepted it, then clear it. Never blocks the caller for long.
pub fn test(app_id: &str) -> Result<(), String> {
    let id = app_id.trim().to_owned();
    if !valid_id(&id) { return Err("Enter your Discord Application ID first (17 to 20 digits).".into()); }
    let (tx, rx) = mpsc::channel();
    let _ = thread::Builder::new().name("discord-test".into()).spawn(move || { let _ = tx.send(probe(&id)); });
    rx.recv_timeout(Duration::from_secs(8)).map_err(|_| NOT_REACHABLE.to_string())?
}
fn probe(id: &str) -> Result<(), String> {
    let mut client = connect(id)?;
    let started = crate::util::now_secs();
    client.set_activity(build("Aether Hub connection test", started)).map_err(|_| NOT_REACHABLE.to_string())?;
    let (_, reply) = client.recv().map_err(|_| "Discord closed the connection. Check the Application ID.".to_string())?;
    let _ = client.clear_activity(); let _ = client.close();
    if reply.get("evt").and_then(|e| e.as_str()) == Some("ERROR") {
        let why = reply["data"]["message"].as_str().unwrap_or("Discord rejected the activity");
        return Err(format!("Discord rejected the request: {why}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn application_ids_are_snowflakes() {
        assert!(valid_id("123456789012345678") && valid_id("12345678901234567") && valid_id("12345678901234567890"));
        assert!(!valid_id("") && !valid_id("1234567890123456") && !valid_id("123456789012345678901") && !valid_id("12345678901234567a"));
    }
    #[test] fn the_activity_names_the_game_and_counts_up_from_the_start_time() {
        let v = serde_json::to_value(build("Hades", 1_731_902_100)).unwrap();
        assert_eq!(v["name"], "Hades");
        assert_eq!(v["details"], "Hades");
        assert_eq!(v["state"], "via Aether Hub");
        assert_eq!(v["timestamps"]["start"], 1_731_902_100u64);
        assert_eq!(v["type"], 0);
        assert_eq!(v["status_display_type"], 2);
        assert!(v["timestamps"].get("end").is_none(), "no end time, so Discord shows an elapsed timer");
    }
    #[test] fn disabled_configuration_starts_no_thread() {
        configure(false, "");
        assert!(TX.get().is_none() && !status().enabled);
        assert!(test("abc").is_err());
    }
}
