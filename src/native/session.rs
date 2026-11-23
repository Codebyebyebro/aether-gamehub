//! Follows a launched game until it exits: records playtime, drives Discord presence and tells the interface.
//! This needs the launcher to stay open, so it is skipped when the player chose "Close Launcher Completely".
use crate::{discord, launch::Handoff, models::{Game, Settings}, playtime, scanner, util};
use serde::Serialize;
use serde_json::Value;
use std::{fs, path::{Component, Path, PathBuf}, sync::Mutex, thread, time::Duration};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, Emitter};

const POLL: Duration = Duration::from_secs(5);
const FAST_POLL: Duration = Duration::from_secs(2);
/// A launcher stub may exit a moment before the real game appears: this many empty polls in a row ends a session.
const GRACE_POLLS: u32 = 4;
/// Storefront clients can spend minutes updating before the game starts.
const STORE_START_POLLS: u32 = 90;
const DIRECT_START_POLLS: u32 = 10;
const MIN_SESSION_SECS: u64 = 15;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEvent { pub id: String, pub title: String, pub started_at: u64 }
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaytimeEvent { pub id: String, pub seconds: u64, pub last_played: u64 }

struct Live { id: String, title: String, started_at: u64, last_seen: u64 }
static LIVE: Mutex<Vec<Live>> = Mutex::new(Vec::new());
/// True while any game launched from Aether Hub is still running. A controller that connects then belongs to that game.
pub fn any_running() -> bool { LIVE.lock().map(|live| !live.is_empty()).unwrap_or(false) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watch {
    /// A directly started executable, plus its folder for games whose first process hands off to another one.
    Pid(u32, Option<PathBuf>),
    /// Any process running from inside this install folder.
    Dir(PathBuf),
    /// Specific process names (the Riot client starts the game as a separate program).
    Names(Vec<String>),
    Nothing,
}
/// Matching "anything under this folder" is only safe for a folder that belongs to one game: never a drive root,
/// anything inside Windows, or a shared root such as `Program Files` itself.
pub fn specific_dir(dir: &Path) -> bool {
    let named = |p: &Path| p.components().filter(|c| matches!(c, Component::Normal(_))).count();
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let inside_windows = ["WINDIR", "SYSTEMROOT"].iter().filter_map(|k| var(k)).any(|w| util::path_starts_with(dir, &w));
    let shared_root = ["PROGRAMFILES", "PROGRAMFILES(X86)", "PROGRAMDATA"].iter().filter_map(|k| var(k)).any(|r| util::path_starts_with(dir, &r) && named(dir) <= named(&r));
    named(dir) >= 1 && !inside_windows && !shared_root
}
fn steam_install_dir(appid: &str) -> Option<PathBuf> {
    let mut ignored = vec![];
    for library in scanner::steam_libraries(&mut ignored) {
        let apps = library.join("steamapps");
        let Ok(text) = fs::read_to_string(apps.join(format!("appmanifest_{appid}.acf"))) else { continue; };
        let Ok(value) = scanner::parse_vdf(&text) else { continue; };
        let Some(dir) = value["AppState"]["installdir"].as_str() else { continue; };
        let path = apps.join("common").join(dir);
        if path.is_dir() { return Some(path); }
    }
    None
}
fn epic_install_dir(target: &str) -> Option<PathBuf> {
    let wanted = url::Url::parse(target).ok()?.path().trim_matches('/').to_owned();
    for entry in fs::read_dir(scanner::program_data().join("Epic/EpicGamesLauncher/Data/Manifests")).ok()?.filter_map(Result::ok) {
        let Ok(bytes) = fs::read(entry.path()) else { continue; };
        let Ok(v) = serde_json::from_slice::<Value>(&bytes) else { continue; };
        let Some(app) = v["AppName"].as_str() else { continue; };
        let encoded: String = url::form_urlencoded::byte_serialize(app.as_bytes()).collect();
        if encoded == wanted { return v["InstallLocation"].as_str().map(PathBuf::from).filter(|p| p.is_dir()); }
    }
    None
}
fn riot_processes(args: &str) -> Vec<String> {
    if args.contains("--launch-product=valorant") { vec!["valorant-win64-shipping.exe".into()] }
    else if args.contains("--launch-product=league_of_legends") { vec!["league of legends.exe".into()] }
    else { vec![] }
}
pub fn target_for(game: &Game, handoff: &Handoff) -> Watch {
    if let Some(pid) = handoff.pid { return Watch::Pid(pid, handoff.dir.clone().filter(|d| specific_dir(d))); }
    match game.platform.as_str() {
        "Steam" => game.steam_app_id().and_then(|id| steam_install_dir(&id)).filter(|d| specific_dir(d)).map(Watch::Dir).unwrap_or(Watch::Nothing),
        "Epic" => epic_install_dir(&game.target).filter(|d| specific_dir(d)).map(Watch::Dir).unwrap_or(Watch::Nothing),
        "Riot" => match riot_processes(&game.args) { names if names.is_empty() => Watch::Nothing, names => Watch::Names(names) },
        _ => Watch::Nothing,
    }
}

pub struct Probe { system: Option<System>, watch: Watch }
impl Probe {
    pub fn new(watch: Watch) -> Self { Probe { system: System::new().ok(), watch } }
    pub fn running(&mut self) -> bool {
        let Some(system) = self.system.as_mut() else { return false; };
        // Only the executable path is read, and sysinfo caches it per process, so each poll is cheap.
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet));
        let own = std::process::id();
        let in_dir = |dir: &Path| system.processes().iter().any(|(pid, p)| pid.as_u32() != own && p.exe().is_some_and(|exe| util::path_starts_with(exe, dir)));
        match &self.watch {
            Watch::Pid(pid, dir) => system.process(Pid::from_u32(*pid)).is_some() || dir.as_deref().is_some_and(in_dir),
            Watch::Dir(dir) => in_dir(dir),
            Watch::Names(names) => system.processes().values().any(|p| names.iter().any(|n| p.name().to_string_lossy().eq_ignore_ascii_case(n))),
            Watch::Nothing => false,
        }
    }
}

