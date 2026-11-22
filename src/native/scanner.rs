use crate::{models::{FilteredItem, Game, ScanDetail, ScanResult}, scan_control::Watch, storage, util};
use serde_json::{Map, Value};
use std::{collections::HashSet, fs, path::{Component, Path, PathBuf}};

/// `.exe` files smaller than this are ignored: helpers, stubs and bootstrappers are tiny. Store entries are exempt.
pub const MIN_EXE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 6;
const MAX_VISITED: usize = 400_000;
const MAX_FILTERED: usize = 200;

/// The strict candidate filters. Everything here is pure so it can be tested without a disk.
pub mod rules {
    use super::MIN_EXE_BYTES;

    /// Rejected when they appear anywhere in a file or folder name: `SetupFiles`, `unins000`, `UnityCrashHandler64`.
    pub const BLOCKED_SUBSTRINGS: &[&str] = &[
        "unins", "setup", "install", "update", "patch", "crash", "dump", "redist", "dxsetup", "vcredist", "helper",
        "launcher", "cef", "unitycrashhandler", "dotnet", "browser", "pdf", "ccna", "study", "school", "download",
        "support", "driver", "config", "diag", "sdk",
    ];
    /// Too short to match safely inside other words (`isa` is in "Isaac", `temp` in "Temple", `doc` in "Docker"),
    /// so these only count as whole words. Each entry is (word, keyword reported to the user).
    pub const BLOCKED_WORDS: &[(&str, &str)] = &[
        ("doc", "doc"), ("docs", "doc"), ("docx", "docx"), ("document", "doc"), ("documents", "doc"),
        ("isa", "isa"), ("temp", "temp"), ("tmp", "temp"), ("temporary", "temp"),
    ];
    /// Programs that are never games, matched exactly (ignoring case, spaces and punctuation). `browser` and `doc`
    /// do not catch `chrome.exe` or `winword.exe`, so the well-known ones are listed here.
    pub const KNOWN_NON_GAMES: &[&str] = &[
        "chrome", "msedge", "msedgewebview2", "firefox", "brave", "opera", "operagx", "vivaldi", "iexplore", "waterfox",
        "librewolf", "chromium", "winword", "excel", "powerpnt", "outlook", "onenote", "msaccess", "mspub", "acrobat",
        "acrord32", "acrord64", "foxitreader", "foxitpdfreader", "sumatrapdf", "notepad", "wordpad", "soffice",
        "swriter", "scalc", "simpress", "code", "devenv", "python", "pythonw", "java", "javaw", "node", "git",
        "powershell", "pwsh", "cmd", "conhost", "windowsterminal", "mintty", "packettracer", "wireshark", "putty",
        "securecrt", "gns3", "virtualbox", "vboxmanage", "vmware", "vmplayer", "7zfm", "7zg", "winrar", "vlc",
        "explorer", "taskmgr", "regedit", "mspaint", "calc", "beservice", "beservicex64", "easyanticheat",
        "easyanticheateos", "oalinst",
    ];

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub enum Kind { TooSmall, Keyword, KnownTool }
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum Verdict { Accept, Reject { kind: Kind, reason: String } }

