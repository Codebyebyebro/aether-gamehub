//! A small log kept only on this PC, and the "Copy diagnostics" report built from it.
//!
//! The log records that something went wrong (a panic, a launch that failed, a background feature that could not
//! start, an error in the interface), never what the player is doing: no game list, no keys, no ids. Paths are
//! shortened so the Windows user name is not in what gets pasted. It is two files of at most 256 KB each, so it can
//! never grow without limit, and nothing here is ever sent anywhere.
use crate::models::Settings;
use std::{fs, io::Write, path::{Path, PathBuf}, sync::{Mutex, OnceLock, TryLockError}, time::Duration};

const FILE: &str = "aether-hub.log";
const OLD: &str = "aether-hub.old.log";
/// A file this big is moved to `OLD` (replacing the previous one) and a fresh one is started.
const LIMIT: u64 = 256 * 1024;
const ENTRY_MAX: usize = 600;
/// How many of the latest log lines the report carries.
const TAIL: usize = 60;

// ---------------------------------------------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------------------------------------------

/// Case-insensitive (ASCII) replace. Matches can only start at a character boundary because `needle` is valid UTF-8.
fn replace_ignoring_case(text: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() { return text.to_owned(); }
    let (t, n) = (text.as_bytes(), needle.as_bytes());
    let (mut out, mut i, mut last) = (String::with_capacity(text.len()), 0, 0);
    while i + n.len() <= t.len() {
        if t[i..i + n.len()].eq_ignore_ascii_case(n) { out.push_str(&text[last..i]); out.push_str(with); i += n.len(); last = i; } else { i += 1; }
    }
    out.push_str(&text[last..]);
    out
}

/// Replaces the Windows profile folder (`C:\Users\Name`, also with forward slashes or the `\\?\` prefix) with `~`.
fn hide_home(text: &str, home: Option<&str>) -> String {
    let Some(home) = home.map(str::trim).filter(|h| h.len() > 3) else { return text.to_owned(); };
    let home = home.trim_end_matches(['\\', '/']);
    let mut out = replace_ignoring_case(text, &format!(r"\\?\{home}"), "~");
    for form in [home.to_owned(), home.replace('\\', "/")] { out = replace_ignoring_case(&out, &form, "~"); }
    out
}

/// Blanks the value of anything that looks like a key or token in an address or a message (`key=…`, `token=…`). The
/// log never receives settings, so this is a second lock on the same door: an error text from a web request could
/// still carry the address it was made to.
fn hide_keys(text: &str) -> String {
    let mut out = text.to_owned();
    for name in ["apikey=", "api_key=", "access_token=", "token=", "secret=", "key="] {
        let (mut next, mut from) = (String::with_capacity(out.len()), 0);
        let bytes = out.as_bytes();
        let mut i = 0;
        while i + name.len() <= bytes.len() {
            if bytes[i..i + name.len()].eq_ignore_ascii_case(name.as_bytes()) {
                let value_start = i + name.len();
                let value_end = out[value_start..].find(|c: char| c.is_whitespace() || "&\"'),];".contains(c)).map_or(out.len(), |k| value_start + k);
                next.push_str(&out[from..value_start]);
                if value_end > value_start { next.push_str("***"); }
                from = value_end; i = value_end.max(i + 1);
            } else { i += 1; }
        }
        next.push_str(&out[from..]);
        out = next;
    }
    out
}

/// One line of text made safe to keep and to paste: no line breaks, no control characters, no profile folder, no
/// keys, and not longer than `ENTRY_MAX` characters.
pub fn clean(text: &str, home: Option<&str>) -> String {
    let lines = text.replace("\r\n", "\n").replace(['\n', '\r'], " / ");
    let flat: String = lines.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let mut s = hide_keys(&hide_home(&flat, home));
    if s.chars().count() > ENTRY_MAX { s = s.chars().take(ENTRY_MAX).collect::<String>() + "…"; }
    s
}

/// `2026-11-22 19:13:05 UTC` from seconds since 1970 (proleptic Gregorian calendar).
pub fn stamp(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let z = days + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC", rem / 3_600, rem % 3_600 / 60, rem % 60)
}

/// `<time> [kind] text`. The kind is a short fixed word chosen by the program, never player text.
pub fn line(secs: u64, kind: &str, text: &str, home: Option<&str>) -> String {
    let kind: String = kind.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(16).collect();
    format!("{} [{kind}] {}", stamp(secs), clean(text, home))
}

