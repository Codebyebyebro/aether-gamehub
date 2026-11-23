use crate::models::Game;
#[cfg(windows)]
use crate::storage;
#[cfg(windows)]
pub fn export() -> Result<Option<usize>, String> {
    use std::{fs, os::windows::ffi::OsStrExt};
    use windows::{core::{Interface, PCWSTR}, Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED}, Win32::UI::Shell::{IShellLinkW, ShellLink}};
    let library = storage::load()?;
    if library.games.is_empty() { return Err("Your library is empty".into()); }
    let Some(folder) = rfd::FileDialog::new().set_title("Create Aether Hub shortcuts in this folder").pick_folder() else { return Ok(None); };
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let wide = |s: &std::ffi::OsStr| -> Vec<u16> { s.encode_wide().chain(Some(0)).collect() };
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().map_err(|e| e.to_string())?; }
    struct Apartment; impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } } let _apartment = Apartment;
    let mut count = 0; let mut failures = Vec::new();
    for game in &library.games {
        let name = filename(game); let mut path = folder.join(format!("{name}.lnk"));
        for suffix in 2..10000 { if !path.exists() { break; } path = folder.join(format!("{name} ({suffix}).lnk")); }
        if path.exists() { failures.push(game.title.clone()); continue; }
        let result = (|| -> Result<(), String> {
            let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.map_err(|e| e.to_string())?;
            let target = wide(exe.as_os_str()); let args = wide(std::ffi::OsStr::new(&format!("--launch {}", game.id))); let desc = wide(std::ffi::OsStr::new(&game.title));
            unsafe {
                link.SetPath(PCWSTR(target.as_ptr())).map_err(|e| e.to_string())?;
                link.SetArguments(PCWSTR(args.as_ptr())).map_err(|e| e.to_string())?;
                link.SetDescription(PCWSTR(desc.as_ptr())).map_err(|e| e.to_string())?;
                let icon = if !game.target.contains("://") && std::path::Path::new(&game.target).is_file() { wide(std::ffi::OsStr::new(&game.target)) } else { target.clone() };
                link.SetIconLocation(PCWSTR(icon.as_ptr()), 0).map_err(|e| e.to_string())?;
                let directory = if !game.working_dir.is_empty() { std::path::PathBuf::from(&game.working_dir) } else { exe.parent().ok_or("Application folder unavailable")?.to_path_buf() };
                let wd = wide(directory.as_os_str()); link.SetWorkingDirectory(PCWSTR(wd.as_ptr())).map_err(|e| e.to_string())?;
                let file: IPersistFile = link.cast().map_err(|e| e.to_string())?;
                let destination = wide(path.as_os_str()); file.Save(PCWSTR(destination.as_ptr()), true).map_err(|e| e.to_string())?;
            }
            if !path.is_file() { return Err("Windows did not create the shortcut".into()); }
            Ok(())
        })();
        if result.is_ok() { count += 1; } else { let _ = fs::remove_file(&path); failures.push(game.title.clone()); }
    }
    if !failures.is_empty() { return Err(format!("Created {count} shortcuts. Could not create: {}. Existing shortcuts were not changed.", failures.join(", "))); }
    Ok(Some(count))
}
#[cfg(not(windows))]
pub fn export() -> Result<Option<usize>, String> { Err("Windows shortcut export is available on Windows only".into()) }
fn filename(game: &Game) -> String {
    let name: String = game.title.chars().filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c)).take(100).collect();
    let trimmed = name.trim().trim_end_matches([' ', '.']);
    let stem = trimmed.split('.').next().unwrap_or("").to_uppercase();
    if trimmed.is_empty() || ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"].contains(&stem.as_str()) { format!("Aether Game {}", &game.id[..8]) } else { trimmed.into() }
}
#[cfg(test)] mod tests { use super::*; #[test] fn safe_names() { let g = Game::new("CON".into(), "Custom", "".into()); assert!(filename(&g).starts_with("Aether Game")); let g = Game::new("A: / Game?".into(), "Custom", "".into()); assert!(!filename(&g).contains(':')); } }
