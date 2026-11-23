use crate::{models::{FilteredItem, Game, ScanDetail, ScanResult}, scan_control::Watch, storage, util};
use serde_json::{Map, Value};
use std::{collections::{HashMap, HashSet}, fs, path::{Component, Path, PathBuf}};

/// `.exe` files smaller than this are ignored: helpers, stubs and bootstrappers are tiny. Store entries are exempt.
pub const MIN_EXE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 6;
const MAX_VISITED: usize = 400_000;
const MAX_FILTERED: usize = 200;

/// The strict candidate filters. Everything here is pure so it can be tested without a disk.
pub mod rules {
    use super::MIN_EXE_BYTES;
    use std::collections::HashSet;

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
        ("isa", "isa"), ("temp", "temp"), ("tmp", "temp"), ("temporary", "temp"), ("inst", "inst"),
        ("tool", "tool"), ("tools", "tool"), ("editor", "editor"),
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
        "easyanticheateos", "oalinst", "aetherhub",
    ];

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub enum Kind { TooSmall, Keyword, KnownTool, Installer, Program, NoGameFiles }
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

    // ---- Where a file lives, and what sits next to it ---------------------------------------------------------------
    // Names alone cannot tell `adb.exe` or `bash.exe` from a game, and a drive scan finds thousands of programs. So
    // installed software (Program Files, AppData...) and downloaded files are offered only when game files sit next to
    // them. A folder that says "games" is trusted, as before.

    /// Folder names that mean "games live here" wherever they appear in the path, even above the chosen folder. Launchers
    /// are left out on purpose: `GOG Galaxy` and `Battle.net` hold the launcher itself, their games sit in a `Games` folder.
    pub const GAME_FOLDERS: &[&str] = &[
        "games", "my games", "steamlibrary", "epic games", "epicgames", "ea games", "origin games", "gog games", "riot games",
        "ubisoft games", "rockstar games", "xboxgames",
    ];
    /// Folders where installed software lives. Games can live here too, but then they must show game files.
    pub const SYSTEM_FOLDERS: &[&str] = &["program files", "program files (x86)", "programdata", "windows", "windowsapps", "$recycle.bin"];
    /// Folders that only wrap an executable (`Game/Binaries/Win64/Game.exe`): the game's own files are one level up.
    const WRAPPER_FOLDERS: &[&str] = &["bin", "bin32", "bin64", "binaries", "win64", "win32", "x64", "x86", "x86_64", "release", "shipping", "retail"];
    /// The score at which game files count as proof.
    pub const ENOUGH: u32 = 3;

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub enum Placement { GameFolder, Program, Downloads { loose: bool }, Other }

    /// `parts` are the names in a full path, outermost first, with the file name last. A "games" folder anywhere wins;
    /// then installed-software folders (a hidden `.folder` such as `.cargo`, and AppData except its Temp folder); then
    /// Downloads, where `loose` means the file sits directly in it.
    pub fn placement(parts: &[&str]) -> Placement {
        let low: Vec<String> = parts.iter().map(|p| p.to_lowercase()).collect();
        let folders = &low[..low.len().saturating_sub(1)];
        if folders.iter().any(|p| GAME_FOLDERS.contains(&p.as_str())) { return Placement::GameFolder; }
        for (i, p) in folders.iter().enumerate() {
            let temp = folders.get(i + 1).map(String::as_str) == Some("local") && folders.get(i + 2).map(String::as_str) == Some("temp");
            if SYSTEM_FOLDERS.contains(&p.as_str()) || (p.len() > 1 && p.starts_with('.')) || (p == "appdata" && !temp) { return Placement::Program; }
        }
        match folders.iter().rposition(|p| p == "downloads") {
            Some(i) => Placement::Downloads { loose: i + 1 == folders.len() },
            None => Placement::Other,
        }
    }

    /// `Rainmeter-4.5.28`, `python 3.13.6 amd64`, `Setup (1)`: a version number or a browser's duplicate counter in the
    /// name means a downloaded installer or tool, not an installed game.
    pub fn looks_downloaded(stem: &str) -> bool {
        let version = stem.as_bytes().windows(3).any(|w| w[0].is_ascii_digit() && w[1] == b'.' && w[2].is_ascii_digit());
        let copy = stem.trim_end().strip_suffix(')').and_then(|s| s.rsplit_once('(')).is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        version || copy
    }

    /// The lower-cased names inside a game's folder, merged with the folders above it that only wrap the executable.
    #[derive(Default, Debug, Clone)]
    pub struct Listing { pub files: HashSet<String>, pub dirs: HashSet<String> }
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct Evidence { pub score: u32, pub notes: Vec<&'static str> }

    pub fn is_wrapper_folder(name: &str) -> bool { WRAPPER_FOLDERS.contains(&name.to_lowercase().as_str()) }

    const STORE_SDK_FILES: &[&str] = &["eossdk-win64-shipping.dll", "galaxypeer64.dll", "galaxy64.dll", "discord_game_sdk.dll", "upc_r2_loader64.dll", "uplay_r1_loader64.dll"];
    const MIDDLEWARE_FILES: &[&str] = &["fmod.dll", "fmodstudio.dll", "fmod64.dll", "fmodex64.dll", "bink2w64.dll", "binkw64.dll", "physxcommon_64.dll", "physx3_x64.dll"];
    const DIRECTX_FILES: &[&str] = &["xinput1_3.dll", "xinput9_1_0.dll", "d3dx9_43.dll", "d3dx11_43.dll", "d3dcompiler_43.dll", "x3daudio1_7.dll", "xaudio2_7.dll"];
    const GAME_DATA_FOLDERS: &[&str] = &["content", "assets", "maps", "levels", "textures", "shaders", "sounds", "music", "mods", "movies", "videos", "localization", "streamingassets", "paks", "savegames", "saves", "screenshots", "characters", "audio", "cutscenes"];
    const ARCHIVE_EXTENSIONS: &[&str] = &[".pak", ".pck", ".wad", ".bsa", ".vpk", ".bank", ".unity3d", ".assets", ".big", ".ba2", ".arc", ".xnb", ".rpf"];

    /// Looks for the files games ship with. Engine and store files are strong proof (3 points). Folders of sounds or levels
    /// and data archives add at most 2 together, so a media or office program with an `assets` folder is never enough.
    pub fn evidence(stem: &str, found: &Listing) -> Evidence {
        let stem = stem.to_lowercase();
        let has = |f: &str| found.files.contains(f);
        let dir = |d: &str| found.dirs.contains(d);
        let any_file = |names: &[&str]| names.iter().any(|n| found.files.contains(*n));
        let mut e = Evidence::default();
        let mut add = |points: u32, why: &'static str| { e.score += points; e.notes.push(why); };
        if any_file(&["steam_api64.dll", "steam_api.dll", "steam_appid.txt"]) { add(3, "Steam game files"); }
        if has("unityplayer.dll") || dir(&format!("{stem}_data")) || dir("monobleedingedge") { add(3, "Unity game files"); }
        if stem.ends_with("-win64-shipping") || stem.ends_with("-win32-shipping") || (dir("engine") && (dir("content") || dir("binaries") || found.files.iter().any(|f| f.starts_with("manifest_") && f.ends_with(".txt")))) { add(3, "Unreal Engine files"); }
        if found.files.iter().any(|f| f.ends_with(".pck")) { add(3, "Godot game files"); }
        if has("data.win") { add(3, "GameMaker game files"); }
        if found.files.iter().any(|f| f.starts_with("rgss") && f.ends_with(".dll")) || dir("renpy") { add(3, "RPG Maker or Ren'Py files"); }
        if any_file(STORE_SDK_FILES) { add(3, "Store or online-service files"); }
        if any_file(MIDDLEWARE_FILES) { add(3, "Game audio, video or physics libraries"); }
        if any_file(DIRECTX_FILES) { add(2, "DirectX game runtime files"); }
        let folders = GAME_DATA_FOLDERS.iter().filter(|d| found.dirs.contains(**d)).count() as u32;
        let archives = u32::from(found.files.iter().any(|f| ARCHIVE_EXTENSIONS.iter().any(|x| f.ends_with(*x))));
        if folders + archives > 0 { add((folders + archives).min(2), "Game data folders or archives"); }
        e
    }

    /// Combines where the file lives with its game-file score. A file loose in Downloads is dropped whatever sits beside it
    /// (that folder is a dumping ground, so neighbours prove nothing). Elsewhere, enough game files accept it; without them
    /// installed programs and versioned installers outside Downloads are dropped, and a file inside a Downloads subfolder is
    /// kept under "Filtered out" in case it is a game that ships none of the files above (zipped indie games often carry a
    /// version in the folder name).
    pub fn place_verdict(place: Placement, stem: &str, score: u32) -> Verdict {
        let reject = |kind, reason: &str| Verdict::Reject { kind, reason: reason.to_owned() };
        match place {
            Placement::GameFolder => Verdict::Accept,
            Placement::Downloads { loose: true } => reject(Kind::Installer, "Sits directly in a Downloads folder, so it is most likely an installer or tool"),
            _ if score >= ENOUGH => Verdict::Accept,
            Placement::Program => reject(Kind::Program, "Installed program with no game files next to it"),
            Placement::Other if looks_downloaded(stem) => reject(Kind::Installer, "The name has a version number, so it is a downloaded installer or tool"),
            Placement::Downloads { .. } => reject(Kind::NoGameFiles, "No game engine, store or data files found next to it"),
            Placement::Other => Verdict::Accept,
        }
    }

    /// Whether `place_verdict` can change its answer by looking at the folder, so the scanner reads the disk only then.
    pub fn needs_proof(place: Placement, stem: &str) -> bool {
        match place {
            Placement::GameFolder | Placement::Downloads { loose: true } => false,
            Placement::Program | Placement::Downloads { .. } => true,
            Placement::Other => looks_downloaded(stem),
        }
    }
}

