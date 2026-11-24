//! The PS / Xbox button as a console "home" button, also while a game has the screen.
//!
//! The browser only sees the controller while Aether Hub's window is in front, so this listens natively, and only while
//! the player has switched it on:
//! - PlayStation controllers (DualSense, DualShock 4): Windows' Raw Input delivers their HID reports to a hidden window
//!   even in the background (`RIDEV_INPUTSINK`). Only the PS button's bit is looked at; nothing is stored or sent.
//! - Xbox controllers: their guide button is only readable through XInput's unnamed `XInputGetStateEx`, polled a few
//!   times a second. Only the guide bit is looked at.
//! Steam and the Xbox Game Bar may react to the same button; that is theirs to switch off.
use std::time::{Duration, Instant};

/// PlayStation controllers whose PS button can be read from their reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sony { DualSense, DualShock4 }
pub const SONY: u32 = 0x054C;
pub fn sony_model(vendor: u32, product: u32) -> Option<Sony> {
    if vendor != SONY { return None; }
    match product { 0x0CE6 | 0x0DF2 => Some(Sony::DualSense), 0x05C4 | 0x09CC | 0x0BA0 => Some(Sony::DualShock4), _ => None }
}
/// Whether the PS button is down in one input report (report id first), or None for a report this does not know.
/// Layouts follow the controllers' published HID reports: USB, Bluetooth "simple" and Bluetooth "full" modes.
pub fn ps_pressed(model: Sony, report: &[u8]) -> Option<bool> {
    let bit = |i: usize| report.get(i).map(|b| b & 0x01 == 0x01);
    match (model, *report.first()?) {
        (Sony::DualSense, 0x01) if report.len() >= 64 => bit(10), // USB
        (Sony::DualSense, 0x01) if report.len() >= 10 => bit(7),  // Bluetooth, simple mode
        (Sony::DualSense, 0x31) if report.len() >= 12 => bit(11), // Bluetooth, full mode
        (Sony::DualShock4, 0x01) if report.len() >= 10 => bit(7), // USB and Bluetooth simple mode
        (Sony::DualShock4, 0x11) if report.len() >= 10 => bit(9), // Bluetooth, full mode
        _ => None,
    }
}
/// XInput's guide button bit, only reported by `XInputGetStateEx`.
pub const XINPUT_GUIDE: u16 = 0x0400;

/// Turns a stream of "is it down" readings into one press per push. A push within `gap` of the last one is ignored,
/// so a bouncing button, or two ways of reading the same controller, never count twice.
#[derive(Default)]
pub struct Edge { down: bool, last: Option<Instant> }
impl Edge {
    pub fn feed(&mut self, down: bool, now: Instant, gap: Duration) -> bool {
        let pressed = down && !self.down && self.last.is_none_or(|t| now.duration_since(t) >= gap);
        if pressed { self.last = Some(now); }
        self.down = down;
        pressed
    }
}
/// The shortest time between two presses that count.
pub const GAP: Duration = Duration::from_millis(600);

