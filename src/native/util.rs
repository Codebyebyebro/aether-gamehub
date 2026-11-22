//! Small helpers shared by the native modules.
use std::{path::Path, time::{SystemTime, UNIX_EPOCH}};

/// Seconds since the Unix epoch (0 if the clock is before 1970).
pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// `20261118-041500` (UTC). File names use UTC so they sort chronologically on every machine.
pub fn utc_stamp(secs: u64) -> String {
    // Howard Hinnant's civil-from-days algorithm.
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3_600, (rem % 3_600) / 60, rem % 60)
}

/// Windows' canonical paths carry a `\\?\` prefix that is noise to people and to duplicate checks.
pub fn clean_path(path: &Path) -> String { strip_verbatim(&path.to_string_lossy()) }

pub fn strip_verbatim(value: &str) -> String {
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") { format!(r"\\{rest}") }
    else if let Some(rest) = value.strip_prefix(r"\\?\") { rest.to_owned() }
    else { value.to_owned() }
}

fn normalized(path: &Path) -> String { clean_path(path).replace('\\', "/").trim_end_matches('/').to_lowercase() }

/// Case-insensitive, separator-insensitive "is `path` the same as or inside `prefix`".
pub fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    let (path, prefix) = (normalized(path), normalized(prefix));
    !prefix.is_empty() && (path == prefix || path.starts_with(&format!("{prefix}/")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn stamps_are_correct_and_sortable() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        assert_eq!(utc_stamp(951_782_400), "20000229-000000"); // leap day
        assert_eq!(utc_stamp(1_731_902_100), "20241118-035500");
        assert!(utc_stamp(1_731_902_100) < utc_stamp(1_731_902_101));
    }
    #[test] fn verbatim_prefixes_are_removed() {
        assert_eq!(strip_verbatim(r"\\?\C:\Games\Hades\Hades.exe"), r"C:\Games\Hades\Hades.exe");
        assert_eq!(strip_verbatim(r"\\?\UNC\nas\games\x.exe"), r"\\nas\games\x.exe");
        assert_eq!(strip_verbatim(r"D:\Games\x.exe"), r"D:\Games\x.exe");
    }
    #[test] fn prefix_checks_ignore_case_and_separators() {
        assert!(path_starts_with(Path::new(r"C:\Games\Hades\Hades.exe"), Path::new("c:/games/hades")));
        assert!(path_starts_with(Path::new(r"\\?\C:\Games\Hades"), Path::new(r"C:\Games\Hades\")));
        assert!(!path_starts_with(Path::new(r"C:\Games\HadesTwo\x.exe"), Path::new(r"C:\Games\Hades")));
        assert!(!path_starts_with(Path::new(r"C:\x"), Path::new("")));
    }
}
