//! Bringing Aether Hub's window to the front when the controller asks for it (the PS button, a game ending, a
//! controller connecting). Windows normally refuses to let a program that is not in front take focus, which leaves
//! only a flashing taskbar button; joining the input queue of the window in front for a moment is the documented way
//! a program the user is actively using takes over.

pub fn force(raw: isize) {
    use windows::Win32::{
        Foundation::HWND,
        System::Threading::{AttachThreadInput, GetCurrentThreadId},
        UI::WindowsAndMessaging::{BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW},
    };
    unsafe {
        let hwnd = HWND(raw as _);
        let front = GetForegroundWindow();
        let me = GetCurrentThreadId();
        let other = GetWindowThreadProcessId(front, None);
        let joined = other != 0 && other != me && AttachThreadInput(me, other, true).as_bool();
        let _ = ShowWindow(hwnd, if IsIconic(hwnd).as_bool() { SW_RESTORE } else { SW_SHOW });
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if joined { let _ = AttachThreadInput(me, other, false); }
    }
}