/// One process, as far as the memory figure needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Proc { pub pid: u32, pub parent: Option<u32>, pub bytes: u64 }
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Footprint { pub own: u64, pub helpers: u64, pub helper_count: usize }

/// Memory of `root` and of everything that descends from it (the web view's helper processes hang below Aether Hub).
pub fn footprint(procs: &[Proc], root: u32) -> Footprint {
    let mut family = vec![root];
    loop {
        let before = family.len();
        for p in procs { if let Some(parent) = p.parent { if family.contains(&parent) && !family.contains(&p.pid) { family.push(p.pid); } } }
        if family.len() == before { break; }
    }
    let own = procs.iter().find(|p| p.pid == root).map_or(0, |p| p.bytes);
    let helpers = procs.iter().filter(|p| p.pid != root && family.contains(&p.pid));
    let (count, bytes) = helpers.fold((0, 0), |(n, b), p| (n + 1, b + p.bytes));
    Footprint { own, helpers: bytes, helper_count: count }
}
fn mb(bytes: u64) -> u64 { (bytes + 524_288) / 1_048_576 }

/// "on, working" / "on, NOT working: why" / "off".
pub fn feature(wanted: bool, working: bool, problem: &str) -> String {
    match (wanted, working) {
        (false, _) => "off".into(),
        (true, true) => "on, working".into(),
        (true, false) if problem.trim().is_empty() => "on, NOT working".into(),
        (true, false) => format!("on, NOT working: {}", problem.trim()),
    }
}

fn on_off(on: bool) -> &'static str { if on { "on" } else { "off" } }

/// What the report says about the settings. Only choices that explain behaviour: never an id, a key, a profile name
/// or a folder, and not the game list.
pub fn settings_lines(s: &Settings) -> Vec<String> {
    vec![
        format!("View: {}, tile size {}", s.view, s.size),
        format!("After launching a game: {}", s.after_launch),
        format!("Performance mode: {}, sounds: {}, living colors: {}", on_off(s.performance), on_off(s.sounds), on_off(s.living_colors)),
        format!("Console mode: {}, rumble: {}, PS / Xbox button: {}, controller layouts: {}", on_off(s.console_mode), on_off(s.rumble), on_off(s.home_button), on_off(s.controller_layouts)),
        format!("Backups: {} (keeps {}), custom backup folder: {}", on_off(s.backup_enabled), s.backup_keep, if s.backup_dir.is_empty() { "no" } else { "yes" }),
        format!("Discord status: {}, Steam sync: {}, update check at start: {}", on_off(s.discord_enabled), on_off(s.steam_sync_enabled), on_off(s.check_updates)),
        format!("Extra scan folders: {}", s.folders.len()),
    ]
}

/// Everything the report needs, already reduced to text or numbers the program chose to share.
pub struct Facts {
    pub version: String, pub webview: String, pub windows: String, pub now: u64, pub data_dir: String,
    pub background: Vec<(String, String)>,
    pub library: Result<(usize, Vec<String>), String>,
    pub memory: Option<Footprint>,
    pub log: Vec<String>,
}

pub fn report(f: &Facts, home: Option<&str>) -> String {
    let mut out = vec![
        "Aether Hub diagnostics".to_owned(),
        format!("Made: {}", stamp(f.now)),
        format!("Version: {}", f.version),
        format!("Windows: {}", f.windows),
        format!("Web view (WebView2): {}", f.webview),
        format!("Data folder: {}", f.data_dir),
        String::new(), "Running in the background".to_owned(),
    ];
    for (name, state) in &f.background { out.push(format!("  {name}: {state}")); }
    out.push(String::new());
    match &f.library {
        Ok((games, lines)) => { out.push(format!("Library: {games} games")); out.extend(lines.iter().map(|l| format!("  {l}"))); }
        Err(e) => out.push(format!("Library: could not be read ({e})")),
    }
    if let Some(m) = &f.memory {
        out.push(String::new());
        out.push(format!("Memory now: Aether Hub {} MB + {} web view helper process{} {} MB = {} MB", mb(m.own), m.helper_count, if m.helper_count == 1 { "" } else { "es" }, mb(m.helpers), mb(m.own + m.helpers)));
        out.push("  (Windows working set; pages shared between processes are counted in each, so the total reads a little high)".to_owned());
    }
    out.push(String::new());
    if f.log.is_empty() { out.push("Log: nothing has been recorded, which is a good sign.".to_owned()); }
    else { out.push(format!("Log: the last {} entries ({FILE} in the data folder)", f.log.len())); out.extend(f.log.iter().map(|l| format!("  {l}"))); }
    out.push(String::new());
    out.push("Nothing here is sent anywhere. It has no keys or ids, but an error line can name a game's folder or file, so read it before you share it.".to_owned());
    // Every line passes through `clean` once more, so a path inside a value cannot reveal the user name either.
    out.iter().map(|l| hide_keys(&hide_home(l, home))).collect::<Vec<_>>().join("\n")
}

