//! Start with Windows: a single value under the current user's `Run` key. Nothing machine-wide, no service, no
//! scheduled task, and switching it off deletes exactly that one value.
use std::path::Path;

/// The name of the value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
pub const VALUE: &str = "AetherHub";
/// Passed at sign-in so the launcher can start quietly in the tray (it only does so when the tray is on).
pub const ARG: &str = "--tray";

/// What Windows runs at sign-in: the executable in quotes (the path may hold spaces) and the start-quietly flag.
pub fn command_line(exe: &Path) -> String { format!("\"{}\" {ARG}", exe.display()) }

#[cfg(windows)]
mod registry {
    use super::command_line;
    use std::{io::ErrorKind, path::Path};
    use winreg::{enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE}, RegKey};
    const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

    pub fn set(name: &str, enabled: bool, exe: &Path) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(RUN, KEY_READ | KEY_WRITE)
            .map_err(|e| format!("Windows would not open the list of programs that start with it: {e}"))?;
        if enabled {
            key.set_value(name, &command_line(exe)).map_err(|e| format!("Could not add Aether Hub to the programs that start with Windows: {e}"))
        } else {
            match key.delete_value(name) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(()), // already off
                Err(e) => Err(format!("Could not remove Aether Hub from the programs that start with Windows: {e}")),
            }
        }
    }
    #[cfg(test)]
    pub fn get(name: &str) -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey(RUN).ok()?.get_value::<String, _>(name).ok()
    }
}

/// Adds or removes the sign-in entry for the running executable.
#[cfg(windows)]
pub fn apply(enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Could not find the Aether Hub program file: {e}"))?;
    registry::set(VALUE, enabled, &exe)
}
#[cfg(not(windows))]
pub fn apply(enabled: bool) -> Result<(), String> { if enabled { Err("Starting with Windows needs Windows".into()) } else { Ok(()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test] fn the_command_quotes_the_path_and_asks_for_a_quiet_start() {
        let line = command_line(&PathBuf::from("C:\\Users\\Some One\\AppData\\Local\\Aether Hub\\Aether Hub.exe"));
        assert_eq!(line, "\"C:\\Users\\Some One\\AppData\\Local\\Aether Hub\\Aether Hub.exe\" --tray");
    }
    #[test] fn only_the_one_flag_is_added() {
        let line = command_line(&PathBuf::from("C:\\x\\a.exe"));
        assert_eq!(line.matches("--").count(), 1, "{line}");
    }

    #[cfg(windows)]
    mod on_windows {
        use super::super::{command_line, registry};
        use std::path::PathBuf;
        // Value names of their own, so this never touches a real "AetherHub" entry on a developer's PC. Every test has
        // names no other test uses: tests run in parallel, and sharing one name let a test's cleanup remove the value
        // another test had just written (that failed a CI run once).
        fn exe() -> PathBuf { PathBuf::from("C:\\Program Files\\Aether Test\\Aether Hub.exe") }

        #[test] fn the_entry_is_written_read_back_and_removed() {
            const TEST_VALUE: &str = "AetherHubTestWriteReadRemove";
            registry::set(TEST_VALUE, false, &exe()).unwrap(); // start clean
            assert_eq!(registry::get(TEST_VALUE), None);
            registry::set(TEST_VALUE, true, &exe()).unwrap();
            assert_eq!(registry::get(TEST_VALUE), Some(command_line(&exe())), "exactly the command Windows will run");
            registry::set(TEST_VALUE, true, &PathBuf::from("C:\\Other\\Aether Hub.exe")).unwrap();
            assert_eq!(registry::get(TEST_VALUE), Some(command_line(&PathBuf::from("C:\\Other\\Aether Hub.exe"))), "writing again replaces it, a moved program is corrected");
            registry::set(TEST_VALUE, false, &exe()).unwrap();
            assert_eq!(registry::get(TEST_VALUE), None);
        }
        #[test] fn switching_off_when_there_is_no_entry_is_not_an_error() {
            const TEST_VALUE: &str = "AetherHubTestNeverAdded";
            registry::set(TEST_VALUE, false, &exe()).unwrap();
            registry::set(TEST_VALUE, false, &exe()).unwrap();
        }
        #[test] fn only_its_own_value_is_removed() {
            const TEST_VALUE: &str = "AetherHubTestOwnValue";
            const OTHER: &str = "AetherHubTestNeighbour";
            registry::set(OTHER, true, &exe()).unwrap(); registry::set(TEST_VALUE, true, &exe()).unwrap();
            registry::set(TEST_VALUE, false, &exe()).unwrap();
            assert_eq!(registry::get(OTHER), Some(command_line(&exe())), "a neighbouring entry is untouched");
            registry::set(OTHER, false, &exe()).unwrap();
        }
    }
}
