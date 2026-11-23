//! Steam playtime import. By default it reads the public "games" feed of a profile (no key, no login). If the player
//! supplies a Steam Web API key, the official `GetOwnedGames` endpoint is used instead, which is steadier.
//! All requests are made here, never from the web view, so the interface keeps its no-network content policy.
use crate::{models::Game, playtime, storage, util};
use reqwest::{blocking::Client, redirect::Policy};
use serde::Serialize;
use serde_json::Value;
use std::{collections::HashMap, io::Read, time::Duration};
use tauri::{AppHandle, Emitter};

const MAX_BYTES: u64 = 16 * 1024 * 1024;
const PRIVATE_HINT: &str = "Steam returned no games. In Steam, open Edit Profile → Privacy Settings and set “Game details” to Public (and untick “Always keep my total playtime private”), then try again.";

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Profile { Id64(String), Vanity(String) }
#[derive(Debug, PartialEq, Eq)]
pub struct Owned { pub appid: String, pub minutes: u64, pub last_played: u64 }
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary { pub owned: u32, pub matched: u32, pub raised: u32, pub used_api: bool }

/// Accepts a SteamID64, a custom profile name, or a full steamcommunity.com profile link.
pub fn parse_profile(input: &str) -> Result<Profile, String> {
    let bad = || "Enter your SteamID64, custom profile name, or profile link (steamcommunity.com/id/… or steamcommunity.com/profiles/…).".to_string();
    let input = input.trim();
    let id64 = |s: &str| s.len() == 17 && s.bytes().all(|b| b.is_ascii_digit());
    // A custom name that is all digits is almost certainly a mistyped SteamID64.
    let vanity = |s: &str| (2..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') && !s.bytes().all(|b| b.is_ascii_digit());
    if input.is_empty() { return Err(bad()); }
    if input.contains('/') || input.contains('.') {
        let url = url::Url::parse(&if input.contains("://") { input.to_owned() } else { format!("https://{input}") }).map_err(|_| bad())?;
        if !matches!(url.host_str(), Some("steamcommunity.com") | Some("www.steamcommunity.com")) { return Err(bad()); }
        let mut parts = url.path_segments().ok_or_else(bad)?.filter(|p| !p.is_empty());
        return match (parts.next(), parts.next()) {
            (Some("profiles"), Some(id)) if id64(id) => Ok(Profile::Id64(id.into())),
            (Some("id"), Some(name)) if vanity(name) => Ok(Profile::Vanity(name.into())),
            _ => Err(bad()),
        };
    }
    if id64(input) { Ok(Profile::Id64(input.into())) } else if vanity(input) { Ok(Profile::Vanity(input.into())) } else { Err(bad()) }
}
fn tag(block: &str, name: &str) -> Option<String> {
    let (open, close) = (format!("<{name}>"), format!("</{name}>"));
    let start = block.find(&open)? + open.len();
    let end = start + block[start..].find(&close)?;
    let text = block[start..end].trim();
    Some(text.strip_prefix("<![CDATA[").and_then(|t| t.strip_suffix("]]>")).unwrap_or(text).trim().to_owned())
}
/// The public feed: `<gamesList><games><game><appID>…</appID><hoursOnRecord>1,234.5</hoursOnRecord></game>…`.
/// Games with no recorded time have no `hoursOnRecord`.
pub fn parse_games_xml(xml: &str) -> Result<Vec<Owned>, String> {
    if let Some(message) = tag(xml, "error").filter(|m| !m.is_empty()) { return Err(format!("Steam says: {message}")); }
    let (mut games, mut rest) = (vec![], xml);
    while let Some(start) = rest.find("<game>") {
        let after = &rest[start + "<game>".len()..];
        let Some(end) = after.find("</game>") else { break; };
        let block = &after[..end]; rest = &after[end + "</game>".len()..];
        let Some(appid) = tag(block, "appID").filter(|a| !a.is_empty() && a.bytes().all(|b| b.is_ascii_digit())) else { continue; };
        let hours = tag(block, "hoursOnRecord").map(|h| h.replace(',', "")).and_then(|h| h.parse::<f64>().ok()).filter(|h| h.is_finite() && *h >= 0.0).unwrap_or(0.0);
        games.push(Owned { appid, minutes: (hours * 60.0).round() as u64, last_played: 0 });
    }
    Ok(games)
}
/// The official API: `{"response":{"games":[{"appid":10,"playtime_forever":123,"rtime_last_played":1700000000}]}}`.
pub fn parse_owned_json(body: &str) -> Result<Vec<Owned>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "Steam sent an answer that could not be read".to_string())?;
    Ok(value["response"]["games"].as_array().into_iter().flatten().filter_map(|g| Some(Owned {
        appid: g["appid"].as_u64()?.to_string(), minutes: g["playtime_forever"].as_u64().unwrap_or(0), last_played: g["rtime_last_played"].as_u64().unwrap_or(0),
    })).collect())
}
fn client() -> Result<Client, String> {
    Client::builder().timeout(Duration::from_secs(20)).user_agent("AetherHub/1 (game launcher; playtime sync)")
        .redirect(Policy::custom(|attempt| if attempt.previous().len() < 3 && matches!(attempt.url().host_str(), Some("steamcommunity.com") | Some("api.steampowered.com")) { attempt.follow() } else { attempt.stop() }))
        .build().map_err(|e| e.without_url().to_string())
}
/// Errors never include the URL: for the API route it contains the player's key.
fn get(client: &Client, url: &str) -> Result<String, String> {
    let response = client.get(url).send().map_err(|e| format!("Could not reach Steam: {}", e.without_url()))?;
    match response.status().as_u16() {
        200..=299 => {}
        401 | 403 => return Err("Steam rejected the request. Check the Web API key, or clear it to use the public profile instead.".into()),
        429 => return Err("Steam is limiting requests right now. It will retry later.".into()),
        code => return Err(format!("Steam answered with HTTP {code}")),
    }
    let mut body = String::new();
    response.take(MAX_BYTES).read_to_string(&mut body).map_err(|e| format!("Steam’s answer could not be read: {e}"))?;
    Ok(body)
}
fn resolve_vanity(client: &Client, key: &str, name: &str) -> Result<String, String> {
    let body = get(client, &format!("https://api.steampowered.com/ISteamUser/ResolveVanityURL/v1/?key={key}&vanityurl={name}"))?;
    let value: Value = serde_json::from_str(&body).map_err(|_| "Steam sent an answer that could not be read".to_string())?;
    match value["response"]["success"].as_u64() {
        Some(1) => value["response"]["steamid"].as_str().map(str::to_owned).ok_or_else(|| "Steam did not return a profile id".into()),
        Some(42) => Err("Steam could not find that custom profile name.".into()),
        _ => Err("Steam could not resolve that profile name.".into()),
    }
}
pub fn sync(profile: &str, api_key: &str, games: &[Game], now: u64) -> Result<Summary, String> {
    let profile = parse_profile(profile)?;
    let client = client()?;
    let (owned, used_api) = if api_key.is_empty() {
        let url = match &profile {
            Profile::Id64(id) => format!("https://steamcommunity.com/profiles/{id}/games?tab=all&xml=1"),
            Profile::Vanity(name) => format!("https://steamcommunity.com/id/{name}/games?tab=all&xml=1"),
        };
        (parse_games_xml(&get(&client, &url)?)?, false)
    } else {
        let id = match &profile { Profile::Id64(id) => id.clone(), Profile::Vanity(name) => resolve_vanity(&client, api_key, name)? };
        let url = format!("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?key={api_key}&steamid={id}&include_played_free_games=1&format=json");
        (parse_owned_json(&get(&client, &url)?)?, true)
    };
    if owned.is_empty() { return Err(PRIVATE_HINT.into()); }
    let by_app: HashMap<&str, &Owned> = owned.iter().map(|o| (o.appid.as_str(), o)).collect();
    let updates: Vec<(String, u64, u64)> = games.iter().filter_map(|g| { let o = by_app.get(g.steam_app_id()?.as_str())?; Some((g.id.clone(), o.minutes, o.last_played)) }).collect();
    let raised = playtime::apply_steam(&updates, owned.len() as u32, now)?;
    Ok(Summary { owned: owned.len() as u32, matched: updates.len() as u32, raised, used_api })
}
/// Sync with explicit credentials (the Settings panel passes what is on screen, which may not be saved yet) and
/// remember the outcome so the panel can show it.
pub fn sync_now(profile: &str, api_key: &str) -> Result<Summary, String> {
    let (library, now) = (storage::load()?, util::now_secs());
    sync(profile, api_key, &library.games, now).inspect_err(|e| { let _ = playtime::record_steam_failure(e, now); })
}

