//! Playtime lives in its own `playtime.json` next to `library.json`. The backend owns it exclusively, so recording
//! a session never races with the interface saving the whole library, and a stale interface can never overwrite it.
use crate::storage;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::Mutex};

pub static LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry { pub seconds: u64, pub last_played: u64 }

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SteamState {
    /// Last successful sync, and the last attempt (so a failing profile is retried hourly, not every poll).
    pub synced_at: u64, pub attempted_at: u64, pub matched: u32, pub owned: u32, pub error: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Store { pub version: u32, pub games: HashMap<String, Entry>, pub steam: SteamState }

fn path() -> Result<PathBuf, String> { Ok(storage::root()?.join("playtime.json")) }
fn read(path: &Path) -> Result<Store, String> {
    if !path.exists() { return Ok(Store { version: 1, ..Store::default() }); }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read playtime: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Cannot parse playtime: {e}. The file was not overwritten."))
}
pub fn load() -> Result<Store, String> { read(&path()?) }
fn write(path: &Path, store: &Store) -> Result<(), String> {
    storage::write_atomic(path, &serde_json::to_vec(store).map_err(|e| e.to_string())?)
}
fn modify<T>(change: impl FnOnce(&mut Store) -> T) -> Result<T, String> {
    let _guard = LOCK.lock().map_err(|_| "Playtime lock unavailable")?;
    let path = path()?; let mut store = read(&path)?;
    let out = change(&mut store); store.version = 1;
    write(&path, &store)?; Ok(out)
}
/// Replace everything (used by backup restore).
pub fn replace(store: Store) -> Result<(), String> { modify(|current| *current = store) }

pub fn add_session(id: &str, seconds: u64, ended_at: u64) -> Result<Entry, String> {
    modify(|store| {
        let entry = store.games.entry(id.to_owned()).or_default();
        entry.seconds += seconds; entry.last_played = entry.last_played.max(ended_at);
        entry.clone()
    })
}
/// Steam's number is authoritative for time played through Steam, which includes sessions launched from Aether Hub.
/// Taking the larger value avoids counting those sessions twice and means a profile that lags behind (it can take
/// a while to update) can never make a total go down.
pub fn merge_steam(store: &mut Store, updates: &[(String, u64, u64)], owned: u32, now: u64) -> u32 {
    let mut raised = 0;
    for (id, minutes, last_played) in updates {
        let entry = store.games.entry(id.clone()).or_default();
        if minutes * 60 > entry.seconds { entry.seconds = minutes * 60; raised += 1; }
        if *last_played > entry.last_played { entry.last_played = *last_played; }
    }
    store.steam = SteamState { synced_at: now, attempted_at: now, matched: updates.len() as u32, owned, error: String::new() };
    raised
}
pub fn apply_steam(updates: &[(String, u64, u64)], owned: u32, now: u64) -> Result<u32, String> {
    modify(|store| merge_steam(store, updates, owned, now))
}
pub fn record_steam_failure(message: &str, now: u64) -> Result<(), String> {
    modify(|store| { store.steam.attempted_at = now; store.steam.error = message.chars().take(300).collect(); })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn sessions_accumulate_and_keep_the_latest_play_date() {
        let _sandbox = storage::testutil::Sandbox::new();
        assert_eq!(add_session("g1", 600, 1_000).unwrap(), Entry { seconds: 600, last_played: 1_000 });
        assert_eq!(add_session("g1", 300, 900).unwrap(), Entry { seconds: 900, last_played: 1_000 });
        assert_eq!(load().unwrap().games["g1"].seconds, 900);
    }
    #[test] fn steam_never_lowers_a_total_and_never_double_counts() {
        let mut store = Store::default();
        store.games.insert("g1".into(), Entry { seconds: 100 * 60 + 30 * 60, last_played: 50 }); // 100 min from Steam + 30 min played here
        // Steam has not caught up yet (stale 100 min): keep the larger local total.
        assert_eq!(merge_steam(&mut store, &[("g1".into(), 100, 40)], 10, 7), 0);
        assert_eq!(store.games["g1"].seconds, 130 * 60);
        // Steam catches up to 130 min: same total, not 260.
        assert_eq!(merge_steam(&mut store, &[("g1".into(), 130, 60)], 10, 8), 0);
        assert_eq!(store.games["g1"].seconds, 130 * 60);
        assert_eq!(store.games["g1"].last_played, 60);
        // Played on another PC: Steam is ahead, so it wins.
        assert_eq!(merge_steam(&mut store, &[("g1".into(), 400, 70), ("g2".into(), 5, 0)], 10, 9), 2);
        assert_eq!((store.games["g1"].seconds, store.games["g2"].seconds), (400 * 60, 300));
        assert_eq!((store.steam.synced_at, store.steam.matched, store.steam.owned), (9, 2, 10));
    }
    #[test] fn a_failure_is_recorded_without_losing_the_last_good_sync() {
        let _sandbox = storage::testutil::Sandbox::new();
        apply_steam(&[("g1".into(), 10, 0)], 1, 100).unwrap();
        record_steam_failure("profile is private", 200).unwrap();
        let steam = load().unwrap().steam;
        assert_eq!((steam.synced_at, steam.attempted_at, steam.error.as_str()), (100, 200, "profile is private"));
    }
    #[test] fn a_corrupt_file_is_reported_not_overwritten() {
        let _sandbox = storage::testutil::Sandbox::new();
        fs::write(path().unwrap(), b"{ not json").unwrap();
        assert!(add_session("g1", 5, 5).is_err());
        assert_eq!(fs::read(path().unwrap()).unwrap(), b"{ not json");
    }
}
