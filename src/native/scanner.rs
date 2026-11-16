use crate::models::{Game, ScanResult};
use serde_json::{Map, Value};
use std::{collections::HashSet, fs, path::{Path, PathBuf}};

pub fn scan(folders: Vec<String>) -> Result<ScanResult, String> {
    if folders.len() > 100 { return Err("Choose no more than 100 custom folders".into()); }
    let mut result = ScanResult { games: vec![], warnings: vec![] };
    steam(&mut result); epic(&mut result); riot(&mut result);
    for folder in folders { custom(Path::new(&folder), &mut result); }
    let mut seen = HashSet::new();
    result.games.retain(|g| seen.insert(format!("{}|{}", g.target.replace('/', "\\").to_lowercase(), g.args.trim())));
    result.games.sort_by_key(|g| g.title.to_lowercase());
    Ok(result)
}
fn read_json(path: &Path) -> Result<Value, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 { return Err("Manifest is unexpectedly large".into()); }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn program_data() -> PathBuf { std::env::var_os("PROGRAMDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\ProgramData")) }
fn steam_roots() -> Vec<PathBuf> {
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
fn steam(out: &mut ScanResult) {
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
            Err(e) => out.warnings.push(format!("Steam library list could not be read: {e}")),
        }
    }
    let mut seen = HashSet::new();
    for library in libraries {
        if !seen.insert(library.to_string_lossy().to_lowercase()) { continue; }
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
                    if !apps.join("common").join(dir).is_dir() { continue; }
                    let mut game = Game::new(title.into(), "Steam", format!("steam://rungameid/{id}")); game.steam_art(id); out.games.push(game);
                }
                Err(_) => out.warnings.push(format!("Skipped unreadable Steam manifest: {name}")),
            }
        }
    }
}
fn epic(out: &mut ScanResult) {
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
                let encoded: String = url::form_urlencoded::byte_serialize(app.as_bytes()).collect();
                let target = format!("com.epicgames.launcher://apps/{encoded}?action=launch&silent=true");
                out.games.push(Game::new(title.into(), "Epic", target));
            }
            Err(_) => out.warnings.push(format!("Skipped unreadable Epic manifest: {}", entry.file_name().to_string_lossy())),
        }
    }
}
fn riot(out: &mut ScanResult) {
    let base = program_data().join("Riot Games"); let installs = base.join("RiotClientInstalls.json");
    if !installs.exists() { return; }
    let data = match read_json(&installs) { Ok(v) => v, Err(e) => { out.warnings.push(format!("Riot installation list could not be read: {e}")); return; } };
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
        out.games.push(game); found = true;
    }
    if !found { out.warnings.push("Riot Client was detected, but no supported installed product could be identified. Add its game manually with the client’s launch arguments.".into()); }
}
fn junk(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["unins", "crashpad", "crashreport", "crash_report", "setup", "redist", "vcredist", "dxsetup", "uninstall", "installer", "update", "cefsubprocess", "helper", "unitycrash", "ue4prereq"].iter().any(|s| name.starts_with(s))
}
fn custom(folder: &Path, out: &mut ScanResult) {
    if !folder.is_absolute() || !folder.is_dir() { out.warnings.push(format!("Custom scan folder is unavailable: {}", folder.display())); return; }
    let walker = walkdir::WalkDir::new(folder).max_depth(6).follow_links(false).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        e.depth() == 0 || (!junk(&name) && !["windows", "$recycle.bin", "system volume information", "node_modules", "_commonredist"].contains(&name.as_str()))
    });
    let mut denied = 0;
    for entry in walker {
        let entry = match entry { Ok(e) => e, Err(_) => { denied += 1; continue; } };
        if out.games.len() >= 20000 { out.warnings.push("The scan reached the 20,000-candidate safety limit".into()); break; }
        if !entry.file_type().is_file() { continue; }
        let path = entry.path(); if !path.extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("exe")) { continue; }
        let name = path.file_stem().unwrap_or_default().to_string_lossy(); if junk(&name) { continue; }
        let Ok(path) = path.canonicalize() else { continue; };
        let title = name.replace(['_', '-'], " ");
        let mut game = Game::new(title, "Custom", path.to_string_lossy().into_owned());
        game.working_dir = path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(); out.games.push(game);
    }
    if denied > 0 { out.warnings.push(format!("Skipped {denied} inaccessible entries in {}", folder.display())); }
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
    use super::*;
    #[test] fn modern_libraries() { let v = parse_vdf(r#""libraryfolders" { "0" { "path" "C:\\Steam" } "1" { "path" "D:\\Games" } }"#).unwrap(); assert_eq!(v["libraryfolders"]["1"]["path"], "D:\\Games"); }
    #[test] fn legacy_libraries() { let v = parse_vdf(r#""LibraryFolders" { "1" "D:\\Steam" }"#).unwrap(); assert_eq!(v["LibraryFolders"]["1"], "D:\\Steam"); }
    #[test] fn rejects_broken() { assert!(parse_vdf("\"AppState\" {").is_err()); assert!(parse_vdf("\"unclosed").is_err()); }
    #[test] fn filters_helpers() { for name in ["unins000.exe", "crashpad_handler.exe", "setup.exe", "vcredist_x64.exe"] { assert!(junk(name)); } assert!(!junk("hollow_knight.exe")); }
}