    /// `PDFReader` -> [pdf, reader]; `UnityCrashHandler64` -> [unity, crash, handler, 64]; `ISA Server` -> [isa, server].
    pub fn tokens(segment: &str) -> Vec<String> {
        let chars: Vec<char> = segment.chars().collect();
        let (mut out, mut current) = (Vec::new(), String::new());
        for (i, &c) in chars.iter().enumerate() {
            if !c.is_alphanumeric() { if !current.is_empty() { out.push(std::mem::take(&mut current)); } continue; }
            if let Some(&prev) = i.checked_sub(1).and_then(|p| chars.get(p)).filter(|_| !current.is_empty()) {
                let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
                let boundary = (prev.is_lowercase() && c.is_uppercase())
                    || (prev.is_uppercase() && c.is_uppercase() && next_lower)
                    || (prev.is_alphabetic() != c.is_alphabetic());
                if boundary { out.push(std::mem::take(&mut current)); }
            }
            current.push(c);
        }
        if !current.is_empty() { out.push(current); }
        out.iter().map(|t| t.to_lowercase()).collect()
    }
    /// The blacklisted keyword a single file or folder name contains, if any.
    pub fn blocked_keyword(segment: &str) -> Option<&'static str> {
        let lower = segment.to_lowercase();
        if let Some(keyword) = BLOCKED_SUBSTRINGS.iter().find(|k| lower.contains(**k)) { return Some(keyword); }
        let words = tokens(segment);
        BLOCKED_WORDS.iter().find(|(word, _)| words.iter().any(|t| t == word)).map(|(_, keyword)| *keyword)
    }
    pub fn known_non_game(stem: &str) -> bool {
        let normal: String = stem.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
        KNOWN_NON_GAMES.contains(&normal.as_str())
    }
    pub fn format_size(bytes: u64) -> String {
        let (mut value, mut unit) = (bytes as f64, 0);
        while value >= 1024.0 && unit < 3 { value /= 1024.0; unit += 1; }
        if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", ["B", "KB", "MB", "GB"][unit]) }
    }
    /// `folders` are the folder names between the scan root and the file (the root itself is the user's own choice,
    /// so it is never judged); `stem` is the file name without `.exe`.
    pub fn evaluate(folders: &[&str], stem: &str, size: u64) -> Verdict {
        let reject = |kind, reason: String| Verdict::Reject { kind, reason };
        if known_non_game(stem) { return reject(Kind::KnownTool, format!("“{stem}” is a browser, office, or system program")); }
        if let Some(keyword) = blocked_keyword(stem) { return reject(Kind::Keyword, format!("File name contains “{keyword}”")); }
        for folder in folders {
            if let Some(keyword) = blocked_keyword(folder) { return reject(Kind::Keyword, format!("Folder “{folder}” contains “{keyword}”")); }
        }
        if size < MIN_EXE_BYTES { return reject(Kind::TooSmall, format!("Smaller than {} ({})", format_size(MIN_EXE_BYTES), format_size(size))); }
        Verdict::Accept
    }
}

struct Scan {
    out: ScanResult,
    /// Install folders already represented by a Steam or Epic entry.
    managed: Vec<PathBuf>,
}

