use crate::models::Game;
use std::{path::Path, process::Command};

pub fn validate_target(target: &str) -> Result<(), String> {
    if target.contains('\0') { return Err("Launch target contains an invalid character".into()); }
    if let Some(id) = target.strip_prefix("steam://rungameid/") {
        if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) { return Ok(()); }
        return Err("Only numeric Steam game launch URIs are supported".into());
    }
    if target.starts_with("com.epicgames.launcher://apps/") {
        let u = url::Url::parse(target).map_err(|_| "Invalid Epic launch URI")?;
        if u.host_str() == Some("apps") && !u.path().trim_matches('/').is_empty() && u.username().is_empty() && u.fragment().is_none() && u.query_pairs().all(|(k,v)| (k == "action" && v == "launch") || (k == "silent" && v == "true")) { return Ok(()); }
        return Err("Unsupported Epic launch URI".into());
    }
    let p = Path::new(target);
    if !p.is_absolute() || !p.extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("exe")) { return Err("Choose an absolute .exe path or a supported Steam/Epic launch URI".into()); }
    Ok(())
}
pub fn handoff(game: &Game) -> Result<(), String> {
    validate_target(&game.target)?;
    if game.target.contains("://") {
        #[cfg(windows)] {
            use windows::{core::PCWSTR, Win32::{Foundation::HWND, UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL}}};
            let target: Vec<u16> = game.target.encode_utf16().chain(Some(0)).collect();
            let operation: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
            let result = unsafe { ShellExecuteW(HWND::default(), PCWSTR(operation.as_ptr()), PCWSTR(target.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
            if result.0 as isize <= 32 { return Err("Windows could not open the platform client. Check that the client is installed.".into()); }
            return Ok(());
        }
        #[cfg(not(windows))] return Err("Platform launching is supported on Windows only".into());
    }
    let exe = Path::new(&game.target);
    if !exe.is_file() { return Err("Game executable not found. Edit its path or reinstall the game.".into()); }
    let dir = if game.working_dir.is_empty() { exe.parent().ok_or("Executable folder unavailable")? } else { Path::new(&game.working_dir) };
    if !dir.is_dir() { return Err("Working directory does not exist".into()); }
    let mut cmd = Command::new(exe); cmd.current_dir(dir);
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
        if !game.args.is_empty() { cmd.raw_arg(&game.args); }
    }
    #[cfg(not(windows))] if !game.args.is_empty() { return Err("Native game launch arguments are supported on Windows only".into()); }
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|e| format!("Could not start the game: {e}"))?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn uri_allowlist() {
        assert!(validate_target("steam://rungameid/1245620").is_ok());
        assert!(validate_target("steam://install/1245620").is_err());
        assert!(validate_target("https://evil.example/run.exe").is_err());
        assert!(validate_target("com.epicgames.launcher://apps/abc?action=launch&silent=true").is_ok());
        assert!(validate_target("com.epicgames.launcher://apps/abc?action=uninstall").is_err());
    }
}