#[cfg(windows)]
mod imp {
    use super::{ps_pressed, sony_model, Edge, Sony, GAP, XINPUT_GUIDE};
    use std::{collections::HashMap, sync::{atomic::{AtomicBool, Ordering}, mpsc, Mutex, OnceLock}, time::{Duration, Instant}};
    use windows::{
        core::{w, PCSTR},
        Win32::{
            Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM},
            System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW},
            UI::{
                Input::{GetRawInputData, GetRawInputDeviceInfoW, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RIDEV_INPUTSINK, RIDI_DEVICEINFO, RID_DEVICE_INFO, RID_INPUT, RIM_TYPEHID},
                WindowsAndMessaging::{CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, TranslateMessage, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_INPUT, WNDCLASSW},
            },
        },
    };

    /// The Raw Input listener is running. Windows allows one per program, so controller layouts share it (`listen_reports`).
    static RUNNING: AtomicBool = AtomicBool::new(false);
    static XINPUT_POLL: AtomicBool = AtomicBool::new(false);
    static ENABLED: AtomicBool = AtomicBool::new(false);
    /// Controller layouts' reader for every PlayStation report, while they are switched on.
    static SINK: OnceLock<fn(Sony, &[u8])> = OnceLock::new();
    static SINK_ON: AtomicBool = AtomicBool::new(false);
    static ON_PRESS: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
    /// One press detector for every source, so a controller seen two ways still counts once.
    static EDGE: Mutex<Option<Edge>> = Mutex::new(None);
    /// Raw Input device handle → its model (None: not a PlayStation controller this knows).
    static MODELS: Mutex<Option<HashMap<isize, Option<Sony>>>> = Mutex::new(None);

    fn report(down: bool) {
        if !ENABLED.load(Ordering::SeqCst) { return; }
        let pressed = EDGE.lock().map(|mut e| e.get_or_insert_with(Edge::default).feed(down, Instant::now(), GAP)).unwrap_or(false);
        if pressed { if let Some(f) = ON_PRESS.get() { f(); } }
    }

    /// Starts listening (once; later calls only switch it back on). `on_press` runs on a worker thread.
    pub fn start(on_press: impl Fn() + Send + Sync + 'static) -> Result<(), String> {
        let _ = ON_PRESS.set(Box::new(on_press));
        // Xbox: a separate, gentle poll. Its failure (no XInput on this PC) does not stop the PlayStation side.
        if !XINPUT_POLL.swap(true, Ordering::SeqCst) { let _ = std::thread::Builder::new().name("home-button-xinput".into()).spawn(poll_xinput); }
        let result = raw();
        ENABLED.store(result.is_ok(), Ordering::SeqCst);
        result
    }
    /// Hands every PlayStation controller report to `sink` (on the listener's thread) until `stop_reports`.
    pub fn listen_reports(sink: fn(Sony, &[u8])) -> Result<(), String> {
        let _ = SINK.set(sink);
        SINK_ON.store(true, Ordering::SeqCst);
        raw()
    }
    pub fn stop_reports() { SINK_ON.store(false, Ordering::SeqCst); }
    /// Starts the Raw Input listener once; later calls only report whether it is running.
    fn raw() -> Result<(), String> {
        static FAILED: Mutex<Option<String>> = Mutex::new(None);
        if RUNNING.swap(true, Ordering::SeqCst) { return FAILED.lock().map(|f| f.clone()).unwrap_or(None).map_or(Ok(()), Err); }
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        let raw = std::thread::Builder::new().name("home-button-raw".into()).spawn(move || {
            if let Err(e) = unsafe { listen_raw() } { let _ = tx.send(Err(e)); return; }
            let _ = tx.send(Ok(()));
            unsafe { pump() };
        });
        let result = match raw {
            Err(e) => Err(format!("The PS button listener could not start: {e}")),
            Ok(_) => rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "The PS button listener did not start in time.".to_string()).and_then(|r| r),
        };
        // A failed start is remembered rather than retried: a second listener window would only fail the same way.
        if let Err(e) = &result { if let Ok(mut f) = FAILED.lock() { *f = Some(e.clone()); } }
        result
    }
    pub fn stop() { ENABLED.store(false, Ordering::SeqCst); }
    pub fn active() -> bool { RUNNING.load(Ordering::SeqCst) && ENABLED.load(Ordering::SeqCst) }

    unsafe fn listen_raw() -> Result<(), String> {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("AetherHubHomeButton");
        let wc = WNDCLASSW { lpfnWndProc: Some(window_proc), hInstance: instance.into(), lpszClassName: class, ..Default::default() };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), class, w!(""), WINDOW_STYLE(0), 0, 0, 0, 0, HWND_MESSAGE, None, instance, None).map_err(|e| format!("Windows would not create the listener: {e}"))?;
        // Game pads (usage 5) and joysticks (usage 4) on the Generic Desktop page, even while another window is in front.
        let devices = [5u16, 4].map(|usage| RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: usage, dwFlags: RIDEV_INPUTSINK, hwndTarget: hwnd });
        RegisterRawInputDevices(&devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32).map_err(|e| format!("Windows would not report controller input: {e}"))
    }
    unsafe fn pump() {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() { let _ = TranslateMessage(&msg); DispatchMessageW(&msg); }
    }
    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if message == WM_INPUT && (ENABLED.load(Ordering::SeqCst) || SINK_ON.load(Ordering::SeqCst)) { read_input(HRAWINPUT(lparam.0 as _)); }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
    unsafe fn model_of(device: HANDLE) -> Option<Sony> {
        let key = device.0 as isize;
        if let Some(known) = MODELS.lock().ok().and_then(|m| m.as_ref().and_then(|m| m.get(&key).copied())) { return known; }
        let mut info = RID_DEVICE_INFO { cbSize: std::mem::size_of::<RID_DEVICE_INFO>() as u32, ..Default::default() };
        let mut size = info.cbSize;
        let got = GetRawInputDeviceInfoW(device, RIDI_DEVICEINFO, Some(&mut info as *mut _ as *mut _), &mut size);
        let model = if got != u32::MAX && got > 0 && info.dwType == RIM_TYPEHID { sony_model(info.Anonymous.hid.dwVendorId, info.Anonymous.hid.dwProductId) } else { None };
        if let Ok(mut m) = MODELS.lock() { m.get_or_insert_with(HashMap::new).insert(key, model); }
        model
    }
    unsafe fn read_input(handle: HRAWINPUT) {
        let header = std::mem::size_of::<RAWINPUTHEADER>() as u32;
        let mut size = 0u32;
        if GetRawInputData(handle, RID_INPUT, None, &mut size, header) != 0 || size == 0 || size > 64 * 1024 { return; }
        // u64 storage keeps the RAWINPUT header properly aligned.
        let mut storage = vec![0u64; (size as usize).div_ceil(8)];
        let got = GetRawInputData(handle, RID_INPUT, Some(storage.as_mut_ptr() as *mut _), &mut size, header);
        if got == u32::MAX || got == 0 { return; }
        let raw = &*(storage.as_ptr() as *const RAWINPUT);
        if raw.header.dwType != RIM_TYPEHID.0 { return; }
        let Some(model) = model_of(raw.header.hDevice) else { return; };
        let hid = &raw.data.hid;
        let (each, count) = (hid.dwSizeHid as usize, hid.dwCount as usize);
        let start = hid.bRawData.as_ptr() as usize - storage.as_ptr() as usize;
        let bytes = std::slice::from_raw_parts(storage.as_ptr() as *const u8, got as usize);
        let Some(data) = bytes.get(start..start.saturating_add(each.saturating_mul(count))) else { return; };
        if each == 0 { return; }
        let sink = SINK.get().filter(|_| SINK_ON.load(Ordering::SeqCst));
        for one in data.chunks(each) {
            if let Some(down) = ps_pressed(model, one) { report(down); }
            if let Some(sink) = sink { sink(model, one); }
        }
    }

    #[repr(C)]
    #[derive(Default)]
    struct XState { packet: u32, buttons: u16, left_trigger: u8, right_trigger: u8, lx: i16, ly: i16, rx: i16, ry: i16, extra: u32 }
    type GetStateEx = unsafe extern "system" fn(u32, *mut XState) -> u32;
    fn poll_xinput() {
        let get: Option<GetStateEx> = unsafe {
            [w!("xinput1_4.dll"), w!("xinput1_3.dll")].into_iter().find_map(|name| LoadLibraryW(name).ok())
                .and_then(|lib| GetProcAddress(lib, PCSTR(100usize as *const u8)))
                .map(|f| std::mem::transmute::<unsafe extern "system" fn() -> isize, GetStateEx>(f))
        };
        let Some(get) = get else { return; };
        // Empty slots are slow to ask about, so they are rechecked only every few seconds.
        let mut connected = [true; 4]; let mut recheck = Instant::now();
        loop {
            if !ENABLED.load(Ordering::SeqCst) { std::thread::sleep(Duration::from_millis(500)); continue; }
            let retry = recheck.elapsed() >= Duration::from_secs(3);
            if retry { recheck = Instant::now(); }
            for slot in 0..4u32 {
                if !connected[slot as usize] && !retry { continue; }
                let mut state = XState::default();
                let ok = unsafe { get(slot, &mut state) } == 0;
                connected[slot as usize] = ok;
                if ok { report(state.buttons & XINPUT_GUIDE != 0); }
            }
            std::thread::sleep(Duration::from_millis(60));
        }
    }
}
#[cfg(windows)]
pub use imp::{active, listen_reports, start, stop, stop_reports};