/// Reads the store install lists, then walks each custom folder. Stops early, returning an empty result marked
/// `cancelled`, once `watch` says so; it checks between sources and before every walked entry.
pub fn scan(folders: Vec<String>, watch: &mut Watch) -> Result<ScanResult, String> {
    if folders.len() > 100 { return Err("Choose no more than 100 custom folders".into()); }
    let mut scan = Scan { out: ScanResult::default(), managed: vec![] };
    let mut seen_roots = HashSet::new();
    let roots: Vec<String> = folders.into_iter().filter(|f| seen_roots.insert(f.to_lowercase())).collect();
    let steps = roots.len() as u32 + 1;
    watch.step(0, steps, String::new(), 0);
    let stores: [fn(&mut Scan); 3] = [steam, epic, riot];
    for store in stores { if watch.cancelled() { break; } store(&mut scan); }
    for (i, folder) in roots.iter().enumerate() {
        if watch.cancelled() { break; }
        watch.step(i as u32 + 1, steps, util::clean_path(Path::new(folder)), scan.out.games.len());
        custom(&mut scan, Path::new(folder), watch);
    }
    if watch.cancelled() { return Ok(ScanResult { cancelled: true, ..ScanResult::default() }); }
    let out = &mut scan.out;
    let mut seen = HashSet::new(); let before = out.games.len();
    out.games.retain(|g| seen.insert(storage::identity(&g.target, &g.args)));
    out.stats.duplicates += (before - out.games.len()) as u32;
    let kept: HashSet<String> = out.games.iter().map(|g| g.id.clone()).collect();
    out.details.retain(|id, _| kept.contains(id));
    let mut seen = HashSet::new();
    out.filtered.retain(|f| seen.insert(storage::identity(&f.game.target, &f.game.args)));
    out.games.sort_by_key(|g| g.title.to_lowercase());
    out.filtered.sort_by(|a, b| b.detail.size_bytes.cmp(&a.detail.size_bytes));
    watch.finish(out.games.len());
    Ok(scan.out)
}
fn read_json(path: &Path) -> Result<Value, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 { return Err("Manifest is unexpectedly large".into()); }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
pub(crate) fn program_data() -> PathBuf { std::env::var_os("PROGRAMDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\ProgramData")) }
pub fn steam_roots() -> Vec<PathBuf> {
    let mut roots = vec![];
    #[cfg(windows)] {
        use winreg::{enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE}, RegKey};
        for (hive, sub, name) in [(HKEY_CURRENT_USER, "Software\\Valve\\Steam", "SteamPath"), (HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath")] {
            if let Ok(key) = RegKey::predef(hive).open_subkey(sub) { if let Ok(path) = key.get_value::<String, _>(name) { roots.push(PathBuf::from(path)); } }
        }
    }
    for base in ["PROGRAMFILES(X86)", "PROGRAMFILES"] { if let Some(p) = std::env::var_os(base) { roots.push(PathBuf::from(p).join("Steam")); } }
    roots.push(PathBuf::from("C:\\Program Files (x86)\\Steam")); roots
}
/// Every Steam library (the Steam folder itself plus the extra drives from `libraryfolders.vdf`).
pub fn steam_libraries(warnings: &mut Vec<String>) -> Vec<PathBuf> {
    let mut libraries = vec![];
    for root in steam_roots().into_iter().filter(|p| p.is_dir()) {
        libraries.push(root.clone());
        let file = root.join("steamapps/libraryfolders.vdf");
        if !file.exists() { continue; }
        match fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|s| parse_vdf(&s)) {
            Ok(value) => {
                if let Some(folders) = value.get("libraryfolders").or_else(|| value.get("LibraryFolders")).and_then(Value::as_object) {
                    for (key, node) in folders { if key.bytes().all(|b| b.is_ascii_digit()) { if let Some(p) = node.as_str().or_else(|| node.get("path").and_then(Value::as_str)) { libraries.push(PathBuf::from(p)); } } }
                }
            }
            Err(e) => warnings.push(format!("Steam library list could not be read: {e}")),
        }
    }
    let mut seen = HashSet::new();
    libraries.retain(|l| seen.insert(l.to_string_lossy().to_lowercase()));
    libraries
}
/// Runtimes and redistributables Steam installs as "apps": they are not games.
fn steam_tool(appid: &str, name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let proton = n.strip_prefix("proton").is_some_and(|rest| rest.starts_with(['-', ' ']) && {
        let word = rest.trim_start_matches(['-', ' ']);
        word.starts_with(|c: char| c.is_ascii_digit()) || ["experimental", "hotfix", "easyanticheat", "battleye"].iter().any(|k| word.starts_with(k))
    });
    appid == "228980" || proton || n.starts_with("steam linux runtime") || n.starts_with("steamworks common") || n.starts_with("source sdk base") || n.starts_with("steam controller configs")
}
fn steam(scan: &mut Scan) {
    for library in steam_libraries(&mut scan.out.warnings) {
        let apps = library.join("steamapps"); let Ok(entries) = fs::read_dir(&apps) else { continue; };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path(); let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("appmanifest_") || path.extension().is_none_or(|e| e != "acf") { continue; }
            let parsed = fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| parse_vdf(&s));
            match parsed {
                Ok(value) => {
                    let state = &value["AppState"];
                    let Some(id) = state["appid"].as_str().filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())) else { continue; };
                    let Some(title) = state["name"].as_str() else { continue; };
                    let Some(dir) = state["installdir"].as_str() else { continue; };
                    let install = apps.join("common").join(dir);
                    if !install.is_dir() { continue; }
                    scan.managed.push(install.clone());
                    if steam_tool(id, title) { scan.out.stats.known_tools += 1; continue; }
                    // A Steam manifest is a trusted signal: these skip the size and keyword rules.
                    let mut game = Game::new(title.into(), "Steam", format!("steam://rungameid/{id}")); game.steam_art(id);
                    let size = state["SizeOnDisk"].as_str().and_then(|s| s.parse::<u64>().ok()).filter(|s| *s > 0);
                    scan.out.details.insert(game.id.clone(), ScanDetail { size_bytes: size, path: util::clean_path(&install), source: "Steam".into() });
                    scan.out.games.push(game);
                }
                Err(_) => scan.out.warnings.push(format!("Skipped unreadable Steam manifest: {name}")),
            }
        }
    }
}
fn epic(scan: &mut Scan) {
    let folder = program_data().join("Epic/EpicGamesLauncher/Data/Manifests");
    let Ok(entries) = fs::read_dir(folder) else { return; };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path(); if path.extension().is_none_or(|e| e != "item") { continue; }
        match read_json(&path) {
            Ok(v) => {
                if v["bIsIncompleteInstall"].as_bool() == Some(true) { continue; }
                let Some(title) = v["DisplayName"].as_str() else { continue; };
                let Some(location) = v["InstallLocation"].as_str() else { continue; };
                if !Path::new(location).is_dir() { continue; }
                let Some(app) = v["AppName"].as_str() else { continue; };
                scan.managed.push(PathBuf::from(location));
                let encoded: String = url::form_urlencoded::byte_serialize(app.as_bytes()).collect();
                let target = format!("com.epicgames.launcher://apps/{encoded}?action=launch&silent=true");
                let game = Game::new(title.into(), "Epic", target);
                scan.out.details.insert(game.id.clone(), ScanDetail { size_bytes: v["InstallSize"].as_u64().filter(|s| *s > 0), path: util::clean_path(Path::new(location)), source: "Epic".into() });
                scan.out.games.push(game);
            }
            Err(_) => scan.out.warnings.push(format!("Skipped unreadable Epic manifest: {}", entry.file_name().to_string_lossy())),
        }
    }
}
fn riot(scan: &mut Scan) {
    let base = program_data().join("Riot Games"); let installs = base.join("RiotClientInstalls.json");
    if !installs.exists() { return; }
    let data = match read_json(&installs) { Ok(v) => v, Err(e) => { scan.out.warnings.push(format!("Riot installation list could not be read: {e}")); return; } };
    let default_client = data["rc_default"].as_str().or_else(|| data["rc_live"].as_str()).unwrap_or("");
    let mut products: Vec<(String, String)> = vec![];
    if let Some(associated) = data["associated_client"].as_object() {
        for (product, client) in associated { if let Some(client) = client.as_str() { products.push((product.to_lowercase(), client.into())); } }
    }
    for (product, directory) in [("valorant", "valorant.live"), ("league of legends", "league_of_legends.live")] {
        if base.join("Metadata").join(directory).is_dir() { products.push((product.into(), default_client.into())); }
    }
    let mut found = false;
    for (product, client) in products {
        if !Path::new(&client).is_file() { continue; }
        let (title, id) = if product.contains("valorant") { ("VALORANT", "valorant") } else if product.contains("league of legends") || product.contains("league_of_legends") { ("League of Legends", "league_of_legends") } else { continue; };
        let mut game = Game::new(title.into(), "Riot", client.clone()); game.args = format!("--launch-product={id} --launch-patchline=live");
        game.working_dir = Path::new(&client).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        scan.out.details.insert(game.id.clone(), ScanDetail { size_bytes: fs::metadata(&client).ok().map(|m| m.len()), path: util::clean_path(Path::new(&client)), source: "Riot".into() });
        scan.out.games.push(game); found = true;
    }
    if !found { scan.out.warnings.push("Riot Client was detected, but no supported installed product could be identified. Add its game manually with the client’s launch arguments.".into()); }
}
/// Folders that are never worth walking. Keyword folders (`Downloads`, `Setup Files`...) are not pruned here: their
/// executables are walked and rejected with a reason, so a wrongly rejected game can still be rescued in the review.
fn never_descend(name: &str) -> bool {
    ["windows", "$recycle.bin", "system volume information", "node_modules", "windowsapps", "winsxs", ".git", "_commonredist", "steamapps"].contains(&name.to_ascii_lowercase().as_str())
}
fn is_exe(path: &Path) -> bool { path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) }
fn custom(scan: &mut Scan, folder: &Path, watch: &mut Watch) {
    if !folder.is_absolute() || !folder.is_dir() { scan.out.warnings.push(format!("Custom scan folder is unavailable: {}", folder.display())); return; }
    let walker = walkdir::WalkDir::new(folder).max_depth(MAX_DEPTH).follow_links(false).into_iter()
        .filter_entry(|e| e.depth() == 0 || !never_descend(&e.file_name().to_string_lossy()));
    let (mut denied, mut visited) = (0, 0usize);
    for entry in walker {
        if !watch.tick(scan.out.games.len()) { return; }
        let entry = match entry { Ok(e) => e, Err(_) => { denied += 1; continue; } };
        visited += 1;
        if visited > MAX_VISITED { scan.out.warnings.push(format!("The scan stopped after {MAX_VISITED} items in {}. Choose a narrower folder to see the rest.", folder.display())); break; }
        if scan.out.games.len() >= 20000 { scan.out.warnings.push("The scan reached the 20,000-candidate safety limit".into()); break; }
        if !entry.file_type().is_file() || !is_exe(entry.path()) { continue; }
        scan.out.stats.executables_seen += 1;
        let path = entry.path();
        if scan.managed.iter().any(|m| util::path_starts_with(path, m)) || path.components().any(|c| matches!(c, Component::Normal(n) if n.eq_ignore_ascii_case("steamapps"))) { scan.out.stats.already_managed += 1; continue; }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let folders: Vec<String> = path.strip_prefix(folder).ok().and_then(Path::parent).map(|p| p.components().filter_map(|c| match c { Component::Normal(n) => Some(n.to_string_lossy().into_owned()), _ => None }).collect()).unwrap_or_default();
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let verdict = rules::evaluate(&folders.iter().map(String::as_str).collect::<Vec<_>>(), &stem, size);
        let (kind, reason) = match &verdict { rules::Verdict::Accept => (None, String::new()), rules::Verdict::Reject { kind, reason } => (Some(*kind), reason.clone()) };
        match kind {
            Some(rules::Kind::TooSmall) => { scan.out.stats.too_small += 1; continue; }
            Some(rules::Kind::KnownTool) => scan.out.stats.known_tools += 1,
            Some(rules::Kind::Keyword) => scan.out.stats.blacklisted += 1,
            None => {}
        }
        // A rejected file below the size floor is noise; a large one may be a real game with an unlucky name.
        if kind.is_some() && (size < MIN_EXE_BYTES || scan.out.filtered.len() >= MAX_FILTERED || kind == Some(rules::Kind::KnownTool)) { continue; }
        let Ok(canonical) = path.canonicalize() else { continue; };
        let clean = util::clean_path(&canonical);
        let title = stem.replace(['_', '-'], " ").trim().to_owned();
        if title.is_empty() { continue; }
        let mut game = Game::new(title, "Custom", clean.clone());
        game.working_dir = canonical.parent().map(util::clean_path).unwrap_or_default();
        let detail = ScanDetail { size_bytes: Some(size), path: clean, source: "Folder".into() };
        if kind.is_some() { scan.out.filtered.push(FilteredItem { game, detail, reason }); }
        else { scan.out.details.insert(game.id.clone(), detail); scan.out.games.push(game); }
    }
    if denied > 0 { scan.out.warnings.push(format!("Skipped {denied} inaccessible entries in {}", folder.display())); }
}

