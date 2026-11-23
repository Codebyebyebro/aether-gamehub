//! Which games can no longer be found.
//!
//! Only a game that points at an `.exe` file on a drive can be judged. A Steam or Epic link is started by its own
//! store, which knows whether the game is installed, and a network share may be asleep rather than gone, so those
//! are never reported. A game on a drive that is not connected is reported as that (`Drive`), not as gone (`File`),
//! so nobody removes a game just because a USB drive is unplugged.
use crate::util;
use serde::Serialize;
use std::{collections::HashMap, sync::{mpsc, Arc}, time::{Duration, Instant}};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Absence {
    /// The drive is there but the file is not: it was moved, renamed or uninstalled.
    File,
    /// The drive itself is not available (unplugged, or a network drive that is off).
    Drive,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Absent { pub id: String, pub reason: Absence }

/// `Z:\` for `Z:\Games\a.exe`, also for forward slashes, any letter case and a `\\?\` prefix. Anything that is not a
/// path on a lettered drive (a link, a network share, a relative path) has none.
fn drive_root(target: &str) -> Option<String> {
    let path = util::strip_verbatim(target);
    let b = path.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')).then(|| format!("{}:\\", (b[0] as char).to_ascii_uppercase()))
}

/// Whether this target can be judged at all.
pub fn judged(target: &str) -> bool { !target.contains("://") && target.to_ascii_lowercase().ends_with(".exe") && drive_root(target).is_some() }

/// The verdict for one game: `None` means it is there, or that nothing can be said about it.
pub fn decide(target: &str, drive_up: &mut dyn FnMut(&str) -> bool, is_file: &dyn Fn(&str) -> bool) -> Option<Absence> {
    if !judged(target) { return None; }
    let root = drive_root(target)?;
    if !drive_up(&root) { return Some(Absence::Drive); }
    if is_file(&util::strip_verbatim(target)) { None } else { Some(Absence::File) }
}

/// Checks every game, one thread per drive so a drive that answers slowly (a network drive whose server is off can
/// take half a minute) cannot hold up the others, and stops waiting after `budget`. A game whose answer has not come
/// in by then is simply not reported: a slow disk must never freeze the library or cause a false alarm. `games` are
/// `(id, target)` pairs; the result keeps their order.
pub fn check_all<D, F>(games: Vec<(String, String)>, drive_up: D, is_file: F, budget: Duration) -> Vec<Absent>
where D: Fn(&str) -> bool + Send + Sync + 'static, F: Fn(&str) -> bool + Send + Sync + 'static {
    let mut by_drive: HashMap<String, Vec<(usize, String, String)>> = HashMap::new();
    for (position, (id, target)) in games.into_iter().enumerate() {
        if let (true, Some(root)) = (judged(&target), drive_root(&target)) { by_drive.entry(root).or_default().push((position, id, target)); }
    }
    let (drive_up, is_file) = (Arc::new(drive_up), Arc::new(is_file));
    let (tx, rx) = mpsc::channel::<(usize, Absent)>();
    for (_, items) in by_drive {
        let (tx, drive_up, is_file) = (tx.clone(), Arc::clone(&drive_up), Arc::clone(&is_file));
        std::thread::spawn(move || {
            // Each drive is asked once, however many games live on it.
            let mut asked: Option<bool> = None;
            let mut up = |root: &str| *asked.get_or_insert_with(|| (*drive_up)(root));
            for (position, id, target) in items {
                if let Some(reason) = decide(&target, &mut up, &*is_file) { if tx.send((position, Absent { id, reason })).is_err() { return; } }
            }
        });
    }
    drop(tx);
    let end = Instant::now() + budget;
    let mut found = Vec::new();
    while let Ok(item) = rx.recv_timeout(end.saturating_duration_since(Instant::now())) { found.push(item); }
    found.sort_by_key(|(position, _)| *position);
    found.into_iter().map(|(_, absent)| absent).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test] fn only_an_exe_on_a_lettered_drive_is_judged() {
        for yes in [r"C:\Games\A\a.exe", r"c:\games\a.EXE", "D:/Games/a.exe", r"\\?\E:\Games\a.exe"] { assert!(judged(yes), "{yes}"); }
        for no in ["steam://rungameid/1245620", "com.epicgames.launcher://apps/x?action=launch", r"\\nas\share\a.exe", r"\\?\UNC\nas\share\a.exe", r"Games\a.exe", r"C:\Games\a.lnk", r"C:\Games\a.bat", "C:", r"C:a.exe", "", "a.exe", r"C:\x.exe://y"] { assert!(!judged(no), "{no}"); }
    }
    #[test] fn the_drive_is_named_the_same_way_whatever_the_spelling() {
        for spelling in [r"d:\x\a.exe", "D:/x/a.exe", r"\\?\D:\x\a.exe"] { assert_eq!(drive_root(spelling).as_deref(), Some(r"D:\"), "{spelling}"); }
        assert_eq!(drive_root(r"\\nas\share\a.exe"), None);
    }
    #[test] fn a_missing_drive_is_a_drive_problem_and_the_file_is_not_even_looked_for() {
        let verdict = decide(r"Z:\g\a.exe", &mut |_| false, &|_| panic!("the file must not be looked for on a drive that is not there"));
        assert_eq!(verdict, Some(Absence::Drive));
    }
    #[test] fn on_a_working_drive_the_file_decides() {
        assert_eq!(decide(r"C:\g\a.exe", &mut |_| true, &|_| true), None);
        assert_eq!(decide(r"C:\g\a.exe", &mut |_| true, &|_| false), Some(Absence::File));
    }
    #[test] fn a_verbatim_path_is_looked_up_in_its_plain_form() {
        let seen = std::cell::RefCell::new(String::new());
        decide(r"\\?\C:\g\a.exe", &mut |_| true, &|p| { *seen.borrow_mut() = p.to_owned(); true });
        assert_eq!(*seen.borrow(), r"C:\g\a.exe");
    }
    #[test] fn what_cannot_be_judged_is_never_reported_and_never_touches_the_disk() {
        for target in ["steam://rungameid/1", r"\\nas\s\a.exe", "relative.exe", r"C:\g\a.lnk"] {
            assert_eq!(decide(target, &mut |_| panic!("no disk access"), &|_| panic!("no disk access")), None, "{target}");
        }
    }

    fn games() -> Vec<(String, String)> {
        [("1", r"C:\g\present.exe"), ("2", r"C:\g\gone.exe"), ("3", "steam://rungameid/9"), ("4", r"D:\g\one.exe"), ("5", r"D:\g\two.exe"), ("6", r"D:\g\three.exe"), ("7", r"\\nas\s\x.exe"), ("8", r"C:\g\also-gone.exe")]
            .iter().map(|(i, t)| (i.to_string(), t.to_string())).collect()
    }
    #[test] fn every_game_is_judged_and_the_answer_keeps_the_library_order() {
        let drive_asks = Arc::new(AtomicUsize::new(0)); let counter = Arc::clone(&drive_asks);
        let found = check_all(games(), move |root| { counter.fetch_add(1, Ordering::SeqCst); root.starts_with('C') }, |file| file.ends_with("present.exe"), Duration::from_secs(5));
        assert_eq!(found, vec![
            Absent { id: "2".into(), reason: Absence::File },
            Absent { id: "4".into(), reason: Absence::Drive },
            Absent { id: "5".into(), reason: Absence::Drive },
            Absent { id: "6".into(), reason: Absence::Drive },
            Absent { id: "8".into(), reason: Absence::File },
        ]);
        assert_eq!(drive_asks.load(Ordering::SeqCst), 2, "C: and D: are each asked once, not once per game");
    }
    #[test] fn a_drive_that_never_answers_cannot_hold_up_the_others() {
        let started = Instant::now();
        let found = check_all(games(), |root| { if root.starts_with('D') { std::thread::sleep(Duration::from_secs(3)); } true }, |file| file.ends_with("present.exe"), Duration::from_millis(400));
        assert!(started.elapsed() < Duration::from_millis(2_000), "gave up on time: {:?}", started.elapsed());
        assert_eq!(found.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["2", "8"], "the fast drive is reported; the slow one is left unreported, not guessed");
    }
    #[test] fn an_empty_library_or_one_of_links_checks_nothing() {
        assert!(check_all(vec![], |_| panic!("no disk"), |_| panic!("no disk"), Duration::from_secs(1)).is_empty());
        assert!(check_all(vec![("1".into(), "steam://rungameid/1".into())], |_| panic!("no disk"), |_| panic!("no disk"), Duration::from_secs(1)).is_empty());
    }
}
