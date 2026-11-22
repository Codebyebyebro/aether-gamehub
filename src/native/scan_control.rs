//! Stopping and reporting on a library scan. The interface picks an id for each scan, listens for progress events
//! carrying that id, and can cancel it. Std-only, so it is unit-tested without Tauri or a disk.
use std::{sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, time::{Duration, Instant}};

/// How many walked entries pass between looks at the clock, so progress costs nothing measurable on a big drive.
const CLOCK_EVERY: u64 = 64;
/// Cancels that never matched a scan are remembered up to this many; a registered scan is never forgotten.
const REMEMBERED: usize = 8;

struct Entry { id: String, cancel: Arc<AtomicBool>, registered: bool }
/// Cancel flags by scan id. Cancelling and scanning are separate calls from the interface, so a cancel can arrive
/// before its scan does; an unknown id is therefore remembered as already cancelled. Ids are never reused.
pub struct Registry { scans: Mutex<Vec<Entry>> }
impl Registry {
    pub const fn new() -> Self { Self { scans: Mutex::new(Vec::new()) } }
    fn flag(&self, id: &str, registering: bool) -> Arc<AtomicBool> {
        let mut scans = self.scans.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = scans.iter_mut().find(|e| e.id == id) { entry.registered |= registering; return entry.cancel.clone(); }
        if scans.len() >= REMEMBERED { if let Some(stale) = scans.iter().position(|e| !e.registered) { scans.remove(stale); } }
        let cancel = Arc::new(AtomicBool::new(false));
        scans.push(Entry { id: id.to_owned(), cancel: cancel.clone(), registered: registering });
        cancel
    }
    /// Registers a scan and returns the flag it must check (already set when its cancel arrived first).
    pub fn begin(&self, id: &str) -> Arc<AtomicBool> { self.flag(id, true) }
    pub fn cancel(&self, id: &str) { self.flag(id, false).store(true, Ordering::SeqCst); }
    pub fn finish(&self, id: &str) { self.scans.lock().unwrap_or_else(|e| e.into_inner()).retain(|e| e.id != id); }
    #[cfg(test)] fn len(&self) -> usize { self.scans.lock().unwrap().len() }
}

/// What a scan has done so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    /// 0 while the store install lists are read, then 1..=folders for each custom folder, and `steps` when done.
    pub step: u32,
    pub steps: u32,
    /// Files and folders looked at inside custom folders.
    pub checked: u64,
    /// Candidates found so far (before duplicates are removed).
    pub found: u32,
    /// The custom folder being searched, as a clean path; empty while reading store lists.
    pub folder: String,
}

/// Handed to the scanner: tells it when to stop, and passes progress on at most once per `every` (step changes
/// are always passed on).
pub struct Watch<'a> {
    cancel: Arc<AtomicBool>,
    report: Box<dyn FnMut(&Progress) + 'a>,
    every: Duration,
    last: Option<Instant>,
    progress: Progress,
}
impl<'a> Watch<'a> {
    pub fn new(cancel: Arc<AtomicBool>, every: Duration, report: impl FnMut(&Progress) + 'a) -> Self {
        Self { cancel, report: Box::new(report), every, last: None, progress: Progress::default() }
    }
    /// Nobody listens and nobody cancels.
    #[cfg(test)]
    pub fn detached() -> Watch<'static> { Watch::new(Arc::new(AtomicBool::new(false)), Duration::MAX, |_| {}) }
    pub fn cancelled(&self) -> bool { self.cancel.load(Ordering::Relaxed) }
    pub fn step(&mut self, step: u32, steps: u32, folder: String, found: usize) {
        self.progress.step = step; self.progress.steps = steps; self.progress.folder = folder;
        self.progress.found = found.try_into().unwrap_or(u32::MAX);
        self.send();
    }
    pub fn finish(&mut self, found: usize) { let steps = self.progress.steps; self.step(steps, steps, String::new(), found); }
    /// Counts one walked entry. Returns `false` once the scan should stop.
    pub fn tick(&mut self, found: usize) -> bool {
        self.progress.checked += 1;
        if self.progress.checked % CLOCK_EVERY == 0 && self.last.is_none_or(|last| last.elapsed() >= self.every) {
            self.progress.found = found.try_into().unwrap_or(u32::MAX);
            self.send();
        }
        !self.cancelled()
    }
    fn send(&mut self) { self.last = Some(Instant::now()); (self.report)(&self.progress); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_cancel_reaches_only_its_own_scan() {
        let scans = Registry::new();
        let (a, b) = (scans.begin("a"), scans.begin("b"));
        scans.cancel("a");
        assert!(a.load(Ordering::SeqCst) && !b.load(Ordering::SeqCst));
    }
    #[test] fn a_cancel_that_arrives_before_its_scan_still_counts() {
        let scans = Registry::new();
        scans.cancel("early");
        assert!(scans.begin("early").load(Ordering::SeqCst));
    }
    #[test] fn finished_scans_are_forgotten_and_stray_cancels_are_bounded() {
        let scans = Registry::new();
        let running = scans.begin("running");
        for i in 0..50 { scans.cancel(&format!("stray-{i}")); }
        assert!(scans.len() <= REMEMBERED + 1, "{}", scans.len());
        scans.cancel("running");
        assert!(running.load(Ordering::SeqCst), "a registered scan is never evicted by stray cancels");
        scans.finish("running");
        assert!(!scans.begin("fresh").load(Ordering::SeqCst));
    }
    #[test] fn progress_is_throttled_but_step_changes_always_report() {
        let mut seen = vec![];
        let mut watch = Watch::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(3600), |p| seen.push(p.clone()));
        watch.step(1, 3, "D:\\Games".into(), 2);
        for _ in 0..(CLOCK_EVERY * 10) { assert!(watch.tick(5)); }
        watch.step(2, 3, "E:\\More".into(), 5);
        watch.finish(7);
        drop(watch);
        let steps: Vec<_> = seen.iter().map(|p| (p.step, p.folder.as_str())).collect();
        assert_eq!(steps, [(1, "D:\\Games"), (2, "E:\\More"), (3, "")], "the hour-long throttle swallowed every tick");
        assert_eq!((seen[2].checked, seen[2].found, seen[2].steps), (CLOCK_EVERY * 10, 7, 3));
    }
    #[test] fn ticks_report_when_the_throttle_allows() {
        let mut seen = 0;
        let mut watch = Watch::new(Arc::new(AtomicBool::new(false)), Duration::ZERO, |_| seen += 1);
        for _ in 0..(CLOCK_EVERY * 3) { watch.tick(0); }
        drop(watch);
        assert_eq!(seen, 3);
    }
    #[test] fn tick_says_stop_once_cancelled() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut watch = Watch::new(cancel.clone(), Duration::MAX, |_| {});
        assert!(watch.tick(0) && !watch.cancelled());
        cancel.store(true, Ordering::SeqCst);
        assert!(!watch.tick(0) && watch.cancelled());
    }
}
