//! Notices when a game controller connects while Aether Hub is hidden or minimized, so it can come forward in Big Box.
//!
//! A hidden window cannot read the controller (the browser only exposes it to a visible, focused page), so this is done
//! natively: Windows tells the program when a controller is added, nothing is polled, and no button is ever read here.
//! It only learns that a controller appeared.

#[cfg(windows)]
mod imp {
    use crate::behavior::Arrivals;
    use std::{sync::{atomic::{AtomicBool, Ordering}, mpsc, Mutex}, time::{Duration, Instant}};
    use windows::{
        Foundation::EventHandler,
        Gaming::Input::RawGameController,
        Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
    };
    static RUNNING: AtomicBool = AtomicBool::new(false);

    /// Starts listening (once). `on_arrival` runs on a Windows worker thread each time the launcher should come forward.
    pub fn start(on_arrival: impl Fn() + Send + Sync + 'static) -> Result<(), String> {
        if RUNNING.swap(true, Ordering::SeqCst) { return Ok(()); }
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        let spawned = std::thread::Builder::new().name("controller-watch".into()).spawn(move || {
            // Windows' game-input events are free-threaded: this thread joins the multithreaded apartment and stays alive.
            let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
            let begun = Instant::now();
            let arrivals = Mutex::new(Arrivals::new(Duration::ZERO));
            // Controllers that are already connected are reported as added right away; `Arrivals` treats those as the starting picture.
            let handler = EventHandler::<RawGameController>::new(move |_, _| {
                let now = begun.elapsed();
                if arrivals.lock().map(|mut a| a.arrived(now)).unwrap_or(false) { on_arrival(); }
                Ok(())
            });
            match RawGameController::RawGameControllerAdded(&handler) {
                Ok(_token) => { let _ = tx.send(Ok(())); loop { std::thread::park(); } }
                Err(e) => { let _ = tx.send(Err(format!("Windows would not report new controllers: {e}"))); }
            }
        });
        let result = match spawned {
            Err(e) => Err(format!("The controller watcher could not start: {e}")),
            Ok(_) => rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "The controller watcher did not start in time.".to_string()).and_then(|r| r),
        };
        if result.is_err() { RUNNING.store(false, Ordering::SeqCst); }
        result
    }
    pub fn running() -> bool { RUNNING.load(Ordering::SeqCst) }
    /// True when Windows lists at least one game controller. Windows fills that list a moment after it is first asked,
    /// so an empty answer is checked again for up to `wait`. Only the count is read, never a button.
    pub fn connected(wait: Duration) -> bool {
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let begun = Instant::now();
        loop {
            if RawGameController::RawGameControllers().and_then(|list| list.Size()).map(|n| n > 0).unwrap_or(false) { return true; }
            if begun.elapsed() >= wait { return false; }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        // What Windows does when a real pad connects cannot be reproduced on a machine with no controller: this only
        // proves the subscription itself is accepted, without any device, and that a second start is harmless.
        #[test] fn the_subscription_is_accepted_without_any_controller_and_a_second_start_is_harmless() {
            start(|| {}).expect("Windows accepts the subscription");
            assert!(running());
            start(|| {}).expect("starting twice is harmless");
        }
    }
}
#[cfg(windows)]
pub use imp::{connected, running, start};

#[cfg(not(windows))]
pub fn start(_: impl Fn() + Send + Sync + 'static) -> Result<(), String> { Err("Controller wake-up needs Windows".into()) }
#[cfg(not(windows))]
pub fn running() -> bool { false }
#[cfg(not(windows))]
pub fn connected(_: std::time::Duration) -> bool { false }
