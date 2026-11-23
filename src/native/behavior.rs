//! The decisions behind the tray, the global shortcut, starting with Windows and waking on a controller, kept free of
//! Windows and Tauri so they can be tested anywhere.
use std::time::Duration;

/// The four background behaviours the player can switch on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Wanted { pub tray: bool, pub shortcut: bool, pub autostart: bool, pub pad: bool }

/// What has to change to get from what was applied last time to what is wanted now. `None` means leave it alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Plan { pub tray: Option<bool>, pub shortcut: Option<bool>, pub autostart: Option<bool>, pub pad: Option<bool> }

/// Every settings save comes through here, including slider drags, so it must be cheap and must not touch Windows
/// (registry, hotkeys, tray) for a change that is not about one of these four.
///
/// The first call after start-up applies what is switched on. Start with Windows is also reconciled in both
/// directions then, so a moved or reinstalled executable gets its entry rewritten and a switched-off one is removed.
pub fn plan(applied: Option<Wanted>, next: Wanted) -> Plan {
    let change = |before: Option<bool>, now: bool| match before { Some(was) if was == now => None, None if !now => None, _ => Some(now) };
    Plan {
        tray: change(applied.map(|w| w.tray), next.tray),
        shortcut: change(applied.map(|w| w.shortcut), next.shortcut),
        autostart: match applied { Some(w) if w.autostart == next.autostart => None, _ => Some(next.autostart) },
        pad: change(applied.map(|w| w.pad), next.pad),
    }
}

/// Windows lists every controller that is already connected as "added" when the watcher starts, so arrivals in the
/// first moments are the starting picture, not news. A Bluetooth pad that drops and reconnects must not pull the
/// window forward over and over, so a wake-up is followed by a quiet period.
pub const WARMUP: Duration = Duration::from_secs(4);
pub const COOLDOWN: Duration = Duration::from_secs(10);

pub struct Arrivals { began: Duration, last: Option<Duration>, warmup: Duration, cooldown: Duration }
impl Arrivals {
    /// `now` is any monotonic clock reading, for example the time since the program started.
    pub fn new(now: Duration) -> Self { Self::with(now, WARMUP, COOLDOWN) }
    pub fn with(now: Duration, warmup: Duration, cooldown: Duration) -> Self { Arrivals { began: now, last: None, warmup, cooldown } }
    /// A controller was reported as added at `now`: should the launcher come forward?
    pub fn arrived(&mut self, now: Duration) -> bool {
        if now < self.began + self.warmup { return false; }
        if self.last.is_some_and(|last| now < last + self.cooldown) { return false; }
        self.last = Some(now);
        true
    }
}

/// Console mode, "back home after playing": whether a game that just ended should bring the launcher back in Big Box.
/// Only with the setting on, no other game still running, and a controller connected. The controller is checked last,
/// because asking Windows can take a moment.
pub fn return_home(console_mode: bool, others_running: bool, pad_connected: impl FnOnce() -> bool) -> bool {
    console_mode && !others_running && pad_connected()
}

#[cfg(test)]
mod tests {
    use super::*;
    const OFF: Wanted = Wanted { tray: false, shortcut: false, autostart: false, pad: false };
    const fn s(secs: u64) -> Duration { Duration::from_secs(secs) }

    #[test] fn a_fresh_start_with_everything_off_touches_nothing_except_to_clear_a_stale_startup_entry() {
        assert_eq!(plan(None, OFF), Plan { tray: None, shortcut: None, autostart: Some(false), pad: None });
    }
    #[test] fn a_fresh_start_applies_what_is_switched_on() {
        let on = Wanted { tray: true, shortcut: true, autostart: true, pad: true };
        assert_eq!(plan(None, on), Plan { tray: Some(true), shortcut: Some(true), autostart: Some(true), pad: Some(true) });
    }
    #[test] fn a_save_that_changes_none_of_the_four_changes_nothing() {
        let on = Wanted { tray: true, shortcut: false, autostart: true, pad: false };
        assert_eq!(plan(Some(on), on), Plan::default());
        assert_eq!(plan(Some(OFF), OFF), Plan::default());
    }
    #[test] fn only_the_switch_that_moved_is_acted_on() {
        let before = Wanted { tray: true, shortcut: false, autostart: false, pad: false };
        assert_eq!(plan(Some(before), Wanted { shortcut: true, ..before }), Plan { shortcut: Some(true), ..Plan::default() });
        assert_eq!(plan(Some(before), Wanted { tray: false, ..before }), Plan { tray: Some(false), ..Plan::default() });
        assert_eq!(plan(Some(before), Wanted { autostart: true, ..before }), Plan { autostart: Some(true), ..Plan::default() });
    }
    #[test] fn switching_a_behaviour_off_after_it_was_on_is_acted_on() {
        let before = Wanted { tray: true, shortcut: true, autostart: true, pad: true };
        assert_eq!(plan(Some(before), OFF), Plan { tray: Some(false), shortcut: Some(false), autostart: Some(false), pad: Some(false) });
    }

    #[test] fn controllers_reported_while_the_watcher_starts_up_are_the_starting_picture() {
        let mut a = Arrivals::with(s(100), s(4), s(10));
        assert!(!a.arrived(s(100)) && !a.arrived(s(103)), "already-connected pads are listed as added at once");
    }
    #[test] fn a_controller_that_connects_after_the_warm_up_wakes_the_launcher() {
        let mut a = Arrivals::with(s(100), s(4), s(10));
        assert!(a.arrived(s(104)));
    }
    #[test] fn a_pad_that_flaps_wakes_the_launcher_once_per_quiet_period() {
        let mut a = Arrivals::with(s(0), s(4), s(10));
        assert!(a.arrived(s(20)));
        assert!(!a.arrived(s(21)) && !a.arrived(s(29)), "reconnects inside the quiet period are ignored");
        assert!(a.arrived(s(30)), "and it works again once the quiet period is over");
    }
    #[test] fn an_ignored_arrival_does_not_extend_the_quiet_period() {
        let mut a = Arrivals::with(s(0), s(4), s(10));
        assert!(a.arrived(s(20)));
        assert!(!a.arrived(s(29)));
        assert!(a.arrived(s(30)), "measured from the wake-up, not from the last ignored arrival");
    }
    #[test] fn a_warm_up_arrival_does_not_start_the_quiet_period() {
        let mut a = Arrivals::with(s(0), s(4), s(10));
        assert!(!a.arrived(s(1)));
        assert!(a.arrived(s(5)), "the first real arrival is not held back by one that came during the warm up");
    }
    #[test] fn the_real_timings_are_a_few_seconds_to_settle_and_a_quiet_period_longer_than_that() {
        assert!(WARMUP >= s(2) && WARMUP <= s(10) && COOLDOWN > WARMUP);
    }
    #[test] fn back_home_only_with_the_setting_a_controller_and_nothing_else_running() {
        assert!(super::return_home(true, false, || true));
        assert!(!super::return_home(false, false, || true), "setting off");
        assert!(!super::return_home(true, true, || true), "another game is still running");
        assert!(!super::return_home(true, false, || false), "no controller: the desktop experience is unchanged");
        assert!(!super::return_home(false, false, || panic!("Windows is not asked when the setting is off")));
        assert!(!super::return_home(true, true, || panic!("Windows is not asked while another game runs")));
    }
}