/// Begin following a game that was just handed off. Returns immediately; the work happens on its own thread.
pub fn start(app: AppHandle, game: Game, handoff: Handoff, settings: &Settings) {
    discord::configure(settings.discord_enabled, &settings.discord_app_id);
    let _ = thread::Builder::new().name(format!("session-{}", game.id)).spawn(move || track(app, game, handoff));
}
fn track(app: AppHandle, game: Game, handoff: Handoff) {
    let watch = target_for(&game, &handoff);
    if watch == Watch::Nothing { return; }
    let budget = if matches!(watch, Watch::Pid(..)) { DIRECT_START_POLLS } else { STORE_START_POLLS };
    let mut probe = Probe::new(watch);
    for attempt in 0.. {
        if probe.running() { break; }
        if attempt + 1 >= budget { return; }
        thread::sleep(FAST_POLL);
    }
    let started_at = util::now_secs();
    begin(&app, &game, started_at);
    let (mut last_seen, mut misses) = (started_at, 0);
    loop {
        thread::sleep(POLL);
        if probe.running() { last_seen = util::now_secs(); misses = 0; touch(&game.id, last_seen); }
        else { misses += 1; if misses >= GRACE_POLLS { break; } }
    }
    finish(&app, &game.id, last_seen);
}
fn begin(app: &AppHandle, game: &Game, started_at: u64) {
    if let Ok(mut live) = LIVE.lock() {
        live.retain(|l| l.id != game.id);
        live.push(Live { id: game.id.clone(), title: game.title.clone(), started_at, last_seen: started_at });
    }
    discord::show(&game.title, started_at);
    let _ = app.emit("aether:session-started", SessionEvent { id: game.id.clone(), title: game.title.clone(), started_at });
}
fn touch(id: &str, now: u64) { if let Ok(mut live) = LIVE.lock() { if let Some(l) = live.iter_mut().find(|l| l.id == id) { l.last_seen = now; } } }
fn remove(id: &str) -> Option<(Live, Option<(String, u64)>)> {
    let mut live = LIVE.lock().ok()?;
    let at = live.iter().position(|l| l.id == id)?;
    let ended = live.remove(at);
    let next = live.last().map(|l| (l.title.clone(), l.started_at));
    Some((ended, next))
}
fn record(app: &AppHandle, ended: &Live, until: u64) {
    let seconds = until.saturating_sub(ended.started_at);
    if seconds >= MIN_SESSION_SECS {
        if let Ok(entry) = playtime::add_session(&ended.id, seconds, until) {
            let _ = app.emit("aether:playtime", PlaytimeEvent { id: ended.id.clone(), seconds: entry.seconds, last_played: entry.last_played });
            crate::backup::note_change();
        }
    }
    let _ = app.emit("aether:session-ended", SessionEvent { id: ended.id.clone(), title: ended.title.clone(), started_at: ended.started_at });
}
fn finish(app: &AppHandle, id: &str, last_seen: u64) {
    let Some((ended, next)) = remove(id) else { return; };
    record(app, &ended, last_seen);
    match next { Some((title, started_at)) => discord::show(&title, started_at), None => discord::clear() }
    crate::background::after_session(app);
}
/// Called as the launcher quits while a game is still running, so that time is not lost.
pub fn flush_all(app: &AppHandle) {
    let ended: Vec<Live> = LIVE.lock().map(|mut live| std::mem::take(&mut *live)).unwrap_or_default();
    let now = util::now_secs();
    for session in &ended { record(app, session, now.max(session.last_seen)); }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process::{Child, Command};
    /// Runs a copy of `sleep` from its own folder, so "a process inside this folder" is unambiguous.
    fn game(folder: &str) -> (PathBuf, Child) {
        let which = Command::new("which").arg("sleep").output().unwrap();
        let dir = std::env::temp_dir().join(format!("aether-session-{folder}-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&dir).unwrap();
        fs::copy(String::from_utf8_lossy(&which.stdout).trim(), dir.join("game")).unwrap();
        let child = Command::new(dir.join("game")).arg("30").spawn().unwrap();
        (dir, child)
    }
    fn stop(mut child: Child, dir: PathBuf) { child.kill().unwrap(); child.wait().unwrap(); let _ = fs::remove_dir_all(dir); }
    #[test] fn a_started_process_is_followed_until_it_exits() {
        let (dir, mut child) = game("pid");
        let mut probe = Probe::new(Watch::Pid(child.id(), None));
        assert!(probe.running());
        child.kill().unwrap(); child.wait().unwrap();
        assert!(!probe.running());
        let _ = fs::remove_dir_all(dir);
    }
    #[test] fn a_launcher_stub_that_exits_hands_over_to_the_game_in_the_same_folder() {
        let (dir, child) = game("stub");
        let mut stub = Command::new("true").spawn().unwrap(); stub.wait().unwrap();
        let mut probe = Probe::new(Watch::Pid(stub.id(), Some(dir.clone())));
        assert!(probe.running(), "the stub has exited but the game process in its folder is found");
        let pid = child.id(); stop(child, dir);
        assert!(!probe.running(), "and the session ends once that process is gone (pid {pid})");
    }
    #[test] fn store_games_are_followed_by_their_install_folder() {
        let (dir, child) = game("dir");
        let mut probe = Probe::new(Watch::Dir(dir.clone()));
        assert!(probe.running());
        stop(child, dir);
        assert!(!probe.running());
    }
    #[test] fn process_names_are_matched_without_regard_to_case() {
        let (dir, child) = game("name");
        let mut probe = Probe::new(Watch::Names(vec!["GAME".into()]));
        assert!(probe.running());
        stop(child, dir);
    }
    #[test] fn only_folders_that_belong_to_one_game_are_watched_wholesale() {
        assert!(specific_dir(Path::new("/games/hades")) && !specific_dir(Path::new("/")));
    }
}
#[cfg(test)]
mod pure_tests {
    use super::*;
    #[test] fn riot_games_map_to_their_game_process() {
        assert_eq!(riot_processes("--launch-product=valorant --launch-patchline=live"), ["valorant-win64-shipping.exe"]);
        assert_eq!(riot_processes("--launch-product=league_of_legends"), ["league of legends.exe"]);
        assert!(riot_processes("--launch-product=bacon").is_empty());
    }
}