/// Checks every 15 minutes whether a sync is due; sleeps otherwise. Does nothing unless the player enabled it.
pub fn spawn_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            let handle = app.clone();
            let wait = tauri::async_runtime::spawn_blocking(move || tick(&handle)).await.unwrap_or(900);
            tokio::time::sleep(Duration::from_secs(wait.clamp(60, 900))).await;
        }
    });
}
/// Seconds until the next check.
fn tick(app: &AppHandle) -> u64 {
    let Ok(library) = storage::load() else { return 900; };
    let s = &library.settings;
    if !s.steam_sync_enabled || s.steam_profile.trim().is_empty() { return 900; }
    let Ok(store) = playtime::load() else { return 900; };
    let (now, interval) = (util::now_secs(), s.steam_sync_hours as u64 * 3600);
    // After a failure retry within the hour rather than waiting out a long interval.
    let due = if store.steam.attempted_at > store.steam.synced_at { store.steam.attempted_at + interval.min(3600) } else { store.steam.synced_at + interval };
    if now < due { return due - now; }
    if let Err(e) = sync(&s.steam_profile, &s.steam_api_key, &library.games, now) { let _ = playtime::record_steam_failure(&e, now); }
    let _ = app.emit("aether:playtime-reload", ());
    900
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn profiles_are_understood_in_every_common_form() {
        assert_eq!(parse_profile("76561198000000000"), Ok(Profile::Id64("76561198000000000".into())));
        assert_eq!(parse_profile("  gaben "), Ok(Profile::Vanity("gaben".into())));
        assert_eq!(parse_profile("https://steamcommunity.com/id/gaben/"), Ok(Profile::Vanity("gaben".into())));
        assert_eq!(parse_profile("steamcommunity.com/profiles/76561198000000000/games"), Ok(Profile::Id64("76561198000000000".into())));
        assert_eq!(parse_profile("https://www.steamcommunity.com/id/some_name-1?l=english"), Ok(Profile::Vanity("some_name-1".into())));
    }
    #[test] fn anything_that_could_redirect_the_request_elsewhere_is_refused() {
        for bad in ["", "   ", "x", "https://evil.example/id/gaben", "https://steamcommunity.com.evil.example/id/gaben", "https://steamcommunity.com/market/", "a b", "../../etc", "gaben/../x", "1234567890123456", "http://localhost:80/id/x"] {
            assert!(parse_profile(bad).is_err(), "{bad:?}");
        }
    }
    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<gamesList><steamID64>76561198000000000</steamID64><steamID><![CDATA[someone]]></steamID><games>