// ---------------------------------------------------------------------------------------------------------------
// The files
// ---------------------------------------------------------------------------------------------------------------

/// Adds one line, first moving a full log aside so it never grows past about twice `LIMIT` in total.
pub fn append(dir: &Path, text: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let file = dir.join(FILE);
    if fs::metadata(&file).map_or(0, |m| m.len()) >= LIMIT {
        let old = dir.join(OLD);
        let _ = fs::remove_file(&old);
        fs::rename(&file, &old)?;
    }
    let mut f = fs::OpenOptions::new().create(true).append(true).open(file)?;
    f.write_all(text.as_bytes())?;
    f.write_all(b"\n")
}

/// The last `n` lines across the old and the current file, oldest first.
pub fn tail(dir: &Path, n: usize) -> Vec<String> {
    let read = |name: &str| fs::read(dir.join(name)).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let all: Vec<String> = read(OLD).lines().chain(read(FILE).lines()).filter(|l| !l.trim().is_empty()).map(str::to_owned).collect();
    all[all.len().saturating_sub(n)..].to_vec()
}

struct Target { dir: PathBuf, home: Option<String> }
static TARGET: OnceLock<Target> = OnceLock::new();
static WRITING: Mutex<()> = Mutex::new(());

/// Says where the log lives. Until this has been called, `event` does nothing.
pub fn init(dir: PathBuf, home: Option<String>) { let _ = TARGET.set(Target { dir, home }); }
pub fn recent() -> Vec<String> { TARGET.get().map(|t| tail(&t.dir, TAIL)).unwrap_or_default() }