#[cfg(not(windows))]
pub fn start(_: impl Fn() + Send + Sync + 'static) -> Result<(), String> { Err("The PS / Xbox button needs Windows".into()) }
#[cfg(not(windows))]
pub fn stop() {}
#[cfg(not(windows))]
pub fn active() -> bool { false }

#[cfg(test)]
mod tests {
    use super::*;
    fn usb_dualsense(ps: bool) -> Vec<u8> { let mut r = vec![0u8; 64]; r[0] = 0x01; r[8] = 0x08; if ps { r[10] = 0x01; } r }
    #[test] fn only_known_playstation_controllers_are_recognised() {
        assert_eq!(sony_model(0x054C, 0x0CE6), Some(Sony::DualSense));
        assert_eq!(sony_model(0x054C, 0x0DF2), Some(Sony::DualSense), "DualSense Edge");
        assert_eq!(sony_model(0x054C, 0x09CC), Some(Sony::DualShock4));
        assert_eq!(sony_model(0x054C, 0x1234), None);
        assert_eq!(sony_model(0x045E, 0x0CE6), None, "same product number from another maker");
    }
    #[test] fn the_ps_button_is_read_from_a_dualsense_over_usb() {
        assert_eq!(ps_pressed(Sony::DualSense, &usb_dualsense(true)), Some(true));
        assert_eq!(ps_pressed(Sony::DualSense, &usb_dualsense(false)), Some(false));
        let mut touchpad = usb_dualsense(false); touchpad[10] = 0x02;
        assert_eq!(ps_pressed(Sony::DualSense, &touchpad), Some(false), "the touchpad click is the next bit, not the PS button");
        let mut cross = usb_dualsense(false); cross[8] = 0x28; cross[9] = 0xFF;
        assert_eq!(ps_pressed(Sony::DualSense, &cross), Some(false), "other buttons do not count");
    }
    #[test] fn the_ps_button_is_read_over_bluetooth_in_both_modes() {
        let mut simple = vec![0u8; 10]; simple[0] = 0x01; simple[7] = 0x01;
        assert_eq!(ps_pressed(Sony::DualSense, &simple), Some(true));
        let mut full = vec![0u8; 78]; full[0] = 0x31; full[11] = 0x01;
        assert_eq!(ps_pressed(Sony::DualSense, &full), Some(true));
        full[11] = 0x00; assert_eq!(ps_pressed(Sony::DualSense, &full), Some(false));
        let mut ds4 = vec![0u8; 78]; ds4[0] = 0x11; ds4[9] = 0x01;
        assert_eq!(ps_pressed(Sony::DualShock4, &ds4), Some(true));
        let mut ds4_usb = vec![0u8; 64]; ds4_usb[0] = 0x01; ds4_usb[7] = 0x05;
        assert_eq!(ps_pressed(Sony::DualShock4, &ds4_usb), Some(true), "the counter in the upper bits is ignored");
    }
    #[test] fn unknown_or_short_reports_are_ignored() {
        assert_eq!(ps_pressed(Sony::DualSense, &[]), None);
        assert_eq!(ps_pressed(Sony::DualSense, &[0x05, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]), None, "feature/other report ids");
        assert_eq!(ps_pressed(Sony::DualSense, &[0x31, 0, 0]), None);
        assert_eq!(ps_pressed(Sony::DualShock4, &[0x01, 0, 0]), None);
    }
    #[test] fn one_push_is_one_press_and_bounces_do_not_count() {
        let t0 = Instant::now(); let ms = |n| t0 + Duration::from_millis(n);
        let mut edge = Edge::default();
        assert!(!edge.feed(false, ms(0), GAP));
        assert!(edge.feed(true, ms(10), GAP), "pressed");
        assert!(!edge.feed(true, ms(30), GAP), "held is not another press");
        assert!(!edge.feed(false, ms(40), GAP));
        assert!(!edge.feed(true, ms(90), GAP), "a bounce soon after does not count");
        assert!(!edge.feed(false, ms(100), GAP));
        assert!(edge.feed(true, ms(700), GAP), "a real second push does");
    }
    #[test] fn the_xbox_guide_bit_is_the_one_xinput_hides() { assert_eq!(XINPUT_GUIDE, 0x0400); }
}
