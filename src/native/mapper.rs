//! Controller layouts: while a game launched from Aether Hub has the screen, its controller is turned into keyboard
//! and mouse input, using the layout saved with that game. Off until the player switches it on in Settings.
//!
//! - PlayStation controllers are read from their HID reports, which `homebutton.rs` already receives through Raw
//!   Input (Windows allows one Raw Input listener per program, so the two share it). Xbox controllers through XInput.
//! - Keys are sent with `SendInput` as scan codes (what games read), the mouse as relative moves, clicks and wheel.
//! - Input is only sent while the window in front belongs to the game's own processes, so Aether Hub's window, the
//!   desktop and other programs never receive any. Every key it pressed is released when it stops, for any reason.
//! - The PS / Xbox button is never mapped: it stays the quick menu's.
//!
//! Everything from report bytes to key and mouse events is pure and tested here; only `imp` talks to Windows.
use crate::homebutton::Sony;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A game's layout, saved with the game. Button names are `BUTTONS`; keys are browser `KeyboardEvent.code` values
/// ("KeyW", "Space", "ControlLeft") or the mouse names in `MOUSE`. Stick modes: "wasd", "arrows", "mouse", "scroll"
/// or anything else for nothing. `preset` only names where the layout came from, for the interface.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Controls { pub preset: String, pub buttons: BTreeMap<String, String>, pub left_stick: String, pub right_stick: String, pub mouse_speed: u32 }

/// Bit order of `Pad::buttons`. The PS button is left out on purpose.
pub const BUTTONS: [&str; 16] = ["cross", "circle", "square", "triangle", "l1", "r1", "l2", "r2", "l3", "r3", "options", "create", "up", "down", "left", "right"];
const fn bit(name: &str) -> u32 {
    let mut i = 0;
    while i < BUTTONS.len() { if const_eq(BUTTONS[i], name) { return 1 << i; } i += 1; }
    panic!("unknown button");
}
const fn const_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() { return false; }
    let mut i = 0;
    while i < a.len() { if a[i] != b[i] { return false; } i += 1; }
    true
}

/// One reading of a controller. Sticks run from -1 to 1 with y pointing down (like the screen); triggers 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad { pub buttons: u32, pub lx: f32, pub ly: f32, pub rx: f32, pub ry: f32, pub l2: f32, pub r2: f32 }
impl Pad {
    #[cfg(test)]
    pub fn down(&self, name: &str) -> bool { BUTTONS.iter().position(|b| *b == name).is_some_and(|i| self.buttons & (1 << i) != 0) }
    /// Triggers count as pressed past half travel, the same as the interface's controller handling.
    fn with_triggers(mut self) -> Self {
        if self.l2 > 0.5 { self.buttons |= bit("l2"); }
        if self.r2 > 0.5 { self.buttons |= bit("r2"); }
        self
    }
}
fn axis(b: u8) -> f32 { ((b as f32 - 127.5) / 127.5).clamp(-1.0, 1.0) }
/// The D-pad as a hat: 0 is up, then clockwise in eighths; 8 or more is released.
fn hat(h: u8) -> u32 {
    let (up, right, down, left) = (bit("up"), bit("right"), bit("down"), bit("left"));
    match h { 0 => up, 1 => up | right, 2 => right, 3 => right | down, 4 => down, 5 => down | left, 6 => left, 7 => left | up, _ => 0 }
}

/// A PlayStation controller's full state from one input report (report id first), or None for a report this does not
/// know. Offsets follow the controllers' published HID layouts, the same three cases as `homebutton::ps_pressed`.
pub fn parse_sony(model: Sony, r: &[u8]) -> Option<Pad> {
    // (first stick byte, face/hat byte, shoulder byte, L2 byte, R2 byte). Sticks are four bytes LX LY RX RY.
    let (sticks, face, shoulders, l2, r2) = match (model, *r.first()?) {
        (Sony::DualSense, 0x01) if r.len() >= 64 => (1, 8, 9, 5, 6),   // USB
        (Sony::DualSense, 0x01) if r.len() >= 10 => (1, 5, 6, 8, 9),   // Bluetooth, simple mode
        (Sony::DualSense, 0x31) if r.len() >= 12 => (2, 9, 10, 6, 7),  // Bluetooth, full mode
        (Sony::DualShock4, 0x01) if r.len() >= 10 => (1, 5, 6, 8, 9),  // USB and Bluetooth simple mode
        (Sony::DualShock4, 0x11) if r.len() >= 12 => (3, 7, 8, 10, 11), // Bluetooth, full mode
        _ => return None,
    };
    let (f, s) = (r[face], r[shoulders]);
    let mut buttons = hat(f & 0x0F);
    for (mask, name) in [(0x10, "square"), (0x20, "cross"), (0x40, "circle"), (0x80, "triangle")] { if f & mask != 0 { buttons |= bit(name); } }
    for (mask, name) in [(0x01, "l1"), (0x02, "r1"), (0x10, "create"), (0x20, "options"), (0x40, "l3"), (0x80, "r3")] { if s & mask != 0 { buttons |= bit(name); } }
    Some(Pad { buttons, lx: axis(r[sticks]), ly: axis(r[sticks + 1]), rx: axis(r[sticks + 2]), ry: axis(r[sticks + 3]), l2: r[l2] as f32 / 255.0, r2: r[r2] as f32 / 255.0 }.with_triggers())
}

