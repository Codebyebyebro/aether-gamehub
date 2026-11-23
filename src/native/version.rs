//! Release numbers, for the "a newer version is available" check. Kept free of the network so it can be tested anywhere.

/// `v1.2.3`, `1.2.3` and `1.2` (patch 0) are understood. Anything after a `-` or `+` (a pre-release or build label) is
/// ignored, so a label can never make a release look newer. Anything else is not a version.
pub fn parse(tag: &str) -> Option<[u64; 3]> {
    let core = tag.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    let number = |part: &str| (!part.is_empty() && part.len() <= 9 && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.parse::<u64>().ok()).flatten();
    let mut parts = core.split('.');
    let major = number(parts.next()?)?;
    let minor = match parts.next() { Some(p) => number(p)?, None => 0 };
    let patch = match parts.next() { Some(p) => number(p)?, None => 0 };
    if parts.next().is_some() { return None; }
    Some([major, minor, patch])
}

/// The version as it is shown to the player: always three numbers, never the raw tag text from the network.
pub fn show(version: [u64; 3]) -> String { format!("{}.{}.{}", version[0], version[1], version[2]) }

/// True only when both are real versions and `latest` is strictly higher. A tag that cannot be read is never an update.
pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse(current), parse(latest)) { (Some(current), Some(latest)) => latest > current, _ => false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn tags_with_and_without_a_leading_v_are_read() {
        assert_eq!(parse("v1.2.3"), Some([1, 2, 3]));
        assert_eq!(parse("1.2.3"), Some([1, 2, 3]));
        assert_eq!(parse(" V0.10.0 "), Some([0, 10, 0]));
        assert_eq!(parse("v2.1"), Some([2, 1, 0]));
        assert_eq!(parse("3"), Some([3, 0, 0]));
    }
    #[test] fn labels_after_the_numbers_are_ignored() {
        assert_eq!(parse("v1.2.3-beta.1"), Some([1, 2, 3]));
        assert_eq!(parse("1.2.3+build.5"), Some([1, 2, 3]));
    }
    #[test] fn things_that_are_not_versions_are_rejected() {
        for bad in ["", "v", "latest", "1.2.3.4", "1..3", "a.b.c", "1.2.x", "v-1.0.0", "1.2.3 beta", "99999999999.0.0", "../../etc"] { assert_eq!(parse(bad), None, "{bad:?}"); }
    }
    #[test] fn numbers_are_compared_as_numbers_not_as_text() {
        assert!(is_newer("0.9.0", "0.10.0"), "10 is more than 9");
        assert!(is_newer("1.9.9", "2.0.0"));
        assert!(is_newer("0.1.0", "v0.1.1"));
        assert!(!is_newer("0.10.0", "0.9.0"));
    }
    #[test] fn the_same_version_or_an_older_one_is_not_an_update() {
        assert!(!is_newer("0.1.0", "v0.1.0"));
        assert!(!is_newer("1.2.3", "1.2.3-rc1"), "a label does not make a release newer");
        assert!(!is_newer("2.0.0", "1.9.9"));
    }
    #[test] fn an_unreadable_tag_is_never_an_update() {
        assert!(!is_newer("0.1.0", "nightly") && !is_newer("0.1.0", "") && !is_newer("junk", "1.0.0"));
    }
    #[test] fn the_player_sees_clean_numbers() {
        assert_eq!(show(parse("v1.2").unwrap()), "1.2.0");
        assert_eq!(show(parse("V3.4.5-beta").unwrap()), "3.4.5");
    }
}