<game><appID>570</appID><name><![CDATA[Dota 2]]></name><logo><![CDATA[x]]></logo><storeLink><![CDATA[y]]></storeLink><hoursLast2Weeks>3.5</hoursLast2Weeks><hoursOnRecord>1,234.5</hoursOnRecord></game>
<game><appID>1245620</appID><name><![CDATA[ELDEN RING]]></name><hoursOnRecord>0.2</hoursOnRecord></game>
<game><appID>440</appID><name><![CDATA[Team Fortress 2]]></name></game>
<game><appID>not-a-number</appID><hoursOnRecord>9</hoursOnRecord></game>
</games></gamesList>"#;
    #[test] fn the_public_feed_is_parsed_including_thousands_separators_and_missing_hours() {
        let games = parse_games_xml(XML).unwrap();
        assert_eq!(games.iter().map(|g| (g.appid.as_str(), g.minutes)).collect::<Vec<_>>(), [("570", 74_070), ("1245620", 12), ("440", 0)]);
    }
    #[test] fn a_private_or_missing_profile_is_reported_in_steams_own_words() {
        let err = parse_games_xml("<response><error><![CDATA[The specified profile could not be found.]]></error></response>").unwrap_err();
        assert!(err.contains("could not be found"), "{err}");
        assert!(parse_games_xml("<gamesList><games></games></gamesList>").unwrap().is_empty());
    }
    #[test] fn the_official_api_answer_is_parsed() {
        let body = r#"{"response":{"game_count":2,"games":[{"appid":570,"playtime_forever":74070,"rtime_last_played":1700000000},{"appid":220,"playtime_forever":0}]}}"#;
        assert_eq!(parse_owned_json(body).unwrap(), [Owned { appid: "570".into(), minutes: 74_070, last_played: 1_700_000_000 }, Owned { appid: "220".into(), minutes: 0, last_played: 0 }]);
        assert!(parse_owned_json(r#"{"response":{}}"#).unwrap().is_empty());
        assert!(parse_owned_json("<html>").is_err());
    }
}