/// An Xbox controller's state from `XInputGetState` (XInput's y points up, so it is turned over).
pub fn from_xinput(buttons: u16, lt: u8, rt: u8, lx: i16, ly: i16, rx: i16, ry: i16) -> Pad {
    let map = [(0x0001, "up"), (0x0002, "down"), (0x0004, "left"), (0x0008, "right"), (0x0010, "options"), (0x0020, "create"), (0x0040, "l3"), (0x0080, "r3"),
        (0x0100, "l1"), (0x0200, "r1"), (0x1000, "cross"), (0x2000, "circle"), (0x4000, "square"), (0x8000, "triangle")];
    let bits = map.iter().filter(|(m, _)| buttons & m != 0).fold(0, |acc, (_, n)| acc | bit(n));
    let a = |v: i16| (v as f32 / 32767.0).clamp(-1.0, 1.0);
    Pad { buttons: bits, lx: a(lx), ly: -a(ly), rx: a(rx), ry: -a(ry), l2: lt as f32 / 255.0, r2: rt as f32 / 255.0 }.with_triggers()
}

/// Sticks rest a little off centre on real controllers; this much travel is ignored.
pub const DEAD_ZONE: f32 = 0.15;
/// Radial dead zone, then a curve: small pushes give fine aim, a full push the full speed. Length 0 to 1.
pub fn shape(x: f32, y: f32) -> (f32, f32) {
    let m = x.hypot(y);
    if m <= DEAD_ZONE { return (0.0, 0.0); }
    let scaled = ((m - DEAD_ZONE) / (1.0 - DEAD_ZONE)).min(1.0);
    let k = scaled * scaled / m;
    (x * k, y * k)
}
/// A stick sending keys: on past `KEY_ON`, off below `KEY_OFF`, so a thumb resting near the edge does not flicker.
const KEY_ON: f32 = 0.5;
const KEY_OFF: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MouseButton { Left, Right, Middle }
/// Something held down: a key (Windows virtual-key code) or a mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Out { Key(u16), Mouse(MouseButton) }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event { Down(Out), Up(Out), Move(i32, i32), Wheel(i32) }