/// Records that something happened. Best effort by design: it never fails, never panics and never waits for long, so a
/// problem in the log can never become a second problem for the player.
pub fn event(kind: &str, text: &str) {
    let Some(target) = TARGET.get() else { return; };
    let entry = line(crate::util::now_secs(), kind, text, target.home.as_deref());
    // A panic raised while the log is being written reaches this function again on the same thread, so wait a
    // little and then give up rather than wait for ever for a lock the thread itself holds.
    for _ in 0..10 {
        match WRITING.try_lock() {
            Ok(_guard) => { let _ = append(&target.dir, &entry); return; }
            Err(TryLockError::Poisoned(poisoned)) => { let _guard = poisoned.into_inner(); let _ = append(&target.dir, &entry); return; }
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

/// Passes an error through unchanged after writing it to the log, for commands that return `Result<_, String>`.
pub fn logged<T>(kind: &str, result: Result<T, String>) -> Result<T, String> {
    if let Err(e) = &result { event(kind, e); }
    result
}

/// Writes every panic to the log before the usual handling carries on.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let what = payload.downcast_ref::<&str>().map(|s| (*s).to_owned()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "no message".into());
        let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_owned();
        event("panic", &format!("{what} (thread {thread}, {place})"));
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A folder of its own per test: tests run in parallel and must not share files.
    fn scratch(label: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("aether-diag-{}-{label}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
    const HOME: &str = r"C:\Users\Some One";

    #[test] fn dates_are_read_correctly_including_leap_days_and_the_far_end() {
        assert_eq!(stamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(stamp(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(stamp(1_700_000_000), "2023-11-14 22:13:20 UTC");
        assert_eq!(stamp(1_785_000_000), "2026-07-25 17:20:00 UTC");
        assert_eq!(stamp(4_102_444_799), "2099-12-31 23:59:59 UTC");
    }
    #[test] fn the_profile_folder_is_hidden_in_every_spelling_and_any_case() {
        for text in [r"C:\Users\Some One\AppData\x", r"c:\users\some one\AppData\x", "C:/Users/Some One/AppData/x", r"\\?\C:\Users\Some One\AppData\x"] {
            let c = clean(text, Some(HOME));
            assert!(c.starts_with('~') && !c.to_lowercase().contains("some one"), "{text} -> {c}");
        }
        assert_eq!(clean(r"D:\Games\a.exe", Some(HOME)), r"D:\Games\a.exe", "a path elsewhere is left alone");
        assert_eq!(clean("plain", None), "plain");
        assert_eq!(clean("C:\\", Some("C:\\")), "C:\\", "a root is never treated as a profile folder");
    }
    #[test] fn keys_and_tokens_in_an_address_or_message_are_blanked() {
        let c = clean("GET https://api.example/x?key=ABC123&steamid=7 failed; token=zzz end, apikey=Q)", None);
        assert!(!c.contains("ABC123") && !c.contains("zzz") && !c.contains("=Q"), "{c}");
        assert!(c.contains("key=***&steamid=7") && c.contains("token=*** end"), "only the values go: {c}");
        assert_eq!(clean("key=", None), "key=", "an empty value is left as it is");
    }
    #[test] fn an_entry_is_one_short_line_without_control_characters() {
        let c = clean("first\r\nsecond\tthird\u{7}", None);
        assert_eq!(c, "first / second third ");
        let long = clean(&"é".repeat(5_000), None);
        assert_eq!(long.chars().count(), ENTRY_MAX + 1, "cut on a character, with a marker");
        assert!(long.ends_with('…'));
    }
    #[test] fn a_log_line_has_a_time_a_safe_kind_and_the_cleaned_text() {
        assert_eq!(line(0, "launch", "no \u{0}way", None), "1970-01-01 00:00:00 UTC [launch] no  way");
        assert_eq!(line(0, "bad kind]\n", "x", None), "1970-01-01 00:00:00 UTC [badkind] x", "the kind cannot break the format");
    }
    #[test] fn memory_adds_up_the_program_and_everything_below_it_and_nothing_else() {
        let procs = [
            Proc { pid: 10, parent: Some(1), bytes: 80 << 20 },
            Proc { pid: 11, parent: Some(10), bytes: 100 << 20 },
            Proc { pid: 12, parent: Some(11), bytes: 50 << 20 },
            Proc { pid: 13, parent: Some(10), bytes: 30 << 20 },
            Proc { pid: 99, parent: Some(1), bytes: 500 << 20 },
            Proc { pid: 98, parent: Some(99), bytes: 500 << 20 },
        ];
        assert_eq!(footprint(&procs, 10), Footprint { own: 80 << 20, helpers: 180 << 20, helper_count: 3 });
        assert_eq!(footprint(&procs, 12), Footprint { own: 50 << 20, helpers: 0, helper_count: 0 });
        assert_eq!(footprint(&procs, 777), Footprint::default(), "a process that is not there");
        let looped = [Proc { pid: 1, parent: Some(2), bytes: 1 }, Proc { pid: 2, parent: Some(1), bytes: 1 }];
        assert_eq!(footprint(&looped, 1).helper_count, 1, "a loop of parents (a reused process number) ends");
    }
    #[test] fn a_feature_is_described_as_off_working_or_not_working_with_the_reason() {
        assert_eq!(feature(false, false, "ignored"), "off");
        assert_eq!(feature(false, true, ""), "off");
        assert_eq!(feature(true, true, ""), "on, working");
        assert_eq!(feature(true, false, ""), "on, NOT working");
        assert_eq!(feature(true, false, "  keys are taken  "), "on, NOT working: keys are taken");
    }

    #[test] fn the_log_grows_by_lines_and_the_tail_returns_the_latest_in_order() {
        let dir = scratch("tail");
        for i in 0..5 { append(&dir, &format!("line {i}")).unwrap(); }
        assert_eq!(tail(&dir, 3), ["line 2", "line 3", "line 4"]);
        assert_eq!(tail(&dir, 99).len(), 5);
        assert!(tail(&scratch("none"), 5).is_empty(), "no log yet is not an error");
    }
    #[test] fn a_full_log_is_moved_aside_and_only_one_old_copy_is_kept() {
        let dir = scratch("rotate");
        fs::write(dir.join(FILE), vec![b'a'; LIMIT as usize]).unwrap();
        append(&dir, "fresh").unwrap();
        assert_eq!(fs::read(dir.join(OLD)).unwrap().len(), LIMIT as usize, "the full file became the old one");
        assert_eq!(fs::read_to_string(dir.join(FILE)).unwrap(), "fresh\n");
        fs::write(dir.join(FILE), vec![b'b'; LIMIT as usize]).unwrap();
        append(&dir, "again").unwrap();
        assert!(fs::read(dir.join(OLD)).unwrap().iter().all(|b| *b == b'b'), "the previous old copy was replaced, not kept");
        let names: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert_eq!(tail(&dir, 5).last().map(String::as_str), Some("again"), "the tail reads across both files, newest last");
    }

    fn sample_facts() -> Facts {
        let mut s = Settings::default();
        s.steam_api_key = "STEAMKEY-SENTINEL".into(); s.steam_profile = "PROFILE-SENTINEL".into(); s.discord_app_id = "12345678901234567".into();
        s.wallpaper = r"C:\Users\Some One\pic-SENTINEL.png".into(); s.folders = vec![r"C:\Users\Some One\FOLDER-SENTINEL".into()];
        s.backup_dir = r"C:\Users\Some One\BACKUP-SENTINEL".into(); s.discord_enabled = true;
        Facts {
            version: "0.1.0".into(), webview: "131.0.1".into(), windows: "Windows 11 23H2 (build 22631.4460)".into(), now: 1_785_000_000,
            data_dir: format!(r"{HOME}\AppData\Roaming\com.aetherhub.launcher"),
            background: vec![("Tray".into(), feature(true, true, "")), ("Shortcut (Ctrl+Alt+Space)".into(), feature(true, false, r"Another program holds it. C:\Users\Some One\x"))],
            library: Ok((42, settings_lines(&s))),
            memory: Some(Footprint { own: 80 << 20, helpers: 190 << 20, helper_count: 4 }),
            log: vec!["2026-07-25 17:00:00 UTC [panic] boom at src\\x.rs:1".into()],
        }
    }
    #[test] fn the_report_has_what_a_helper_needs_and_none_of_the_private_things() {
        let text = report(&sample_facts(), Some(HOME));
        for wanted in ["Aether Hub diagnostics", "Version: 0.1.0", "Windows 11 23H2", "WebView2): 131.0.1", "Tray: on, working", "Shortcut (Ctrl+Alt+Space): on, NOT working: Another program holds it.", "Library: 42 games", "Discord status: on", "Memory now: Aether Hub 80 MB + 4 web view helper processes 190 MB = 270 MB", "[panic] boom"] {
            assert!(text.contains(wanted), "missing {wanted:?} in:\n{text}");
        }
        for secret in ["STEAMKEY-SENTINEL", "PROFILE-SENTINEL", "12345678901234567", "pic-SENTINEL", "FOLDER-SENTINEL", "BACKUP-SENTINEL", "Some One"] {
            assert!(!text.contains(secret), "{secret} leaked into:\n{text}");
        }
        assert!(text.contains("Extra scan folders: 1") && text.contains("custom backup folder: yes"), "counts and yes/no are fine");
        assert!(text.contains(r"~\AppData\Roaming\com.aetherhub.launcher"), "the data folder is shown without the user name");
    }
    #[test] fn the_report_copes_with_no_library_no_memory_and_no_log() {
        let mut f = sample_facts();
        f.library = Err("library.json is damaged".into()); f.memory = None; f.log.clear();
        let text = report(&f, Some(HOME));
        assert!(text.contains("Library: could not be read (library.json is damaged)") && !text.contains("Memory now") && text.contains("nothing has been recorded"), "{text}");
    }
    #[test] fn event_writes_nothing_until_a_folder_is_chosen_and_never_panics() {
        // The global target is not set in this test process on purpose: this must be a silent no-op.
        if TARGET.get().is_none() { event("test", "ignored"); assert!(recent().is_empty()); }
    }
    #[test] fn logged_hands_the_result_back_unchanged() {
        assert_eq!(logged("t", Ok::<_, String>(5)), Ok(5));
        assert_eq!(logged::<()>("t", Err("nope".into())), Err("nope".to_string()));
    }
}