/// Reads one folder's entry names (lower-cased, at most 5,000) for the game-file check.
fn list_dir(dir: &Path) -> rules::Listing {
    let mut listing = rules::Listing::default();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten().take(5000) {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        match entry.file_type() { Ok(t) if t.is_dir() => { listing.dirs.insert(name); } Ok(_) => { listing.files.insert(name); } Err(_) => {} }
    }
    listing
}
/// The game-file evidence for an executable: its own folder plus the folders above it that only wrap it (`bin`, `Win64`).
/// `cache` keeps each folder's listing so a folder full of executables is read once.
fn game_files(path: &Path, stem: &str, cache: &mut HashMap<PathBuf, rules::Listing>) -> rules::Evidence {
    if cache.len() > 4000 { cache.clear(); }
    let mut merged = rules::Listing::default();
    let mut dir = path.parent();
    for _ in 0..4 {
        let Some(d) = dir else { break };
        let listing = cache.entry(d.to_path_buf()).or_insert_with(|| list_dir(d));
        merged.files.extend(listing.files.iter().cloned());
        merged.dirs.extend(listing.dirs.iter().cloned());
        if !d.file_name().is_some_and(|n| rules::is_wrapper_folder(&n.to_string_lossy())) { break; }
        dir = d.parent();
    }
    rules::evidence(stem, &merged)
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
    let mut listings: HashMap<PathBuf, rules::Listing> = HashMap::new();
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
        let mut verdict = rules::evaluate(&folders.iter().map(String::as_str).collect::<Vec<_>>(), &stem, size);
        if verdict == rules::Verdict::Accept {
            // The name rules passed. Now judge where the file lives and, for installed programs and downloads, what sits next to it.
            let parts: Vec<String> = path.components().filter_map(|c| match c { Component::Normal(n) => Some(n.to_string_lossy().into_owned()), _ => None }).collect();
            let place = rules::placement(&parts.iter().map(String::as_str).collect::<Vec<_>>());
            let score = if rules::needs_proof(place, &stem) { game_files(path, &stem, &mut listings).score } else { 0 };
            verdict = rules::place_verdict(place, &stem, score);
        }
        let (kind, reason) = match &verdict { rules::Verdict::Accept => (None, String::new()), rules::Verdict::Reject { kind, reason } => (Some(*kind), reason.clone()) };
        match kind {
            Some(rules::Kind::TooSmall) => { scan.out.stats.too_small += 1; continue; }
            Some(rules::Kind::Installer) => { scan.out.stats.installers += 1; continue; }
            Some(rules::Kind::Program) => { scan.out.stats.programs += 1; continue; }
            Some(rules::Kind::KnownTool) => scan.out.stats.known_tools += 1,
            Some(rules::Kind::Keyword) => scan.out.stats.blacklisted += 1,
            Some(rules::Kind::NoGameFiles) => scan.out.stats.no_game_files += 1,
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
    /// A fresh scratch folder spelled the way the scanner reports paths (canonical, no `\\?\`). A raw temp path can differ:
    /// on GitHub's Windows runners it uses 8.3 short names ("RUNNER~1"), so comparing targets against it never matches.
    fn scratch() -> PathBuf { PathBuf::from(util::clean_path(&std::env::temp_dir().canonicalize().unwrap())).join(format!("aether-scan-{}", uuid::Uuid::new_v4().simple())) }
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
        let temp = scratch();
        sparse(&temp.join("Games/Hades/Hades.exe"), 4 * MB);
        let result = scan(vec![temp.join("Games").to_string_lossy().into_owned(), temp.join("Games/Hades").to_string_lossy().into_owned()], &mut Watch::detached()).unwrap();
        let ours: Vec<_> = result.games.iter().filter(|g| g.target.contains("Hades.exe") && g.target.starts_with(temp.to_string_lossy().as_ref())).collect();
        assert_eq!(ours.len(), 1, "under {} found {:?}", temp.display(), result.games.iter().map(|g| g.target.as_str()).collect::<Vec<_>>());
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
        let temp = scratch();
        sparse(&temp.join("A/Hades/Hades.exe"), 3 * MB); sparse(&temp.join("B/Celeste/Celeste.exe"), 3 * MB);
        // "a" is the same folder as "A" to Windows, so it is searched once.
        let folders = ["A", "B", "a"].map(|f| temp.join(f).to_string_lossy().into_owned()).to_vec();
        let mut seen = vec![];
        let result = scan(folders, &mut Watch::new(Arc::new(AtomicBool::new(false)), Duration::MAX, |p| seen.push(p.clone()))).unwrap();
        assert_eq!(seen.iter().map(|p| (p.step, p.steps)).collect::<Vec<_>>(), [(0, 3), (1, 3), (2, 3), (3, 3)]);
        assert!(seen[0].folder.is_empty() && seen[1].folder.ends_with('A') && seen[2].folder.ends_with('B') && seen[3].folder.is_empty());
        let ours = result.games.iter().filter(|g| g.target.starts_with(temp.to_string_lossy().as_ref())).count();
        assert!(!result.cancelled && ours == 2, "under {} found {:?}", temp.display(), result.games.iter().map(|g| g.target.as_str()).collect::<Vec<_>>());
        assert!(seen[3].found as usize >= 2 && seen[3].checked >= 6, "{:?}", seen[3]);
        let _ = fs::remove_dir_all(temp);
    }

    // ---- Where a file lives and what sits next to it ---------------------------------------------------------------
    fn listing(files: &[&str], dirs: &[&str]) -> Listing { Listing { files: files.iter().map(|s| s.to_string()).collect(), dirs: dirs.iter().map(|s| s.to_string()).collect() } }
    fn place(full: &str) -> Placement { placement(&full.split(['\\', '/']).filter(|s| !s.is_empty() && !s.ends_with(':')).collect::<Vec<_>>()) }
    fn dropped(v: Verdict) -> Option<Kind> { match v { Verdict::Reject { kind, .. } => Some(kind), Verdict::Accept => None } }

    #[test] fn placement_reads_the_whole_path_not_only_the_part_below_the_chosen_folder() {
        for program in [r"C:\Program Files\Google\Play Games\current\emulator\adb.exe", r"C:\Program Files (x86)\Steam\steam.exe", r"C:\Users\Sam\AppData\Local\Programs\Tool\tool.exe",
                        r"C:\Users\Sam\AppData\Roaming\App\app.exe", r"C:\Users\Sam\.cargo\bin\cargo.exe", r"C:\ProgramData\Thing\thing.exe", r"C:\Windows\System32\calc.exe"] {
            assert_eq!(place(program), Placement::Program, "{program}");
        }
        assert_eq!(place(r"D:\Games\Hades\Hades.exe"), Placement::GameFolder);
        assert_eq!(place(r"C:\Program Files (x86)\Ubisoft\Ubisoft Game Launcher\games\Far Cry\fc.exe"), Placement::GameFolder, "a games folder wins over Program Files");
        assert_eq!(place(r"C:\Users\Sam\Downloads\x.exe"), Placement::Downloads { loose: true });
        assert_eq!(place(r"C:\Users\Sam\Downloads\Cool Game\cg.exe"), Placement::Downloads { loose: false });
        assert_eq!(place(r"E:\Stuff\Cool\cg.exe"), Placement::Other);
        assert_eq!(place(r"C:\Users\Sam\Desktop\Cool\cg.exe"), Placement::Other);
        assert_eq!(place(r"C:\Users\Sam\AppData\Local\Temp\run\x.exe"), Placement::Other, "the tests below build their fixtures in Temp, which is not an installed program");
        assert_eq!(place(r"/home/sam/Play Games/x.exe"), Placement::Other, "only a folder named exactly “Games” counts");
    }
    #[test] fn a_version_number_or_copy_counter_marks_a_download() {
        for name in ["Rainmeter-4.5.28", "python 3.13.6 amd64", "OBS-Studio-30.1", "tool v2.0", "Setup (1)", "thing(12)"] { assert!(looks_downloaded(name), "{name}"); }
        for name in ["Hades", "Cyberpunk2077", "Half-Life 2", "NBA 2K24", "Fallout4", "Rainbow Six (Siege)", "Mount & Blade II", "Portal 2"] { assert!(!looks_downloaded(name), "{name}"); }
    }
    #[test] fn engine_store_and_runtime_files_prove_a_game() {
        let proves = |stem: &str, l: Listing| evidence(stem, &l).score >= ENOUGH;
        assert!(proves("Hollow Knight", listing(&["unityplayer.dll"], &["hollow knight_data"])));
        assert!(proves("Hades", listing(&["steam_api64.dll"], &[])));
        assert!(proves("Tunic", listing(&["eossdk-win64-shipping.dll"], &[])));
        assert!(proves("Game-Win64-Shipping", listing(&[], &[])));
        assert!(proves("Game", listing(&["manifest_nonufsfiles_win64.txt"], &["engine", "game"])));
        assert!(proves("Celeste", listing(&["fmod.dll"], &[])));
        assert!(proves("Undertale", listing(&["data.win"], &[])));
        assert!(proves("Godot Game", listing(&["godot game.pck"], &[])));
        assert!(proves("To The Moon", listing(&["rgss301.dll"], &[])));
        assert!(!proves("xinput only", listing(&["xinput1_3.dll"], &[])), "DirectX files alone are 2 points");
    }
    #[test] fn a_tool_with_asset_folders_is_never_enough_proof() {
        let e = evidence("editor", &listing(&["resources.pak"], &["assets", "sounds", "music", "textures", "shaders", "localization", "content"]));
        assert_eq!(e.score, 2, "data folders and archives are capped at 2 points together: {e:?}");
        assert_eq!(evidence("tool", &listing(&["tool.exe", "readme.txt"], &["lib", "plugins"])), Evidence::default());
    }
    #[test] fn placement_and_proof_decide_what_is_offered() {
        let (loose, nested) = (Placement::Downloads { loose: true }, Placement::Downloads { loose: false });
        assert_eq!(place_verdict(Placement::GameFolder, "adb", 0), Verdict::Accept, "a games folder is trusted, as before");
        assert_eq!(place_verdict(Placement::Program, "hades", ENOUGH), Verdict::Accept);
        assert_eq!(dropped(place_verdict(Placement::Program, "adb", ENOUGH - 1)), Some(Kind::Program));
        assert_eq!(dropped(place_verdict(loose, "tool", 0)), Some(Kind::Installer));
        assert_eq!(dropped(place_verdict(loose, "Hades", 9)), Some(Kind::Installer), "a loose file never borrows proof from the shared folder");
        assert_eq!(place_verdict(nested, "Hades", ENOUGH), Verdict::Accept);
        assert_eq!(dropped(place_verdict(nested, "Hades", 0)), Some(Kind::NoGameFiles), "kept under Filtered out, not lost");
        assert_eq!(dropped(place_verdict(nested, "Hades 1.2.3", 0)), Some(Kind::NoGameFiles), "a versioned game folder in Downloads is kept for review");
        assert_eq!(place_verdict(Placement::Other, "Hades", 0), Verdict::Accept, "no evidence either way: the name rules already passed");
        assert_eq!(dropped(place_verdict(Placement::Other, "Rainmeter-4.5.28", 0)), Some(Kind::Installer));
        assert_eq!(place_verdict(Placement::Other, "Rainmeter-4.5.28", ENOUGH), Verdict::Accept, "a versioned name with real game files next to it is kept");
        assert!(!needs_proof(Placement::GameFolder, "x") && !needs_proof(Placement::Downloads { loose: true }, "x") && !needs_proof(Placement::Other, "x"));
        assert!(needs_proof(Placement::Program, "x") && needs_proof(nested, "x") && needs_proof(Placement::Other, "x 1.0"));
    }
    #[test] fn modding_tools_and_editors_are_rejected_but_stay_rescuable() {
        for (folders, name) in [(&["Repentance", "tools", "RoomEditor"][..], "RoomEditor"), (&["Isaac"][..], "IsaacAnimationEditor"), (&["Dev Tools"][..], "viewer")] {
            assert!(matches!(evaluate(folders, name, 9 * MB), Verdict::Reject { kind: Kind::Keyword, .. }), "{name}");
        }
        assert_eq!(evaluate(&["Games", "Toolbox Tycoon"], "ToolboxTycoon", 30 * MB), Verdict::Accept, "whole words only: “Toolbox” is not “tool”");
        assert!(matches!(evaluate(&["Games", "Tools Up"], "ToolsUp", 30 * MB), Verdict::Reject { kind: Kind::Keyword, .. }), "a real title with the word is filtered, not lost: see the walker tests");
    }
    #[test] fn the_launcher_never_offers_itself() {
        for name in ["Aether Hub", "aether-hub", "AetherHub"] { assert!(matches!(evaluate(&[], name, 6 * MB), Verdict::Reject { kind: Kind::KnownTool, .. }), "{name}"); }
        assert!(matches!(evaluate(&[], "wps_office_inst", 6 * MB), Verdict::Reject { kind: Kind::Keyword, .. }));
        assert_eq!(evaluate(&[], "Instinct", 6 * MB), Verdict::Accept, "“inst” matches whole words only");
    }

    /// Builds a folder tree under a scratch root and runs the real walker over `root`.
    fn walk(temp: &Path, root: &Path, files: &[(&str, u64)]) -> Scan {
        for (rel, size) in files { sparse(&temp.join(rel), *size); }
        let mut scan = Scan { out: ScanResult::default(), managed: vec![] };
        custom(&mut scan, root, &mut Watch::detached());
        scan
    }
    fn titles(scan: &Scan) -> Vec<String> { let mut t: Vec<_> = scan.out.games.iter().map(|g| g.title.clone()).collect(); t.sort(); t }
    fn filtered(scan: &Scan) -> Vec<String> { let mut t: Vec<_> = scan.out.filtered.iter().map(|f| f.game.title.clone()).collect(); t.sort(); t }

    #[test] fn a_scan_of_program_files_offers_only_programs_that_ship_game_files() {
        let temp = scratch();
        let root = temp.join("Program Files");
        let scan = walk(&temp, &root, &[
            ("Program Files/Android/adb.exe", 6 * MB), ("Program Files/Rainmeter/Rainmeter.exe", 5 * MB),
            // Media or office tools ship asset folders, but that alone is not proof.
            ("Program Files/Office Suite/suite.exe", 9 * MB), ("Program Files/Office Suite/resources.pak", MB),
            ("Program Files/Office Suite/assets/a.png", 10), ("Program Files/Office Suite/sounds/a.wav", 10), ("Program Files/Office Suite/music/a.mp3", 10),
            ("Program Files/Cool Game/CoolGame.exe", 40 * MB), ("Program Files/Cool Game/steam_api64.dll", MB),
            ("Program Files/Hollow/Hollow.exe", 20 * MB), ("Program Files/Hollow/UnityPlayer.dll", 20 * MB), ("Program Files/Hollow/Hollow_Data/level0", 10),
            ("Program Files/Tunic/Binaries/Win64/Tunic-Win64-Shipping.exe", 90 * MB),
            ("Program Files/Bundled/Bundled.exe", 30 * MB), ("Program Files/Bundled/FMOD.dll", MB),
            // The game's own files sit one level above an executable in `bin`, but a tool in an ordinary subfolder cannot borrow them.
            ("Program Files/Wrapped/bin/Wrapped.exe", 30 * MB), ("Program Files/Wrapped/steam_api64.dll", MB),
            ("Program Files/Suite/steam_api64.dll", MB), ("Program Files/Suite/Extras/worker.exe", 9 * MB),
        ]);
        assert_eq!(titles(&scan), ["Bundled", "CoolGame", "Hollow", "Tunic Win64 Shipping", "Wrapped"]);
        assert_eq!(scan.out.stats.programs, 4, "adb, Rainmeter, the office suite and the tool in a subfolder");
        assert!(filtered(&scan).is_empty(), "dropped programs are not offered for rescue");
        let _ = fs::remove_dir_all(temp);
    }
    #[test] fn a_scan_of_downloads_drops_installers_and_keeps_game_folders() {
        let temp = scratch();
        let root = temp.join("Downloads");
        let scan = walk(&temp, &root, &[
            ("Downloads/Rainmeter-4.5.28.exe", 5 * MB), ("Downloads/widget.exe", 5 * MB), ("Downloads/python 3.13.6 amd64.exe", 30 * MB),
            ("Downloads/steam_api64.dll", MB), // a stray dll beside loose files proves nothing
            ("Downloads/Cool Game/CoolGame.exe", 40 * MB), ("Downloads/Cool Game/steam_api64.dll", MB),
            ("Downloads/Plain Game/PlainGame.exe", 40 * MB),
            ("Downloads/Plain Game 2.0/Plain Game 2.0.exe", 40 * MB),
        ]);
        assert_eq!(titles(&scan), ["CoolGame"]);
        assert_eq!(filtered(&scan), ["Plain Game 2.0", "PlainGame"].map(String::from).to_vec(), "no proof and not obviously an installer: offered under Filtered out");
        let s = &scan.out.stats;
        assert_eq!((s.installers, s.no_game_files), (3, 2), "installers {}, no game files {}", s.installers, s.no_game_files);
        let _ = fs::remove_dir_all(temp);
    }
    #[test] fn appdata_programs_need_proof_but_games_folders_do_not() {
        let temp = scratch();
        let root = temp.join("Users/Sam");
        let scan = walk(&temp, &root, &[
            ("Users/Sam/AppData/Local/Aether Hub/Aether Hub.exe", 6 * MB), ("Users/Sam/AppData/Local/Programs/Libre/libre.exe", 9 * MB),
            ("Users/Sam/AppData/Local/Ubisoft/r6/R6.exe", 90 * MB), ("Users/Sam/AppData/Local/Ubisoft/r6/steam_api64.dll", MB),
            ("Users/Sam/.cargo/bin/cargo-thing.exe", 9 * MB),
            ("Users/Sam/Documents/Games/Hades/Hades.exe", 40 * MB),
        ]);
        assert!(!titles(&scan).iter().any(|t| t == "libre" || t.starts_with("cargo") || t.starts_with("Aether")), "{:?}", titles(&scan));
        assert!(titles(&scan).contains(&"R6".to_string()), "a game installed under AppData is kept when it ships game files");
        let _ = fs::remove_dir_all(temp);
    }
    #[test] fn a_games_folder_is_trusted_even_inside_program_files() {
        let temp = scratch();
        let root = temp.join("Program Files");
        let scan = walk(&temp, &root, &[("Program Files/Ubisoft/games/Far Cry/FarCry.exe", 50 * MB), ("Program Files/Ubisoft/Connect/UbisoftConnect.exe", 50 * MB)]);
        assert_eq!(titles(&scan), ["FarCry"]);
        let _ = fs::remove_dir_all(temp);
    }
    #[test] fn a_synthetic_scan_review_shaped_like_a_real_drive_keeps_only_the_games() {
        // (path, game files beside it, kept?). These are made-up names in the shapes a real drive scan produced:
        // tools under Program Files and AppData, installers and loose downloads, and games in folders that say so.
        let cases: &[(&str, &[&str], bool)] = &[
            ("C/Program Files/Git/usr/bin/bash.exe", &[], false), ("C/Program Files/Google/Chrome/Application/launcher.exe", &[], false),
            ("C/Program Files/Wireshark/Wireshark.exe", &[], false), ("C/Program Files/Blender/blender.exe", &["python311.dll"], false),
            ("C/Program Files/VLC/vlc.exe", &[], false), ("C/Program Files/Some Game/SomeGame.exe", &["steam_api.dll"], true),
            ("C/Users/Sam/AppData/Local/Programs/Tool/tool.exe", &[], false), ("C/Users/Sam/AppData/Local/Notes/notes.exe", &[], false),
            ("C/Users/Sam/Downloads/Cool-1.4.2-setup.exe", &[], false), ("C/Users/Sam/Downloads/gimp 2.10.exe", &[], false),
            ("C/Users/Sam/Downloads/portable.exe", &[], false), ("C/Users/Sam/Downloads/Cool Game/CoolGame.exe", &[], false),
            ("D/Games/Hades/Hades.exe", &[], true), ("D/Games/Odd Game/OddGame.exe", &[], true), ("D/Stuff/Odd Game 2.0/Odd Game 2.0.exe", &["fmod.dll"], true),
            ("D/Stuff/Cool Tool 3.1/Cool Tool 3.1.exe", &[], false),
        ];
        let wrong: Vec<_> = cases.iter().filter_map(|(path, beside, expected)| {
            let parts: Vec<&str> = path.split('/').collect();
            let file = parts[parts.len() - 1].trim_end_matches(".exe");
            let place = placement(&parts);
            let score = if needs_proof(place, file) { evidence(file, &listing(beside, &[])).score } else { 0 };
            (( place_verdict(place, file, score) == Verdict::Accept) != *expected).then_some(*path)
        }).collect();
        assert!(wrong.is_empty(), "misjudged: {wrong:?}");
    }
    #[test] fn known_limitation_an_unmarked_program_in_a_neutral_folder_is_still_offered() {
        // Outside Program Files, AppData, Downloads and a games folder there is nothing to judge by except the name, so a
        // tool the name rules do not recognise is offered. The review dialog is where the user rejects it.
        assert_eq!(place_verdict(placement(&["D", "Stuff", "Mod Tool", "modtool"]), "modtool", 0), Verdict::Accept);
    }
}