/// The Windows virtual-key code for a `KeyboardEvent.code`, or None for one this does not send.
pub fn vk(code: &str) -> Option<u16> {
    if let Some(c) = code.strip_prefix("Key").filter(|c| c.len() == 1 && c.as_bytes()[0].is_ascii_uppercase()) { return Some(c.as_bytes()[0] as u16); }
    if let Some(d) = code.strip_prefix("Digit").filter(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_digit()) { return Some(d.as_bytes()[0] as u16); }
    if let Some(d) = code.strip_prefix("Numpad").filter(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_digit()) { return Some(0x60 + (d.as_bytes()[0] - b'0') as u16); }
    if let Some(n) = code.strip_prefix('F').and_then(|n| n.parse::<u16>().ok()).filter(|n| (1..=12).contains(n)) { return Some(0x6F + n); }
    Some(match code {
        "Space" => 0x20, "Enter" => 0x0D, "Escape" => 0x1B, "Tab" => 0x09, "Backspace" => 0x08, "CapsLock" => 0x14,
        "ShiftLeft" => 0xA0, "ShiftRight" => 0xA1, "ControlLeft" => 0xA2, "ControlRight" => 0xA3, "AltLeft" => 0xA4, "AltRight" => 0xA5,
        "ArrowUp" => 0x26, "ArrowDown" => 0x28, "ArrowLeft" => 0x25, "ArrowRight" => 0x27,
        "Insert" => 0x2D, "Delete" => 0x2E, "Home" => 0x24, "End" => 0x23, "PageUp" => 0x21, "PageDown" => 0x22,
        "Minus" => 0xBD, "Equal" => 0xBB, "BracketLeft" => 0xDB, "BracketRight" => 0xDD, "Backslash" => 0xDC,
        "Semicolon" => 0xBA, "Quote" => 0xDE, "Backquote" => 0xC0, "Comma" => 0xBC, "Period" => 0xBE, "Slash" => 0xBF,
        _ => return None,
    })
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Bind { Hold(Out), Wheel(i32) }
fn bind(code: &str) -> Option<Bind> {
    Some(match code {
        "MouseLeft" => Bind::Hold(Out::Mouse(MouseButton::Left)), "MouseRight" => Bind::Hold(Out::Mouse(MouseButton::Right)),
        "MouseMiddle" => Bind::Hold(Out::Mouse(MouseButton::Middle)), "WheelUp" => Bind::Wheel(1), "WheelDown" => Bind::Wheel(-1),
        other => Bind::Hold(Out::Key(vk(other)?)),
    })
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stick { Keys([u16; 4]), Mouse, Scroll, Nothing }
fn stick(mode: &str) -> Stick {
    match mode { "wasd" => Stick::Keys([0x57, 0x41, 0x53, 0x44]), "arrows" => Stick::Keys([0x26, 0x25, 0x28, 0x27]), "mouse" => Stick::Mouse, "scroll" => Stick::Scroll, _ => Stick::Nothing }
}
/// Mouse speed at full push, in pixels a second, for the layout's speed setting (1 to 10, 5 when unset).
pub fn mouse_px_per_sec(speed: u32) -> f32 { let s = if speed == 0 { 5 } else { speed.clamp(1, 10) }; 300.0 + 250.0 * s as f32 }
/// Wheel notches a second at full push of a "scroll" stick.
const SCROLL_PER_SEC: f32 = 10.0;

/// Turns controller readings into key and mouse events for one layout. Unknown buttons and keys in a layout are ignored.
pub struct Engine { binds: Vec<(u32, Bind)>, sticks: [Stick; 2], speed: f32, held: BTreeSet<Out>, stick_keys: BTreeSet<u16>, prev: u32, ignore: u32, rest: (f32, f32), wheel: f32 }
impl Engine {
    pub fn new(c: &Controls) -> Self {
        let binds = c.buttons.iter().filter_map(|(button, code)| Some((bit_of(button)?, bind(code)?))).collect();
        Engine { binds, sticks: [stick(&c.left_stick), stick(&c.right_stick)], speed: mouse_px_per_sec(c.mouse_speed), held: BTreeSet::new(), stick_keys: BTreeSet::new(), prev: 0, ignore: 0, rest: (0.0, 0.0), wheel: 0.0 }
    }
    /// The events for one reading, `dt` seconds after the last one. Releases come before presses.
    pub fn step(&mut self, pad: &Pad, dt: f32) -> Vec<Event> {
        let dt = dt.clamp(0.0, 0.1); // after a stall, one small step rather than a jump across the screen
        let mut events = vec![];
        let mut want = BTreeSet::new();
        self.ignore &= pad.buttons;
        let live = pad.buttons & !self.ignore;
        for (mask, b) in &self.binds {
            if live & mask == 0 { continue; }
            match b { Bind::Hold(o) => { want.insert(*o); } Bind::Wheel(n) => if self.prev & mask == 0 { events.push(Event::Wheel(*n)); } }
        }
        self.prev = live;
        let (mut mx, mut my, mut wheel) = (0.0, 0.0, 0.0);
        let mut stick_keys = BTreeSet::new();
        for (mode, (x, y)) in self.sticks.iter().zip([(pad.lx, pad.ly), (pad.rx, pad.ry)]) {
            match mode {
                Stick::Keys([up, left, down, right]) => for (key, v) in [(up, -y), (left, -x), (down, y), (right, x)] {
                    if v > if self.stick_keys.contains(key) { KEY_OFF } else { KEY_ON } { stick_keys.insert(*key); want.insert(Out::Key(*key)); }
                },
                Stick::Mouse => { let (sx, sy) = shape(x, y); mx += sx * self.speed * dt; my += sy * self.speed * dt; }
                Stick::Scroll => { wheel += -shape(0.0, y).1 * SCROLL_PER_SEC * dt; }
                Stick::Nothing => {}
            }
        }
        self.stick_keys = stick_keys;
        events.extend(self.held.difference(&want).map(|o| Event::Up(*o)));
        events.extend(want.difference(&self.held).map(|o| Event::Down(*o)));
        self.held = want;
        // Whole pixels and notches go out now; the fractions are kept so slow movement still adds up.
        self.rest = (self.rest.0 + mx, self.rest.1 + my);
        let (dx, dy) = (self.rest.0.trunc(), self.rest.1.trunc());
        self.rest = (self.rest.0 - dx, self.rest.1 - dy);
        if mx == 0.0 && my == 0.0 { self.rest = (0.0, 0.0); }
        if dx != 0.0 || dy != 0.0 { events.push(Event::Move(dx as i32, dy as i32)); }
        self.wheel = if wheel == 0.0 { 0.0 } else { self.wheel + wheel };
        let notches = self.wheel.trunc();
        if notches != 0.0 { self.wheel -= notches; events.push(Event::Wheel(notches as i32)); }
        events
    }
    /// Lets go of everything this is holding (the game ended, lost the screen, or the setting was switched off).
    pub fn release_all(&mut self) -> Vec<Event> {
        self.stick_keys.clear(); self.rest = (0.0, 0.0); self.wheel = 0.0;
        std::mem::take(&mut self.held).into_iter().map(Event::Up).collect()
    }
    #[cfg(test)]
    pub fn holding(&self) -> bool { !self.held.is_empty() }
    /// Buttons already down when mapping (re)starts are ignored until let go, so a press made in Aether Hub (✕ on
    /// Play or Resume) does not spill into the game.
    pub fn settle(&mut self, pad: &Pad) { self.ignore = pad.buttons; self.prev = pad.buttons; }
}
fn bit_of(name: &str) -> Option<u32> { BUTTONS.iter().position(|b| *b == name).map(|i| 1 << i) }

/// What Settings shows about the feature right now, in plain words.
pub fn describe(enabled: bool, error: &str, game: Option<&str>, sending: bool) -> String {
    match (enabled, game) {
        (false, _) => "Off".into(),
        _ if !error.is_empty() => format!("Not working: {error}"),
        (true, None) => "Ready. It starts when you launch a game that has a controller layout.".into(),
        (true, Some(title)) if sending => format!("Working: your controller is sending keys and mouse to {title}."),
        (true, Some(title)) => format!("Waiting: {title} is running but not in front, so nothing is sent."),
    }
}

#[cfg(windows)]
mod imp {
    use super::{describe, from_xinput, parse_sony, Controls, Engine, Event, MouseButton, Out, Pad, Sony};
    use std::{sync::{atomic::{AtomicBool, Ordering}, Mutex}, time::{Duration, Instant}};
    use windows::{
        core::{s, w},
        Win32::{
            System::{LibraryLoader::{GetProcAddress, LoadLibraryW}, Threading::GetCurrentProcessId},
            UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId}},
        },
    };

    type Owns = Box<dyn FnMut(u32) -> bool + Send>;
    struct Game { id: String, title: String, controls: Controls, owns: Owns }
    struct Shared { enabled: bool, game: Option<Game>, paused_until: Option<Instant>, error: String, sending: bool }
    static SHARED: Mutex<Shared> = Mutex::new(Shared { enabled: false, game: None, paused_until: None, error: String::new(), sending: false });
    /// The engine for the game being mapped, kept here so every way of stopping can release its keys at once.
    static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
    static SONY_PAD: Mutex<Option<(Instant, Pad)>> = Mutex::new(None);
    static THREAD: AtomicBool = AtomicBool::new(false);
    const TICK: Duration = Duration::from_millis(8);
    /// A PlayStation reading older than this is stale (controller off, or it went quiet): XInput is asked instead.
    const FRESH: Duration = Duration::from_millis(500);

    fn shared() -> std::sync::MutexGuard<'static, Shared> { SHARED.lock().unwrap_or_else(|e| e.into_inner()) }
    fn engine() -> std::sync::MutexGuard<'static, Option<Engine>> { ENGINE.lock().unwrap_or_else(|e| e.into_inner()) }
    /// Releases every key and button this pressed, right now, from any thread.
    fn release_now() { let events = engine().as_mut().map(Engine::release_all).unwrap_or_default(); send(&events); }

    fn feed_sony(model: Sony, report: &[u8]) {
        if let Some(pad) = parse_sony(model, report) { if let Ok(mut p) = SONY_PAD.lock() { *p = Some((Instant::now(), pad)); } }
    }
    pub fn set_enabled(on: bool) -> Result<(), String> {
        shared().enabled = on;
        if !on { crate::homebutton::stop_reports(); release_now(); shared().error.clear(); return Ok(()); }
        let result = crate::homebutton::listen_reports(feed_sony).map_err(|e| format!("PlayStation controllers cannot be read: {e}"));
        if !THREAD.swap(true, Ordering::SeqCst) {
            if let Err(e) = std::thread::Builder::new().name("controller-layouts".into()).spawn(run) { THREAD.store(false, Ordering::SeqCst); shared().error = format!("Controller layouts could not start: {e}"); return Err(shared().error.clone()); }
        }
        shared().error = result.clone().err().unwrap_or_default();
        result
    }
    /// A game with a layout started. `owns` says whether a process id belongs to it.
    pub fn begin(id: &str, title: &str, controls: Option<Controls>, owns: Owns) {
        release_now(); *engine() = None;
        shared().game = controls.filter(|c| !c.preset.is_empty() && c.preset != "off").map(|controls| Game { id: id.into(), title: title.into(), controls, owns });
    }
    pub fn end(id: &str) {
        let mut s = shared();
        if s.game.as_ref().is_some_and(|g| g.id == id) { s.game = None; drop(s); release_now(); *engine() = None; }
    }
    /// The quick menu is opening: let go of everything now, and send nothing while Aether Hub comes forward.
    pub fn pause() { shared().paused_until = Some(Instant::now() + Duration::from_secs(3)); release_now(); }
    pub fn shutdown() { shared().game = None; release_now(); }
    pub fn status() -> (bool, String, String) {
        let s = shared();
        (s.enabled && s.error.is_empty() && THREAD.load(Ordering::SeqCst), describe(s.enabled, &s.error, s.game.as_ref().map(|g| g.title.as_str()), s.sending), s.error.clone())
    }

    fn foreground_pid() -> u32 {
        let mut pid = 0u32;
        unsafe { let hwnd = GetForegroundWindow(); if !hwnd.is_invalid() { GetWindowThreadProcessId(hwnd, Some(&mut pid)); } }
        pid
    }
    fn run() {
        let own = unsafe { GetCurrentProcessId() };
        let xinput = load_xinput();
        let (mut last, mut cached): (Instant, Option<(u32, bool, Instant)>) = (Instant::now(), None);
        loop {
            std::thread::sleep(TICK);
            let now = Instant::now();
            let dt = now.duration_since(last).as_secs_f32(); last = now;
            // Should input go out now? Only with the setting on, a game with a layout, no pause, and that game in front.
            let go = {
                let mut s = shared();
                if s.paused_until.is_some_and(|t| now >= t) { s.paused_until = None; }
                let pid = foreground_pid();
                let ok = s.enabled && s.paused_until.is_none() && pid != own && pid != 0 && match s.game.as_mut() {
                    None => false,
                    Some(g) => match cached {
                        Some((p, ok, at)) if p == pid && now.duration_since(at) < Duration::from_secs(2) => ok,
                        _ => { let ok = (g.owns)(pid); cached = Some((pid, ok, now)); ok }
                    },
                };
                s.sending = ok;
                ok.then(|| s.game.as_ref().map(|g| g.controls.clone())).flatten()
            };
            // A fresh engine next time, so buttons still held from Aether Hub are ignored until let go.
            let Some(controls) = go else { release_now(); *engine() = None; std::thread::sleep(Duration::from_millis(40)); continue; };
            let sony = SONY_PAD.lock().ok().and_then(|p| *p).filter(|(at, _)| now.duration_since(*at) < FRESH).map(|(_, p)| p);
            let pad = sony.or_else(|| xinput.and_then(read_xinput)).unwrap_or_default();
            let events = {
                let mut e = engine();
                let fresh = e.is_none();
                let engine = e.get_or_insert_with(|| Engine::new(&controls));
                if fresh { engine.settle(&pad); }
                engine.step(&pad, dt)
            };
            send(&events);
        }
    }

    #[repr(C)]
    #[derive(Default)]
    struct XState { packet: u32, buttons: u16, lt: u8, rt: u8, lx: i16, ly: i16, rx: i16, ry: i16 }
    type GetState = unsafe extern "system" fn(u32, *mut XState) -> u32;
    fn load_xinput() -> Option<GetState> {
        unsafe {
            [w!("xinput1_4.dll"), w!("xinput1_3.dll"), w!("xinput9_1_0.dll")].into_iter().find_map(|name| LoadLibraryW(name).ok())
                .and_then(|lib| GetProcAddress(lib, s!("XInputGetState")))
                .map(|f| std::mem::transmute::<unsafe extern "system" fn() -> isize, GetState>(f))
        }
    }
    /// The first connected Xbox controller. (Empty slots are slow to ask about, but this only runs while a game is mapped.)
    fn read_xinput(get: GetState) -> Option<Pad> {
        (0..4u32).find_map(|slot| {
            let mut x = XState::default();
            (unsafe { get(slot, &mut x) } == 0).then(|| from_xinput(x.buttons, x.lt, x.rt, x.lx, x.ly, x.rx, x.ry))
        })
    }

    fn key(vk: u16, up: bool) -> INPUT {
        let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) };
        let mut flags = KEYEVENTF_SCANCODE;
        if scan & 0xFF00 == 0xE000 { flags |= KEYEVENTF_EXTENDEDKEY; }
        if up { flags |= KEYEVENTF_KEYUP; }
        INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: (scan & 0xFF) as u16, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
    }
    fn mouse(dx: i32, dy: i32, data: i32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
        INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: data as _, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
    }
    fn send(events: &[Event]) {
        if events.is_empty() { return; }
        let inputs: Vec<INPUT> = events.iter().map(|e| match *e {
            Event::Down(Out::Key(k)) => key(k, false), Event::Up(Out::Key(k)) => key(k, true),
            Event::Down(Out::Mouse(b)) | Event::Up(Out::Mouse(b)) => {
                let down = matches!(e, Event::Down(_));
                let f = match (b, down) {
                    (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN, (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
                    (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN, (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                    (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN, (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
                };
                mouse(0, 0, 0, f)
            }
            Event::Move(dx, dy) => mouse(dx, dy, 0, MOUSEEVENTF_MOVE),
            Event::Wheel(n) => mouse(0, 0, n * 120, MOUSEEVENTF_WHEEL),
        }).collect();
        // A game with anti-cheat, or one running as administrator, may refuse injected input; the count shows that.
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            let mut s = shared();
            if s.error.is_empty() { s.error = "Windows did not accept the keys. A game that runs as administrator, or has anti-cheat, can block them.".into(); crate::diag::event("layouts", &s.error); }
        }
    }
}
#[cfg(windows)]
pub use imp::{begin, end, pause, set_enabled, shutdown, status};

#[cfg(not(windows))]
pub fn set_enabled(on: bool) -> Result<(), String> { if on { Err("Controller layouts need Windows".into()) } else { Ok(()) } }
#[cfg(not(windows))]
pub fn begin(_: &str, _: &str, _: Option<Controls>, _: Box<dyn FnMut(u32) -> bool + Send>) {}
#[cfg(not(windows))]
pub fn end(_: &str) {}
#[cfg(not(windows))]
pub fn pause() {}
#[cfg(not(windows))]
pub fn shutdown() {}
/// (listening, what Settings shows, the error if any).
#[cfg(not(windows))]
pub fn status() -> (bool, String, String) { (false, "Off".into(), String::new()) }

#[cfg(test)]
mod tests {
    use super::*;
    const W: u16 = 0x57;
    fn layout(buttons: &[(&str, &str)], left: &str, right: &str) -> Controls {
        Controls { preset: "custom".into(), buttons: buttons.iter().map(|(b, k)| (b.to_string(), k.to_string())).collect(), left_stick: left.into(), right_stick: right.into(), mouse_speed: 5 }
    }
    /// A neutral report: sticks centred, D-pad released (hat 8), with `edit` applied.
    fn report(len: usize, id: u8, sticks: usize, face: usize, edit: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut r = vec![0u8; len]; r[0] = id;
        for i in 0..4 { r[sticks + i] = 128; }
        r[face] = 0x08; edit(&mut r); r
    }

    #[test] fn a_dualsense_over_usb_is_read_in_full() {
        let r = report(64, 0x01, 1, 8, |r| { r[1] = 255; r[2] = 0; r[3] = 0; r[4] = 255; r[5] = 255; r[6] = 40; r[8] = 0x20 | 0x02; r[9] = 0x01 | 0x20 | 0x80; r[10] = 0x01; });
        let p = parse_sony(Sony::DualSense, &r).unwrap();
        assert!(p.down("cross") && p.down("right") && !p.down("up") && p.down("l1") && p.down("options") && p.down("r3"));
        assert!(p.down("l2") && !p.down("r2"), "L2 fully in, R2 barely touched");
        assert!(p.lx > 0.99 && p.ly < -0.99 && p.rx < -0.99 && p.ry > 0.99, "{p:?}");
        assert!(!p.down("circle") && !p.down("square") && !p.down("triangle") && !p.down("create"), "the PS bit is not a mapped button");
    }
    #[test] fn a_dualsense_over_bluetooth_is_read_in_both_modes() {
        let full = report(78, 0x31, 2, 9, |r| { r[2] = 0; r[9] = 0x06 | 0x80; r[10] = 0x02 | 0x10; r[7] = 200; });
        let p = parse_sony(Sony::DualSense, &full).unwrap();
        assert!(p.down("left") && p.down("triangle") && p.down("r1") && p.down("create") && p.down("r2") && p.lx < -0.99);
        let simple = report(10, 0x01, 1, 5, |r| { r[5] = 0x07 | 0x10; r[6] = 0x40; r[8] = 255; });
        let p = parse_sony(Sony::DualSense, &simple).unwrap();
        assert!(p.down("left") && p.down("up") && p.down("square") && p.down("l3") && p.down("l2"));
    }
    #[test] fn a_dualshock_4_is_read_over_usb_and_bluetooth() {
        let usb = report(64, 0x01, 1, 5, |r| { r[5] = 0x04 | 0x40; r[6] = 0x02; });
        let p = parse_sony(Sony::DualShock4, &usb).unwrap();
        assert!(p.down("down") && p.down("circle") && p.down("r1"));
        let bt = report(78, 0x11, 3, 7, |r| { r[7] = 0x08 | 0x20; r[11] = 255; r[3] = 255; });
        let p = parse_sony(Sony::DualShock4, &bt).unwrap();
        assert!(p.down("cross") && p.down("r2") && p.lx > 0.99 && !p.down("up"));
    }
    #[test] fn unknown_or_short_reports_are_ignored() {
        assert_eq!(parse_sony(Sony::DualSense, &[]), None);
        assert_eq!(parse_sony(Sony::DualSense, &[0x31, 0, 0]), None);
        assert_eq!(parse_sony(Sony::DualSense, &[0x05; 20]), None);
        assert_eq!(parse_sony(Sony::DualShock4, &[0x11, 0, 0, 0]), None);
    }
    #[test] fn a_centred_stick_reads_as_almost_nothing() {
        let p = parse_sony(Sony::DualSense, &report(64, 0x01, 1, 8, |_| {})).unwrap();
        assert_eq!(p.buttons, 0);
        assert!(p.lx.abs() < 0.01 && shape(p.lx, p.ly) == (0.0, 0.0));
    }
    #[test] fn xbox_controllers_map_to_the_same_buttons_with_y_turned_over() {
        let p = from_xinput(0x1000 | 0x0010 | 0x0001 | 0x0200, 255, 0, 0, 32767, -32768, 0);
        assert!(p.down("cross") && p.down("options") && p.down("up") && p.down("r1") && p.down("l2") && !p.down("r2"));
        assert!(p.ly < -0.99, "stick pushed up reads as up (negative y)");
        assert!(p.rx <= -1.0);
    }
    #[test] fn the_dead_zone_hides_drift_and_the_curve_keeps_small_pushes_fine() {
        assert_eq!(shape(0.1, 0.1), (0.0, 0.0), "drift inside the dead zone");
        let (small, _) = shape(0.3, 0.0); let (half, _) = shape(0.6, 0.0); let (full, _) = shape(1.0, 0.0);
        assert!(small > 0.0 && small < 0.05, "{small}");
        assert!(half > small * 4.0 && half < 0.5, "{half}");
        assert!((full - 1.0).abs() < 1e-6);
        let (x, y) = shape(1.0, 1.0); assert!((x.hypot(y) - 1.0).abs() < 1e-5, "a diagonal is not faster than straight");
    }
    #[test] fn key_codes_from_the_interface_become_windows_keys() {
        assert_eq!(vk("KeyW"), Some(W)); assert_eq!(vk("Digit1"), Some(0x31)); assert_eq!(vk("Space"), Some(0x20));
        assert_eq!(vk("ControlLeft"), Some(0xA2)); assert_eq!(vk("Escape"), Some(0x1B)); assert_eq!(vk("F12"), Some(0x7B));
        assert_eq!(vk("ArrowLeft"), Some(0x25)); assert_eq!(vk("Numpad5"), Some(0x65));
        for bad in ["", "Keyw", "KeyWW", "F13", "F0", "MetaLeft", "Power", "Digit10"] { assert_eq!(vk(bad), None, "{bad}"); }
    }
    #[test] fn buttons_press_and_release_their_keys() {
        let mut e = Engine::new(&layout(&[("cross", "Space"), ("r2", "MouseLeft"), ("triangle", "NoSuchKey"), ("ps", "KeyX")], "", ""));
        let cross = Pad { buttons: bit("cross"), ..Pad::default() };
        assert_eq!(e.step(&cross, 0.01), vec![Event::Down(Out::Key(0x20))]);
        assert_eq!(e.step(&cross, 0.01), vec![], "held is not pressed again");
        let both = Pad { buttons: bit("cross"), r2: 1.0, ..Pad::default() }.with_triggers();
        assert_eq!(e.step(&both, 0.01), vec![Event::Down(Out::Mouse(MouseButton::Left))]);
        assert_eq!(e.step(&Pad { buttons: bit("triangle"), ..Pad::default() }, 0.01), vec![Event::Up(Out::Key(0x20)), Event::Up(Out::Mouse(MouseButton::Left))], "unknown keys and buttons are ignored");
    }
    #[test] fn the_wheel_buttons_scroll_once_per_press() {
        let mut e = Engine::new(&layout(&[("r1", "WheelDown")], "", ""));
        let r1 = Pad { buttons: bit("r1"), ..Pad::default() };
        assert_eq!(e.step(&r1, 0.01), vec![Event::Wheel(-1)]);
        assert_eq!(e.step(&r1, 0.01), vec![]);
        e.step(&Pad::default(), 0.01);
        assert_eq!(e.step(&r1, 0.01), vec![Event::Wheel(-1)]);
    }
    #[test] fn a_stick_as_keys_presses_wasd_with_a_little_give() {
        let mut e = Engine::new(&layout(&[], "wasd", ""));
        let push = |x: f32, y: f32| Pad { lx: x, ly: y, ..Pad::default() };
        assert_eq!(e.step(&push(0.0, -0.9), 0.01), vec![Event::Down(Out::Key(W))]);
        assert_eq!(e.step(&push(0.0, -0.4), 0.01), vec![], "still held between the two thresholds");
        assert_eq!(e.step(&push(0.0, -0.3), 0.01), vec![Event::Up(Out::Key(W))]);
        assert_eq!(e.step(&push(0.0, -0.4), 0.01), vec![], "and not pressed again until pushed past the start");
        let diag = e.step(&push(0.8, 0.8), 0.01);
        assert_eq!(diag, vec![Event::Down(Out::Key(0x44)), Event::Down(Out::Key(0x53))], "a diagonal holds two keys");
    }
    #[test] fn the_same_key_from_two_places_is_pressed_once() {
        let mut e = Engine::new(&layout(&[("up", "ArrowUp")], "arrows", ""));
        let both = Pad { buttons: bit("up"), ly: -1.0, ..Pad::default() };
        assert_eq!(e.step(&both, 0.01), vec![Event::Down(Out::Key(0x26))]);
        assert_eq!(e.step(&Pad { ly: -1.0, ..Pad::default() }, 0.01), vec![], "the stick still holds it");
    }
    #[test] fn the_right_stick_moves_the_mouse_and_slow_pushes_add_up() {
        let mut e = Engine::new(&layout(&[], "", "mouse"));
        let full = Pad { rx: 1.0, ..Pad::default() };
        assert_eq!(e.step(&full, 0.01), vec![Event::Move(15, 0)], "1550 px/s at speed 5, for 10 ms");
        let slow = Pad { rx: 0.3, ..Pad::default() };
        let moved: i32 = (0..100).flat_map(|_| e.step(&slow, 0.01)).map(|ev| if let Event::Move(dx, _) = ev { dx } else { 0 }).sum();
        assert!(moved > 20 && moved < 70, "a gentle push still moves, slowly: {moved}");
        assert_eq!(e.step(&Pad { rx: 1.0, ..Pad::default() }, 5.0), vec![Event::Move(155, 0)], "a long stall is one small step");
        assert_eq!(e.step(&Pad { ry: -1.0, ..Pad::default() }, 0.01), vec![Event::Move(0, -15)], "up is up");
    }
    #[test] fn a_scroll_stick_scrolls_and_the_mouse_speed_is_kept_in_range() {
        let mut e = Engine::new(&layout(&[], "", "scroll"));
        let up = Pad { ry: -1.0, ..Pad::default() };
        let notches: i32 = (0..50).flat_map(|_| e.step(&up, 0.01)).map(|ev| if let Event::Wheel(n) = ev { n } else { 0 }).sum();
        assert_eq!(notches, 5, "half a second at full push");
        assert_eq!(mouse_px_per_sec(0), mouse_px_per_sec(5));
        assert_eq!(mouse_px_per_sec(99), mouse_px_per_sec(10));
    }
    #[test] fn stopping_lets_go_of_everything() {
        let mut e = Engine::new(&layout(&[("cross", "Space"), ("l2", "MouseRight")], "wasd", ""));
        e.step(&Pad { buttons: bit("cross"), l2: 1.0, lx: 1.0, ..Pad::default() }.with_triggers(), 0.01);
        assert!(e.holding());
        let mut up = e.release_all(); up.sort_by_key(|e| format!("{e:?}"));
        assert_eq!(up, vec![Event::Up(Out::Key(0x20)), Event::Up(Out::Key(0x44)), Event::Up(Out::Mouse(MouseButton::Right))]);
        assert!(!e.holding() && e.release_all().is_empty());
    }
    #[test] fn a_button_held_over_from_aether_hub_does_nothing_until_pressed_again() {
        let mut e = Engine::new(&layout(&[("cross", "Space"), ("r1", "WheelUp")], "", ""));
        let held = Pad { buttons: bit("cross") | bit("r1"), ..Pad::default() };
        e.settle(&held);
        assert_eq!(e.step(&held, 0.01), vec![]);
        assert_eq!(e.step(&Pad::default(), 0.01), vec![]);
        assert_eq!(e.step(&held, 0.01), vec![Event::Wheel(1), Event::Down(Out::Key(0x20))]);
    }
    #[test] fn the_status_says_what_is_happening() {
        assert_eq!(describe(false, "", Some("Hades"), true), "Off");
        assert!(describe(true, "", None, false).starts_with("Ready"));
        assert!(describe(true, "", Some("Hades"), true).contains("Working") && describe(true, "", Some("Hades"), false).contains("not in front"));
        assert!(describe(true, "blocked", Some("Hades"), true).contains("blocked"));
    }
    #[test] fn a_layout_saved_by_the_interface_loads() {
        let c: Controls = serde_json::from_str(r#"{"preset":"shooter","buttons":{"cross":"Space"},"leftStick":"wasd","rightStick":"mouse","mouseSpeed":7}"#).unwrap();
        assert_eq!((c.buttons["cross"].as_str(), c.left_stick.as_str(), c.mouse_speed), ("Space", "wasd", 7));
        assert_eq!(serde_json::from_str::<Controls>("{}").unwrap(), Controls::default());
    }
}
