//! "Find missing artwork": looks up games that have no cover by name in Steam's public store (no account or key),
//! and optionally on SteamGridDB with the player's own key, which is used for this run only and never saved.
//!
//! A match is accepted only when the names agree after tidying (case, punctuation, "™", an edition suffix, spacing)
//! and exactly one store entry fits. Anything less certain is left alone and reported, so a wrong picture is never set.
//! The picture is downloaded into the artwork cache before it is offered, so a store entry without a cover is not used.
use serde::{Deserialize, Serialize};
use std::{sync::atomic::{AtomicBool, Ordering}, time::Duration};

/// One game the interface wants artwork for.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wanted { pub id: String, pub title: String }
/// Artwork found for a game. `cover` and `banner` are approved HTTPS addresses, already cached.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found { pub id: String, pub cover: String, pub banner: String, pub source: String, pub matched: String }
/// A game left as it was, and why.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unmatched { pub id: String, pub title: String, pub reason: String }
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome { pub found: Vec<Found>, pub unmatched: Vec<Unmatched>, pub cancelled: bool }
/// One entry from a store search.
#[derive(Clone, Debug, Deserialize)]
pub struct Candidate { pub id: u64, pub name: String, #[serde(rename = "type", default = "app")] pub kind: String }
fn app() -> String { "app".into() }

pub const NOT_ON_STEAM: &str = "Not found on Steam";
pub const AMBIGUOUS: &str = "More than one game has this name";
pub const NO_PICTURE: &str = "Found, but it has no cover picture";
pub const NOT_ON_STEAM_OR_GRID: &str = "Not found on Steam or SteamGridDB";
pub const OFFLINE: &str = "The store could not be reached";

/// Lower case, apostrophes dropped ("Baldur's" = "Baldurs"), "&" read as "and", "™ ® ©" removed, every other
/// non-letter or digit becomes a space, and runs of spaces collapse.
pub fn normalize(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for c in title.chars() {
        match c {
            '\'' | '’' | '‘' | '`' | '´' | '™' | '®' | '©' => {}
            '&' => out.push_str(" and "),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => out.push(' '),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
/// The name without a trailing edition label, so "Control Ultimate Edition" and "Control" are the same game.
fn without_edition(normalized: &str) -> &str {
    const SUFFIXES: [&str; 17] = ["game of the year edition", "goty edition", "goty", "definitive edition", "complete edition",
        "deluxe edition", "ultimate edition", "gold edition", "standard edition", "enhanced edition", "special edition",
        "anniversary edition", "remastered", "directors cut", "legendary edition", "premium edition", "edition"];
    for suffix in SUFFIXES {
        // Only a whole-word label after a real name counts: "Edition" alone, or "Reedition", stays as it is.
        if let Some(rest) = normalized.strip_suffix(suffix).and_then(|r| r.strip_suffix(' ')) {
            if !rest.is_empty() { return rest; }
        }
    }
    normalized
}
/// "HollowKnight" → "Hollow Knight", "DOOMEternal" → "DOOM Eternal", for searching with a file name.
pub fn split_words(title: &str) -> String {
    let chars: Vec<char> = title.chars().collect();
    let mut out = String::with_capacity(title.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if prev.is_lowercase() || (prev.is_uppercase() && next_lower) { out.push(' '); }
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
/// The single store entry this title names, or why there is none. Exact names win over edition-free names, which win over
/// names compared without spaces (a file name like "HollowKnight"). Two different games fitting equally means no answer.
pub fn pick<'a>(title: &str, candidates: &'a [Candidate]) -> Result<&'a Candidate, &'static str> {
    let apps: Vec<&Candidate> = candidates.iter().filter(|c| c.kind == "app").collect();
    let wanted = normalize(&split_words(title));
    if wanted.is_empty() { return Err(NOT_ON_STEAM); }
    let tiers: [&dyn Fn(&str) -> String; 3] = [
        &|s: &str| normalize(s),
        &|s: &str| without_edition(&normalize(s)).to_owned(),
        &|s: &str| without_edition(&normalize(s)).replace(' ', ""),
    ];
    for key in tiers {
        let target = key(&wanted);
        let hits: Vec<&Candidate> = apps.iter().copied().filter(|c| key(&split_words(&c.name)) == target).collect();
        let mut ids: Vec<u64> = hits.iter().map(|c| c.id).collect(); ids.sort_unstable(); ids.dedup();
        match ids.len() { 0 => continue, 1 => return Ok(hits[0]), _ => return Err(AMBIGUOUS) }
    }
    Err(NOT_ON_STEAM)
}

pub fn steam_cover(id: u64) -> String { format!("https://cdn.akamai.steamstatic.com/steam/apps/{id}/library_600x900.jpg") }
pub fn steam_banner(id: u64) -> String { format!("https://cdn.akamai.steamstatic.com/steam/apps/{id}/library_hero.jpg") }

/// What the finder needs from the outside world. The real one talks to the network; tests use a fake.
pub trait Lookup {
    /// Store entries for a search term.
    fn steam(&mut self, term: &str) -> Result<Vec<Candidate>, String>;
    /// True when the picture downloads and decodes (it is then in the artwork cache).
    fn picture(&mut self, url: &str) -> bool;
    /// A SteamGridDB cover whose game name matches `title`, if any.
    fn grid(&mut self, title: &str) -> Result<Option<(String, String)>, String>;
    /// Whether a SteamGridDB key was given for this run.
    fn has_grid(&self) -> bool;
}

/// Finds artwork for each game in turn. `progress(done, total, title)` is called before each game;
/// `cancelled()` is checked between games, and a cancelled run returns what it found so far, marked `cancelled`.
pub fn run(games: &[Wanted], look: &mut dyn Lookup, cancelled: &dyn Fn() -> bool, progress: &mut dyn FnMut(usize, usize, &str)) -> Outcome {
    let mut outcome = Outcome::default();
    let total = games.len();
    for (done, game) in games.iter().enumerate() {
        if cancelled() { outcome.cancelled = true; break; }
        progress(done, total, &game.title);
        match find_one(game, look) {
            Ok(found) => outcome.found.push(found),
            // Stopped while waiting on this game: it was not really looked up, so it is not reported as unmatched.
            Err(_) if cancelled() => { outcome.cancelled = true; break; }
            Err(reason) => outcome.unmatched.push(Unmatched { id: game.id.clone(), title: game.title.clone(), reason }),
        }
    }
    if !outcome.cancelled { progress(total, total, ""); }
    outcome
}
fn find_one(game: &Wanted, look: &mut dyn Lookup) -> Result<Found, String> {
    // A title with no spaces is usually a file name ("HollowKnight"), so it is searched as words.
    let term = if game.title.trim().contains(' ') { game.title.trim().to_owned() } else { split_words(game.title.trim()) };
    let steam_reason = match look.steam(&term) {
        Err(_) => OFFLINE.to_string(),
        Ok(candidates) => match pick(&game.title, &candidates) {
            Err(reason) => reason.to_string(),
            Ok(hit) => {
                let cover = steam_cover(hit.id);
                if look.picture(&cover) {
                    let banner = steam_banner(hit.id);
                    let banner = if look.picture(&banner) { banner } else { String::new() };
                    return Ok(Found { id: game.id.clone(), cover, banner, source: "Steam".into(), matched: hit.name.clone() });
                }
                NO_PICTURE.to_string()
            }
        },
    };
    if look.has_grid() {
        match look.grid(&game.title) {
            Ok(Some((cover, name))) if look.picture(&cover) => return Ok(Found { id: game.id.clone(), cover, banner: String::new(), source: "SteamGridDB".into(), matched: name }),
            Ok(_) if steam_reason == NOT_ON_STEAM => return Err(NOT_ON_STEAM_OR_GRID.into()),
            Ok(_) => return Err(format!("{steam_reason}; not on SteamGridDB either")),
            Err(_) if steam_reason == OFFLINE => return Err(OFFLINE.into()),
            Err(_) => return Err(format!("{steam_reason}; SteamGridDB could not be reached")),
        }
    }
    Err(steam_reason)
}

/* ---------- The real network lookup ---------- */

static CANCEL: AtomicBool = AtomicBool::new(false);
pub fn cancel() { CANCEL.store(true, Ordering::SeqCst); }
pub fn reset() { CANCEL.store(false, Ordering::SeqCst); }
pub fn is_cancelled() -> bool { CANCEL.load(Ordering::SeqCst) }

/// Waits up to `total`, returning early (false) if the run is cancelled.
fn pause(total: Duration) -> bool {
    let step = Duration::from_millis(100);
    let mut waited = Duration::ZERO;
    while waited < total { if is_cancelled() { return false; } std::thread::sleep(step); waited += step; }
    true
}

pub struct Network { client: reqwest::blocking::Client, key: String, searched: bool }
impl Network {
    pub fn new(key: String) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(12)).redirect(reqwest::redirect::Policy::none()).build().map_err(|e| e.to_string())?;
        Ok(Self { client, key, searched: false })
    }
}
#[derive(Deserialize)] struct SearchAnswer { #[serde(default)] items: Vec<Candidate> }
impl Lookup for Network {
    fn steam(&mut self, term: &str) -> Result<Vec<Candidate>, String> {
        // Steam's store asks for a gentle pace; a short gap between searches keeps a large library well inside it.
        if self.searched && !pause(Duration::from_millis(700)) { return Err("cancelled".into()); }
        self.searched = true;
        let mut url = url::Url::parse("https://store.steampowered.com/api/storesearch/").map_err(|e| e.to_string())?;
        url.query_pairs_mut().append_pair("term", term).append_pair("l", "english").append_pair("cc", "US");
        for attempt in 0..2 {
            let response = self.client.get(url.clone()).send().map_err(|_| OFFLINE.to_string())?;
            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt == 0 {
                if !pause(Duration::from_secs(20)) { return Err("cancelled".into()); }
                continue;
            }
            let answer: SearchAnswer = response.error_for_status().map_err(|_| OFFLINE.to_string())?.json().map_err(|_| "Unexpected answer from Steam".to_string())?;
            return Ok(answer.items);
        }
        Err(OFFLINE.into())
    }
    fn picture(&mut self, url: &str) -> bool { crate::artwork::resolve(url).is_ok() }
    fn grid(&mut self, title: &str) -> Result<Option<(String, String)>, String> { crate::metadata::exact_cover(&self.client, title, &self.key) }
    fn has_grid(&self) -> bool { !self.key.is_empty() }
}
/// Checks what the interface sent: real game ids, sensible titles, and a key shaped like a SteamGridDB key.
pub fn validate(games: &[Wanted], key: &str) -> Result<(), String> {
    if games.len() > 5000 { return Err("Too many games in one request".into()); }
    if !key.is_empty() && (key.len() > 200 || !key.bytes().all(|b| b.is_ascii_alphanumeric())) { return Err("That SteamGridDB API key doesn't look right. Leave it empty to use Steam only.".into()); }
    for game in games {
        uuid::Uuid::parse_str(&game.id).map_err(|_| "Invalid game identifier")?;
        if game.title.trim().is_empty() || game.title.len() > 640 { return Err(format!("“{}” has no usable title", game.title.chars().take(60).collect::<String>())); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    fn c(id: u64, name: &str) -> Candidate { Candidate { id, name: name.into(), kind: "app".into() } }

    #[test] fn names_are_compared_without_case_punctuation_or_trademarks() {
        assert_eq!(normalize("Baldur’s Gate 3"), "baldurs gate 3");
        assert_eq!(normalize("Baldur's Gate 3"), normalize("Baldurs Gate 3"));
        assert_eq!(normalize("DOOM® Eternal™"), "doom eternal");
        assert_eq!(normalize("Ori and the Will of the Wisps"), normalize("Ori & the Will of the Wisps"));
        assert_eq!(normalize("Tom Clancy's Rainbow Six® Siege"), "tom clancys rainbow six siege");
        assert_eq!(normalize("  Hades:  II  "), "hades ii");
        assert_eq!(normalize("Pokémon"), "pokémon");
    }
    #[test] fn file_names_are_split_into_words_for_searching() {
        assert_eq!(split_words("HollowKnight"), "Hollow Knight");
        assert_eq!(split_words("DOOMEternal"), "DOOM Eternal");
        assert_eq!(split_words("Witcher3"), "Witcher3");
        assert_eq!(split_words("Hades"), "Hades");
        assert_eq!(split_words("VALORANT"), "VALORANT");
        assert_eq!(split_words("Baldur's Gate 3"), "Baldur's Gate 3");
    }
    #[test] fn an_exact_name_is_matched_and_sequels_are_not_confused() {
        let results = [c(1145350, "Hades II"), c(1145360, "Hades"), c(9, "Hades Soundtrack")];
        assert_eq!(pick("Hades", &results).unwrap().id, 1145360);
        assert_eq!(pick("Hades II", &results).unwrap().id, 1145350);
        assert_eq!(pick("hades", &results).unwrap().id, 1145360, "case does not matter");
    }
    #[test] fn an_edition_label_on_either_side_still_matches() {
        assert_eq!(pick("Control Ultimate Edition", &[c(870780, "Control")]).unwrap().id, 870780);
        assert_eq!(pick("The Witcher 3: Wild Hunt", &[c(292030, "The Witcher® 3: Wild Hunt — Game of the Year Edition")]).unwrap().id, 292030);
        assert_eq!(pick("Edition", &[c(5, "Edition")]).unwrap().id, 5, "a game called only 'Edition' keeps its name");
        assert_eq!(without_edition("reedition"), "reedition", "only a whole word is a label");
        assert_eq!(without_edition("skyrim special edition"), "skyrim");
    }
    #[test] fn an_exact_match_wins_over_an_edition_match() {
        let results = [c(1, "Skyrim Special Edition"), c(2, "Skyrim")];
        assert_eq!(pick("Skyrim", &results).unwrap().id, 2);
        assert_eq!(pick("Skyrim Special Edition", &results).unwrap().id, 1);
    }
    #[test] fn a_file_name_without_spaces_finds_the_game() {
        assert_eq!(pick("HollowKnight", &[c(367520, "Hollow Knight"), c(1030300, "Hollow Knight: Silksong")]).unwrap().id, 367520);
        assert_eq!(pick("hollowknight", &[c(367520, "Hollow Knight")]).unwrap().id, 367520);
    }
    #[test] fn a_near_miss_is_never_accepted() {
        assert_eq!(pick("Fortnite", &[c(1, "Fortnite Battle Royale Guide"), c(2, "Fortnite Soundtrack")]).unwrap_err(), NOT_ON_STEAM);
        assert_eq!(pick("Doom", &[c(1, "Doom Eternal"), c(2, "Doom 3")]).unwrap_err(), NOT_ON_STEAM);
        assert_eq!(pick("VALORANT", &[]).unwrap_err(), NOT_ON_STEAM);
        assert_eq!(pick("Hades", &[c(1145350, "Hades II")]).unwrap_err(), NOT_ON_STEAM);
        assert_eq!(pick("   ", &[c(1, "x")]).unwrap_err(), NOT_ON_STEAM);
    }
    #[test] fn two_different_games_with_the_same_name_are_left_alone() {
        assert_eq!(pick("Prey", &[c(480490, "Prey"), c(3970, "Prey")]).unwrap_err(), AMBIGUOUS);
        assert_eq!(pick("Prey", &[c(480490, "Prey"), c(480490, "Prey")]).unwrap().id, 480490, "the same entry twice is one game");
    }
    #[test] fn only_games_are_matched_not_bundles_or_packages() {
        let bundle = Candidate { id: 7, name: "Hades".into(), kind: "sub".into() };
        assert_eq!(pick("Hades", &[bundle.clone()]).unwrap_err(), NOT_ON_STEAM);
        assert_eq!(pick("Hades", &[bundle, c(1145360, "Hades")]).unwrap().id, 1145360);
    }
    #[test] fn the_steam_answer_is_read_even_without_a_type_field() {
        let answer: SearchAnswer = serde_json::from_str(r#"{"total":2,"items":[{"type":"app","name":"Hades","id":1145360,"tiny_image":"x","metascore":"93"},{"name":"Old","id":5}]}"#).unwrap();
        assert_eq!(answer.items.len(), 2);
        assert_eq!(answer.items[1].kind, "app");
        assert!(serde_json::from_str::<SearchAnswer>(r#"{"total":0}"#).unwrap().items.is_empty());
    }

    struct Fake { store: HashMap<String, Vec<Candidate>>, missing: Vec<String>, grid: HashMap<String, String>, key: bool, offline: bool, asked: Vec<String> }
    impl Fake { fn new() -> Self { Self { store: HashMap::new(), missing: vec![], grid: HashMap::new(), key: false, offline: false, asked: vec![] } } }
    impl Lookup for Fake {
        fn steam(&mut self, term: &str) -> Result<Vec<Candidate>, String> { self.asked.push(term.into()); if self.offline { return Err(OFFLINE.into()); } Ok(self.store.get(term).cloned().unwrap_or_default()) }
        fn picture(&mut self, url: &str) -> bool { !self.missing.iter().any(|m| url.contains(m.as_str())) }
        fn grid(&mut self, title: &str) -> Result<Option<(String, String)>, String> { if self.offline { return Err("x".into()); } Ok(self.grid.get(title).map(|u| (u.clone(), title.to_owned()))) }
        fn has_grid(&self) -> bool { self.key }
    }
    fn wanted(title: &str) -> Wanted { Wanted { id: format!("id-{title}"), title: title.into() } }

    #[test] fn a_run_fills_what_it_is_sure_of_and_explains_the_rest() {
        let mut fake = Fake::new();
        fake.store.insert("Hades".into(), vec![c(1145360, "Hades"), c(1145350, "Hades II")]);
        fake.store.insert("Hollow Knight".into(), vec![c(367520, "Hollow Knight")]);
        fake.store.insert("Prey".into(), vec![c(480490, "Prey"), c(3970, "Prey")]);
        let games = [wanted("Hades"), wanted("HollowKnight"), wanted("VALORANT"), wanted("Prey")];
        let mut seen = vec![];
        let out = run(&games, &mut fake, &|| false, &mut |done, total, title| seen.push((done, total, title.to_owned())));
        assert_eq!(out.found.iter().map(|f| f.cover.as_str()).collect::<Vec<_>>(), [steam_cover(1145360), steam_cover(367520)]);
        assert_eq!(out.found[0].banner, steam_banner(1145360));
        assert_eq!(out.found[1].matched, "Hollow Knight");
        assert_eq!(out.unmatched.iter().map(|u| (u.title.as_str(), u.reason.as_str())).collect::<Vec<_>>(), [("VALORANT", NOT_ON_STEAM), ("Prey", AMBIGUOUS)]);
        assert!(fake.asked.contains(&"Hollow Knight".to_string()), "the file name is searched as words");
        assert_eq!(seen.first().unwrap(), &(0, 4, "Hades".to_string()));
        assert_eq!(seen.last().unwrap(), &(4, 4, String::new()));
        assert!(!out.cancelled);
    }
    #[test] fn a_store_entry_without_a_cover_picture_is_not_used() {
        let mut fake = Fake::new();
        fake.store.insert("Old Game".into(), vec![c(42, "Old Game")]);
        fake.missing.push("/42/library_600x900".into());
        let out = run(&[wanted("Old Game")], &mut fake, &|| false, &mut |_, _, _| {});
        assert!(out.found.is_empty());
        assert_eq!(out.unmatched[0].reason, NO_PICTURE);
    }
    #[test] fn a_missing_banner_still_gives_the_cover() {
        let mut fake = Fake::new();
        fake.store.insert("Hades".into(), vec![c(1145360, "Hades")]);
        fake.missing.push("library_hero".into());
        let out = run(&[wanted("Hades")], &mut fake, &|| false, &mut |_, _, _| {});
        assert_eq!(out.found[0].cover, steam_cover(1145360));
        assert_eq!(out.found[0].banner, "");
    }
    #[test] fn steamgriddb_is_asked_only_with_a_key_and_only_for_what_steam_lacks() {
        let mut fake = Fake::new();
        fake.store.insert("Hades".into(), vec![c(1145360, "Hades")]);
        fake.grid.insert("VALORANT".into(), "https://cdn2.steamgriddb.com/grid/v.png".into());
        let games = [wanted("Hades"), wanted("VALORANT")];
        let without = run(&games, &mut fake, &|| false, &mut |_, _, _| {});
        assert_eq!(without.found.len(), 1);
        assert_eq!(without.unmatched[0].reason, NOT_ON_STEAM);
        fake.key = true;
        let with = run(&games, &mut fake, &|| false, &mut |_, _, _| {});
        assert_eq!(with.found.iter().map(|f| f.source.as_str()).collect::<Vec<_>>(), ["Steam", "SteamGridDB"]);
        assert_eq!(with.found[1].cover, "https://cdn2.steamgriddb.com/grid/v.png");
        fake.grid.clear();
        let neither = run(&[wanted("VALORANT")], &mut fake, &|| false, &mut |_, _, _| {});
        assert_eq!(neither.unmatched[0].reason, NOT_ON_STEAM_OR_GRID);
    }
    #[test] fn without_a_connection_every_game_says_so_and_nothing_is_set() {
        let mut fake = Fake::new(); fake.offline = true; fake.key = true;
        let out = run(&[wanted("Hades"), wanted("Celeste")], &mut fake, &|| false, &mut |_, _, _| {});
        assert!(out.found.is_empty());
        assert!(out.unmatched.iter().all(|u| u.reason == OFFLINE));
    }
    #[test] fn cancelling_stops_between_games_and_keeps_what_was_found() {
        let mut fake = Fake::new();
        fake.store.insert("Hades".into(), vec![c(1145360, "Hades")]);
        let calls = std::cell::Cell::new(0);
        let out = run(&[wanted("Hades"), wanted("Celeste"), wanted("Tunic")], &mut fake, &|| { calls.set(calls.get() + 1); calls.get() > 1 }, &mut |_, _, _| {});
        assert!(out.cancelled);
        assert_eq!(out.found.len(), 1);
        assert!(out.unmatched.is_empty());
    }
    #[test] fn requests_with_bad_ids_titles_or_keys_are_refused() {
        let good = Wanted { id: uuid::Uuid::new_v4().to_string(), title: "Hades".into() };
        assert!(validate(&[good.clone()], "").is_ok());
        assert!(validate(&[good.clone()], "abcDEF123").is_ok());
        assert!(validate(&[good.clone()], "has space").is_err());
        assert!(validate(&[Wanted { id: "../x".into(), title: "Hades".into() }], "").is_err());
        assert!(validate(&[Wanted { id: good.id.clone(), title: "  ".into() }], "").is_err());
    }
    #[test] fn the_answer_has_the_names_the_interface_reads() {
        let out = Outcome { found: vec![Found { id: "a".into(), cover: "c".into(), banner: "b".into(), source: "Steam".into(), matched: "Hades".into() }], unmatched: vec![Unmatched { id: "b".into(), title: "X".into(), reason: "r".into() }], cancelled: false };
        assert_eq!(serde_json::to_string(&out).unwrap(), r#"{"found":[{"id":"a","cover":"c","banner":"b","source":"Steam","matched":"Hades"}],"unmatched":[{"id":"b","title":"X","reason":"r"}],"cancelled":false}"#);
        let w: Wanted = serde_json::from_str(r#"{"id":"x","title":"Hades"}"#).unwrap();
        assert_eq!(w.title, "Hades");
    }
}
