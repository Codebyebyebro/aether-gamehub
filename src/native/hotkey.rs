//! One global shortcut that brings Aether Hub back from anywhere. It uses the plain Windows `RegisterHotKey` call on a
//! small thread of its own: no extra libraries, and nothing is hooked or read, so it cannot see any other keystroke.

/// What the player presses. Ctrl+Alt+Space needs no letter, so it cannot clash with the characters that
/// Ctrl+Alt (AltGr) types on many keyboard layouts.
pub const LABEL: &str = "Ctrl+Alt+Space";

#[cfg(windows)]
mod imp {
    use super::LABEL;
    use std::{sync::{mpsc, Mutex}, thread::JoinHandle, time::Duration};
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, VK_SPACE},
            WindowsAndMessaging::{GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_HOTKEY, WM_QUIT, WM_USER},
        },
    };
    struct Running { thread: u32, join: JoinHandle<()> }
    static RUNNING: Mutex<Option<Running>> = Mutex::new(None);
    const ID: i32 = 0x41E7;

    /// Registers the shortcut. An error means another program already holds it (or Windows refused); nothing is left registered then.
    pub fn start(on_press: impl Fn() + Send + 'static) -> Result<(), String> {
        let mut slot = RUNNING.lock().map_err(|_| "The shortcut state is unavailable".to_string())?;
        if slot.is_some() { return Ok(()); }
        let (tx, rx) = mpsc::channel::<Result<u32, String>>();
        let join = std::thread::Builder::new().name("global-shortcut".into()).spawn(move || unsafe {
            let mut msg = MSG::default();
            // Looking at the queue once creates this thread's message queue, so a stop request can be posted to it.
            let _ = PeekMessageW(&mut msg, HWND::default(), WM_USER, WM_USER, PM_NOREMOVE);
            if let Err(e) = RegisterHotKey(HWND::default(), ID, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(VK_SPACE.0)) {
                let _ = tx.send(Err(format!("{LABEL} could not be reserved. Another program is probably using it ({e}).")));
                return;
            }
            let _ = tx.send(Ok(GetCurrentThreadId()));
            while GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
                if msg.message == WM_HOTKEY && msg.wParam.0 == ID as usize { on_press(); }
            }
            let _ = UnregisterHotKey(HWND::default(), ID);
        }).map_err(|e| format!("The shortcut could not start: {e}"))?;
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(thread)) => { *slot = Some(Running { thread, join }); Ok(()) }
            Ok(Err(e)) => { let _ = join.join(); Err(e) }
            Err(_) => Err("The shortcut did not start in time.".into()),
        }
    }
    pub fn stop() {
        let running = RUNNING.lock().ok().and_then(|mut slot| slot.take());
        if let Some(Running { thread, join }) = running {
            unsafe { let _ = PostThreadMessageW(thread, WM_QUIT, WPARAM(0), LPARAM(0)); }
            let _ = join.join();
        }
    }
    pub fn active() -> bool { RUNNING.lock().map(|slot| slot.is_some()).unwrap_or(false) }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::atomic::{AtomicBool, Ordering};
        fn can_register_here() -> bool {
            unsafe {
                let ok = RegisterHotKey(HWND::default(), 0x41E8, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(VK_SPACE.0)).is_ok();
                if ok { let _ = UnregisterHotKey(HWND::default(), 0x41E8); }
                ok
            }
        }
        // These talk to the real Windows hotkey table, so one test runs them in order (tests run in parallel).
        #[test] fn the_shortcut_is_reserved_released_and_a_clash_is_reported() {
            if !can_register_here() { eprintln!("SKIP: this session cannot register a global hotkey at all"); return; }
            let pressed = std::sync::Arc::new(AtomicBool::new(false));
            let seen = pressed.clone();
            start(move || seen.store(true, Ordering::SeqCst)).expect("the shortcut registers when nobody holds it");
            assert!(active());
            start(|| {}).expect("starting twice is harmless");
            assert!(!pressed.load(Ordering::SeqCst), "nothing fires without a key press");
            // While it is held, nobody else can have it: this is what a clash with another program looks like.
            assert!(!can_register_here(), "Windows reports the combination as taken while we hold it");
            stop();
            assert!(!active());
            assert!(can_register_here(), "stopping releases the combination");
            // Another program holds it: our attempt must fail with a message and leave nothing behind.
            unsafe { RegisterHotKey(HWND::default(), 0x41E9, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(VK_SPACE.0)).unwrap(); }
            let clash = start(|| {});
            unsafe { let _ = UnregisterHotKey(HWND::default(), 0x41E9); }
            let message = clash.expect_err("a held combination cannot be taken twice");
            assert!(message.contains(LABEL), "{message}");
            assert!(!active(), "a failed start leaves nothing running");
        }
    }
}
#[cfg(windows)]
pub use imp::{active, start, stop};

#[cfg(not(windows))]
pub fn start(_: impl Fn() + Send + 'static) -> Result<(), String> { Err("The global shortcut needs Windows".into()) }
#[cfg(not(windows))]
pub fn stop() {}
#[cfg(not(windows))]
pub fn active() -> bool { false }