pub fn parse_vdf(text: &str) -> Result<Value, String> {
    if text.len() > 8 * 1024 * 1024 { return Err("VDF exceeds size limit".into()); }
    let mut tokens = vec![]; let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() || ch == '\u{feff}' { continue; }
        if ch == '/' && chars.peek() == Some(&'/') { for c in chars.by_ref() { if c == '\n' { break; } } continue; }
        if ch == '{' || ch == '}' { tokens.push(ch.to_string()); continue; }
        let mut token = String::new();
        if ch == '"' {
            let mut closed = false;
            while let Some(c) = chars.next() { if c == '"' { closed = true; break; } if c == '\\' { match chars.peek().copied() { Some('\\') | Some('"') => { token.push(chars.next().unwrap()); }, _ => token.push(c) } } else { token.push(c); } }
            if !closed { return Err("Unclosed VDF string".into()); }
        } else { token.push(ch); while chars.peek().is_some_and(|c| !c.is_whitespace() && *c != '{' && *c != '}') { token.push(chars.next().unwrap()); } }
        tokens.push(token);
    }
    fn object(tokens: &[String], pos: &mut usize, depth: usize, nested: bool) -> Result<Value, String> {
        if depth > 32 { return Err("VDF nesting exceeds limit".into()); }
        let mut map = Map::new();
        while *pos < tokens.len() {
            if tokens[*pos] == "}" { if !nested { return Err("Unexpected closing brace".into()); } *pos += 1; return Ok(Value::Object(map)); }
            let key = tokens[*pos].clone(); *pos += 1;
            let next = tokens.get(*pos).ok_or("VDF value is missing")?; *pos += 1;
            let value = if next == "{" { object(tokens, pos, depth + 1, true)? } else if next == "}" { return Err("VDF value is missing".into()); } else { Value::String(next.clone()) };
            map.insert(key, value);
        }
        if nested { return Err("VDF object is not closed".into()); }
        Ok(Value::Object(map))
    }
    object(&tokens, &mut 0, 0, false)
}
#[cfg(test)] mod tests {
    use super::{rules::*, *};
    use std::{fs::File, sync::{atomic::{AtomicBool, Ordering}, Arc}, time::Duration};
    const MB: u64 = 1024 * 1024;
    fn reason(v: Verdict) -> String { match v { Verdict::Reject { reason, .. } => reason, Verdict::Accept => "accepted".into() } }

    #[test] fn modern_libraries() { let v = parse_vdf(r#""libraryfolders" { "0" { "path" "C:\\Steam" } "1" { "path" "D:\\Games" } }"#).unwrap(); assert_eq!(v["libraryfolders"]["1"]["path"], "D:\\Games"); }
    #[test] fn legacy_libraries() { let v = parse_vdf(r#""LibraryFolders" { "1" "D:\\Steam" }"#).unwrap(); assert_eq!(v["LibraryFolders"]["1"], "D:\\Steam"); }
    #[test] fn rejects_broken() { assert!(parse_vdf("\"AppState\" {").is_err()); assert!(parse_vdf("\"unclosed").is_err()); }

    #[test] fn every_keyword_from_the_brief_is_blocked() {
        for keyword in ["unins", "setup", "install", "update", "patch", "crash", "dump", "redist", "dxsetup", "vcredist", "helper", "launcher", "cef", "unitycrashhandler", "dotnet", "browser", "pdf", "ccna", "study", "school", "download", "support", "driver", "config", "diag", "sdk"] {
            assert!(blocked_keyword(&format!("My{keyword}Thing")).is_some() && blocked_keyword(&keyword.to_uppercase()).is_some(), "{keyword}");
        }
        for word in ["doc", "docx", "isa", "temp"] { assert!(blocked_keyword(word).is_some() && blocked_keyword(&format!("old {word} files")).is_some(), "{word}"); }
    }
    #[test] fn installers_helpers_and_runtimes_are_rejected_by_name() {
        for name in ["unins000", "setup", "Setup_x64", "vcredist_x64", "dxsetup", "UnityCrashHandler64", "crashpad_handler", "CrashReportClient", "notification_helper", "cefsubprocess", "UE4PrereqSetup_x64", "EasyAntiCheat_Setup", "dotnet-runtime", "Updater", "MinecraftLauncher", "Patch_1.2", "dump64"] {
            assert!(matches!(evaluate(&[], name, 50 * MB), Verdict::Reject { kind: Kind::Keyword, .. }), "{name}");
        }
    }
    #[test] fn browsers_office_and_school_tools_are_rejected() {
        for name in ["chrome", "msedge", "Firefox", "brave", "WINWORD", "Acrobat", "AcroRd32", "Notepad++", "PacketTracer", "putty", "BEService_x64"] {
            assert!(matches!(evaluate(&[], name, 90 * MB), Verdict::Reject { kind: Kind::KnownTool, .. }), "{name}");
        }
    }
    #[test] fn school_and_download_folders_reject_everything_beneath_them() {
        assert!(reason(evaluate(&["School", "CCNA", "Labs"], "lab01", 90 * MB)).contains("Folder “School”"));
        assert!(reason(evaluate(&["My Documents", "Games"], "game", 90 * MB)).contains("doc"));
        assert!(reason(evaluate(&["Downloads"], "game", 90 * MB)).contains("download"));
        assert!(reason(evaluate(&["ISA Server 2006"], "isa", 90 * MB)).contains("isa"));
        assert!(reason(evaluate(&["Temp"], "game", 90 * MB)).contains("temp"));
    }
    #[test] fn ordinary_games_pass_including_names_that_merely_contain_a_short_keyword() {
        for name in ["hollow_knight", "Hades", "Stardew Valley", "The Binding of Isaac", "Temple Run", "Tempest 4000", "Docker Tycoon", "Sisyphus", "Ferrari Racing", "Terraria", "eldenring", "Cyberpunk2077"] {
            assert_eq!(evaluate(&["Games", name], name, 30 * MB), Verdict::Accept, "{name}");
        }
    }
    #[test] fn the_size_floor_is_two_megabytes_inclusive() {
        assert!(reason(evaluate(&[], "Hades", MIN_EXE_BYTES - 1)).starts_with("Smaller than 2.0 MB"));
        assert_eq!(evaluate(&[], "Hades", MIN_EXE_BYTES), Verdict::Accept);
        assert!(matches!(evaluate(&[], "setup", 10), Verdict::Reject { kind: Kind::Keyword, .. }), "the more useful reason wins");
    }
    #[test] fn tokenizer_splits_the_way_file_names_are_written() {
        assert_eq!(tokens("PDFReader"), ["pdf", "reader"]);
        assert_eq!(tokens("UnityCrashHandler64"), ["unity", "crash", "handler", "64"]);
        assert_eq!(tokens("ISA Server_2006-x64"), ["isa", "server", "2006", "x", "64"]);
        assert_eq!(tokens("Isaac"), ["isaac"]);
        assert!(tokens("").is_empty() && tokens("___").is_empty());
    }
    #[test] fn known_limitation_names_containing_a_long_keyword_are_filtered_not_lost() {
        // "patch" and "crash" are substrings in real titles. They are rejected as asked, and the scan keeps them in
        // the "Filtered out" list so the review can still import them (see `walker_*` below).
        assert!(blocked_keyword("Dispatch").is_some() && blocked_keyword("Crash Bandicoot").is_some());
    }
    #[test] fn sizes_read_naturally() { assert_eq!(format_size(512), "512 B"); assert_eq!(format_size(2 * MB), "2.0 MB"); assert_eq!(format_size(1536 * MB), "1.5 GB"); }
    #[test] fn steam_runtimes_are_not_games() {
        assert!(steam_tool("228980", "Steamworks Common Redistributables") && steam_tool("1", "Proton 9.0") && steam_tool("1", "Proton Experimental") && steam_tool("1", "Steam Linux Runtime 3.0 (sniper)"));
        assert!(!steam_tool("1", "Proton Pulse") && !steam_tool("1", "ELDEN RING") && !steam_tool("1", "Crash Bandicoot N. Sane Trilogy"));
    }

    fn sparse(path: &Path, size: u64) { fs::create_dir_all(path.parent().unwrap()).unwrap(); File::create(path).unwrap().set_len(size).unwrap(); }
    #[test] fn walker_applies_every_rule_relative_to_the_chosen_root() {
        let temp = std::env::temp_dir().join(format!("aether-scan-{}", uuid::Uuid::new_v4().simple()));
        // The root's own name contains "download" and "setup": it is the user's choice, so it is never judged.
        let root = temp.join("Downloads and Setup");
        for (rel, size) in [
            ("Games/Hades/Hades.exe", 3 * MB), ("Games/Hades/UnityCrashHandler64.exe", 3 * MB), ("Games/Hades/tiny.exe", 100 * 1024),
            ("Games/Half Life/hl.exe", 5 * MB), ("Games/Crash Bandicoot/CrashBandicoot.exe", 4 * MB), ("School/CCNA/Labs/lab01.exe", 4 * MB),
            ("Installers/setup.exe", 4 * MB), ("Browsers/chrome.exe", 4 * MB), ("Games/notes.txt", MB), ("Games/guide.pdf", 3 * MB),
            ("steamapps/common/Managed/managed.exe", 9 * MB), ("node_modules/x/x.exe", 9 * MB),
        ] { sparse(&root.join(rel), size); }
        let mut scan = Scan { out: ScanResult::default(), managed: vec![] };
        custom(&mut scan, &root, &mut Watch::detached());
        let mut titles: Vec<_> = scan.out.games.iter().map(|g| g.title.as_str()).collect(); titles.sort();
        assert_eq!(titles, ["Hades", "hl"]);
        let hades = scan.out.games.iter().find(|g| g.title == "Hades").unwrap();
        let detail = &scan.out.details[&hades.id];
        assert_eq!((detail.size_bytes, detail.source.as_str()), (Some(3 * MB), "Folder"));
        assert!(!detail.path.starts_with(r"\\?\") && detail.path.ends_with("Hades.exe"));
        let mut filtered: Vec<_> = scan.out.filtered.iter().map(|f| f.game.title.as_str()).collect(); filtered.sort();
        assert_eq!(filtered, ["CrashBandicoot", "UnityCrashHandler64", "lab01", "setup"], "large keyword hits are kept for rescue; tiny files and known programs are not");
        let s = &scan.out.stats;
        assert_eq!((s.too_small, s.blacklisted, s.known_tools, s.already_managed), (1, 4, 1, 0));
        assert_eq!(s.executables_seen, 8, "steamapps and node_modules are never walked; non-exe files are ignored");
        let _ = fs::remove_dir_all(temp);
    }
    #[test] fn walker_skips_folders_a_store_already_manages() {
        let temp = std::env::temp_dir().join(format!("aether-scan-{}", uuid::Uuid::new_v4().simple()));
        sparse(&temp.join("Epic/Fortnite/FortniteClient.exe"), 3 * MB); sparse(&temp.join("Other/Game/Game.exe"), 3 * MB);
        let mut scan = Scan { out: ScanResult::default(), managed: vec![temp.join("Epic/Fortnite")] };
        custom(&mut scan, &temp, &mut Watch::detached());
        assert_eq!(scan.out.games.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(), ["Game"]);
        assert_eq!(scan.out.stats.already_managed, 1);
        let _ = fs::remove_dir_all(temp);
    }
    #[test] fn scan_is_deterministic_and_removes_duplicates_from_overlapping_roots() {
        let temp = std::env::temp_dir().join(format!("aether-scan-{}", uuid::Uuid::new_v4().simple()));
        sparse(&temp.join("Games/Hades/Hades.exe"), 4 * MB);
        let result = scan(vec![temp.join("Games").to_string_lossy().into_owned(), temp.join("Games/Hades").to_string_lossy().into_owned()], &mut Watch::detached()).unwrap();
        let ours: Vec<_> = result.games.iter().filter(|g| g.target.contains("Hades.exe") && g.target.starts_with(temp.to_string_lossy().as_ref())).collect();
        assert_eq!(ours.len(), 1);
        assert!(result.stats.duplicates >= 1 && result.details.contains_key(&ours[0].id));
        let _ = fs::remove_dir_all(temp);
    }
    #[test] fn a_cancelled_scan_stops_walking_and_returns_nothing() {
        let temp = std::env::temp_dir().join(format!("aether-scan-{}", uuid::Uuid::new_v4().simple()));
        for i in 0..400 { sparse(&temp.join(format!("Games/Game{i:03}/Game{i:03}.exe")), if i < 10 { 3 * MB } else { 1024 }); }
        // Cancel from inside a progress report, as the interface does while a walk is under way.
        let cancel = Arc::new(AtomicBool::new(false));
        let (flag, mut reports) = (cancel.clone(), 0);
        let mut watch = Watch::new(cancel.clone(), Duration::ZERO, |p| { reports += 1; if p.checked >= 128 { flag.store(true, Ordering::SeqCst); } });
        let mut partial = Scan { out: ScanResult::default(), managed: vec![] };
        custom(&mut partial, &temp, &mut watch);
        drop(watch);
        assert!(reports >= 2, "{reports}");
        assert!(partial.out.stats.executables_seen < 100, "the walk stopped right after the cancel ({} of 400 seen)", partial.out.stats.executables_seen);
        let result = scan(vec![temp.to_string_lossy().into_owned()], &mut Watch::new(cancel, Duration::ZERO, |_| {})).unwrap();
        assert!(result.cancelled && result.games.is_empty() && result.filtered.is_empty() && result.details.is_empty());
        let _ = fs::remove_dir_all(temp);
    }
    #[test] fn progress_names_each_folder_once_and_ends_complete() {
        let temp = std::env::temp_dir().join(format!("aether-scan-{}", uuid::Uuid::new_v4().simple()));
        sparse(&temp.join("A/Hades/Hades.exe"), 3 * MB); sparse(&temp.join("B/Celeste/Celeste.exe"), 3 * MB);
        // "a" is the same folder as "A" to Windows, so it is searched once.
        let folders = ["A", "B", "a"].map(|f| temp.join(f).to_string_lossy().into_owned()).to_vec();
        let mut seen = vec![];
        let result = scan(folders, &mut Watch::new(Arc::new(AtomicBool::new(false)), Duration::MAX, |p| seen.push(p.clone()))).unwrap();
        assert_eq!(seen.iter().map(|p| (p.step, p.steps)).collect::<Vec<_>>(), [(0, 3), (1, 3), (2, 3), (3, 3)]);
        assert!(seen[0].folder.is_empty() && seen[1].folder.ends_with('A') && seen[2].folder.ends_with('B') && seen[3].folder.is_empty());
        let ours = result.games.iter().filter(|g| g.target.starts_with(temp.to_string_lossy().as_ref())).count();
        assert!(!result.cancelled && ours == 2);
        assert!(seen[3].found as usize >= 2 && seen[3].checked >= 6, "{:?}", seen[3]);
        let _ = fs::remove_dir_all(temp);
    }
}
