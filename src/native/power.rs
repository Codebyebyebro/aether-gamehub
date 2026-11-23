//! The console power menu: sleep, restart or shut down the PC from the controller.
//!
//! Only these three fixed actions exist; the interface names one and nothing it sends reaches a command line. Restart
//! and shut down use Windows' own `shutdown.exe` (no special rights needed, other programs are asked to close as
//! usual); sleep asks Windows directly. Running game sessions are saved first by the caller.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action { Sleep, Restart, ShutDown }

/// The action the interface asked for, or why it is not one.
pub fn parse(name: &str) -> Result<Action, String> {
    match name { "sleep" => Ok(Action::Sleep), "restart" => Ok(Action::Restart), "shutdown" => Ok(Action::ShutDown), _ => Err("Unknown power action".into()) }
}
/// The `shutdown.exe` arguments for restart and shut down; sleep does not use it.
pub fn shutdown_args(action: Action) -> Option<[&'static str; 4]> {
    match action { Action::Restart => Some(["/r", "/t", "0", "/d"]), Action::ShutDown => Some(["/s", "/t", "0", "/d"]), Action::Sleep => None }
}
/// The reason code passed with `/d`: "planned, other". It only labels the event in Windows' own log.
pub const REASON: &str = "p:0:0";

#[cfg(windows)]
pub fn perform(action: Action) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if let Some(args) = shutdown_args(action) {
        let system = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
        let status = std::process::Command::new(system.join("System32").join("shutdown.exe")).args(args).arg(REASON).creation_flags(CREATE_NO_WINDOW).status().map_err(|e| format!("Windows could not start the shutdown: {e}"))?;
        return if status.success() { Ok(()) } else { Err("Windows refused to restart or shut down right now. Another program may be blocking it.".into()) };
    }
    // Sleep. On PCs that only support "Modern Standby" Windows does not let programs ask for sleep, so say so plainly.
    let ok = unsafe { windows::Win32::System::Power::SetSuspendState(false, false, false) };
    if ok.as_bool() { Ok(()) } else { Err("This PC doesn't let apps put it to sleep. Use the power button or the Start menu instead.".into()) }
}
#[cfg(not(windows))]
pub fn perform(_: Action) -> Result<(), String> { Err("Sleep, restart and shut down from Aether Hub need Windows".into()) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_the_three_named_actions_exist() {
        assert_eq!(parse("sleep"), Ok(Action::Sleep));
        assert_eq!(parse("restart"), Ok(Action::Restart));
        assert_eq!(parse("shutdown"), Ok(Action::ShutDown));
        for bad in ["", "Shutdown", "shutdown /s", "logoff", "hibernate", "restart;calc"] { assert!(parse(bad).is_err(), "{bad}"); }
    }
    #[test] fn restart_and_shut_down_are_immediate_and_labelled_planned() {
        assert_eq!(shutdown_args(Action::Restart), Some(["/r", "/t", "0", "/d"]));
        assert_eq!(shutdown_args(Action::ShutDown), Some(["/s", "/t", "0", "/d"]));
        assert_eq!(shutdown_args(Action::Sleep), None, "sleep never runs shutdown.exe");
        assert_eq!(REASON, "p:0:0");
    }
}
