//! Facts about this PC for the diagnostics report: the Windows version, and how much memory Aether Hub is using.
use crate::diag::{self, Footprint, Proc};

/// For example `Windows 11 23H2 (build 22631.4460)`.
#[cfg(windows)]
pub fn windows() -> String {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion") else { return "unknown".into(); };
    let text = |name: &str| key.get_value::<String, _>(name).unwrap_or_default();
    let build = text("CurrentBuild");
    let revision = key.get_value::<u32, _>("UBR").map(|u| format!(".{u}")).unwrap_or_default();
    // The registry's product name still says "Windows 10" on Windows 11, so the build number decides which it is.
    let name = match build.parse::<u32>() { Ok(b) if b >= 22_000 => "Windows 11", Ok(_) => "Windows 10", Err(_) => "Windows" };
    format!("{name} {} (build {build}{revision})", text("DisplayVersion")).replace("  ", " ")
}
#[cfg(not(windows))]
pub fn windows() -> String { "not Windows".into() }

/// Memory held by this program and by the processes below it (the web view's helpers), read once, right now.
pub fn footprint() -> Option<Footprint> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut system = System::new().ok()?;
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_memory());
    let procs: Vec<Proc> = system.processes().iter().map(|(pid, p)| Proc { pid: pid.as_u32(), parent: p.parent().map(|parent| parent.as_u32()), bytes: p.memory() }).collect();
    Some(diag::footprint(&procs, std::process::id()))
}
