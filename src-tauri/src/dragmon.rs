//! Detects "modifier + drag from Explorer or the desktop" and opens the
//! wheel. Port of the macOS `DragMonitor`.
//!
//! A low-level mouse hook only forwards button down/up (it must return
//! quickly). A controller thread owns the state machine and polls the
//! cursor and modifier keys at ~60 Hz while a candidate drag is active, so
//! the modifier can change mid-drag.
//!
//! Windows has no global drag pasteboard, so the hook cannot know whether
//! files are really being dragged. The wheel window is shown once the
//! cursor has moved far enough with a modifier held; the page only draws
//! the wheel after the OS delivers a drag-enter carrying file paths, and a
//! rubber-band selection (no drag-enter) leaves nothing visible.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{CloseHandle, HINSTANCE, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSEEVENTF_LEFTDOWN, MOUSEINPUT, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LBUTTON, VK_LSHIFT,
    VK_MENU, VK_RSHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GA_ROOT, GetAncestor, GetClassNameW, GetCursorPos, GetMessageW,
    GetWindowThreadProcessId, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, SetWindowsHookExW,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WindowFromPoint,
};
use windows::core::PWSTR;

use crate::diag;
use crate::settings::SettingsStore;
use crate::wheel::{self, Mode};

/// Pixels the cursor must travel from the press point before the wheel
/// opens (the Finder fallback on macOS uses the same 24 pt).
const DRAG_THRESHOLD: i32 = 24;
const POLL_INTERVAL: Duration = Duration::from_millis(16);
/// How long the wheel lingers after release so the drop can be delivered.
const RELEASE_GRACE: Duration = Duration::from_millis(350);

/// Modes whose wheel has content. Tools and file actions are added to this
/// list as those phases land.
fn mode_enabled(mode: Mode) -> bool {
    matches!(mode, Mode::Convert)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// Shift = convert, Alt+Shift = tools, Ctrl+Shift = file actions.
pub fn mode_for(mods: Modifiers) -> Option<Mode> {
    match (mods.shift, mods.alt, mods.ctrl) {
        (true, false, false) => Some(Mode::Convert),
        (true, true, false) => Some(Mode::Tools),
        (true, false, true) => Some(Mode::Actions),
        _ => None,
    }
}

/// Explorer windows and the desktop (icons live under Progman or WorkerW).
pub fn is_shell_class(class: &str) -> bool {
    matches!(class, "CabinetWClass" | "Progman" | "WorkerW")
}

pub fn is_excluded(exe: &str, excluded: &[String]) -> bool {
    let exe = exe.to_lowercase();
    excluded.iter().any(|entry| *entry == exe)
}

pub fn exceeds_threshold(start: (i32, i32), now: (i32, i32)) -> bool {
    let dx = (now.0 - start.0) as i64;
    let dy = (now.1 - start.1) as i64;
    dx * dx + dy * dy >= (DRAG_THRESHOLD as i64) * (DRAG_THRESHOLD as i64)
}

enum MouseEvent {
    Down(i32, i32),
    Up,
}

static EVENTS: OnceLock<Sender<MouseEvent>> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Tags input this module injects so its own hooks can recognise it.
const MARKER: usize = 0x4558_504C;

/// Physical Shift state (bit 0 = left, bit 1 = right), tracked from the
/// keyboard hook. It is separate from the system key state because the
/// mouse hook briefly releases Shift for Explorer (see `hook_proc`).
static SHIFT_KEYS: AtomicU8 = AtomicU8::new(0);

fn shift_held() -> bool {
    SHIFT_KEYS.load(Ordering::SeqCst) != 0
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if info.dwExtraInfo != MARKER {
            let bit = match VIRTUAL_KEY(info.vkCode as u16) {
                VK_LSHIFT => 1,
                VK_RSHIFT => 2,
                _ => 0,
            };
            if bit != 0 {
                match wparam.0 as u32 {
                    WM_KEYDOWN | WM_SYSKEYDOWN => {
                        SHIFT_KEYS.fetch_or(bit, Ordering::SeqCst);
                    }
                    WM_KEYUP | WM_SYSKEYUP => {
                        SHIFT_KEYS.fetch_and(!bit, Ordering::SeqCst);
                    }
                    _ => {}
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Presses the left button as if Shift were not held. Explorer applies
/// Shift+click as a range selection at mouse-down, so a Shift-drag would
/// otherwise select every file between the last selection and the one
/// being dragged. Shift stays physically held; only the system's view of
/// it is cleared for this one click.
unsafe fn press_without_shift() -> bool {
    let key = |vk: VIRTUAL_KEY| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: MARKER,
            },
        },
    };
    let mut inputs: Vec<INPUT> = Vec::with_capacity(3);
    let keys = SHIFT_KEYS.load(Ordering::SeqCst);
    if keys & 1 != 0 {
        inputs.push(key(VK_LSHIFT));
    }
    if keys & 2 != 0 {
        inputs.push(key(VK_RSHIFT));
    }
    inputs.push(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_LEFTDOWN,
                time: 0,
                dwExtraInfo: MARKER,
            },
        },
    });
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    sent == inputs.len() as u32
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if let Some(sender) = EVENTS.get() {
            match wparam.0 as u32 {
                WM_LBUTTONDOWN => {
                    let point = (info.pt.x, info.pt.y);
                    let ours = info.dwExtraInfo == MARKER;
                    let plain_shift = shift_held() && !key_down(VK_CONTROL) && !key_down(VK_MENU);
                    if !ours && plain_shift && allowed_source(point) {
                        // Swallow the real press and replay it without Shift.
                        if unsafe { press_without_shift() } {
                            return LRESULT(1);
                        }
                    }
                    let _ = sender.send(MouseEvent::Down(point.0, point.1));
                }
                WM_LBUTTONUP => {
                    let _ = sender.send(MouseEvent::Up);
                }
                _ => {}
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Installs the hook and starts the controller. Failures are logged and
/// leave the rest of the app working (the picker wheel still opens).
pub fn start(app: AppHandle) {
    let (sender, receiver) = mpsc::channel();
    if EVENTS.set(sender).is_err() {
        return; // already running
    }
    let _ = APP.set(app.clone());

    std::thread::Builder::new()
        .name("fileforge-mouse-hook".into())
        .spawn(|| unsafe {
            let module = GetModuleHandleW(None).map(|m| HINSTANCE(m.0)).ok();
            // Start from the real Shift state in case it is already held.
            let mut keys = 0;
            if key_down(VK_LSHIFT) {
                keys |= 1;
            }
            if key_down(VK_RSHIFT) {
                keys |= 2;
            }
            SHIFT_KEYS.store(keys, Ordering::SeqCst);
            if let Err(error) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0) {
                diag::log(&format!("drag monitor: keyboard hook failed: {error}"));
            }
            match SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), module, 0) {
                Ok(_) => {
                    // The hook is serviced by this thread's message loop.
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {}
                }
                Err(error) => diag::log(&format!("drag monitor: hook failed: {error}")),
            }
        })
        .ok();

    std::thread::Builder::new()
        .name("fileforge-drag-controller".into())
        .spawn(move || controller(app, receiver))
        .ok();
}

enum State {
    Idle,
    /// Left button is down on an allowed source; waiting for movement and
    /// a modifier.
    Candidate { start: (i32, i32) },
    /// The wheel window is open.
    Active { start: (i32, i32), mode: Mode },
    /// Button released; the wheel is hidden after a short grace period.
    Releasing { until: Instant },
}

fn controller(app: AppHandle, events: Receiver<MouseEvent>) {
    let mut state = State::Idle;
    loop {
        let event = match state {
            State::Idle => match events.recv() {
                Ok(event) => Some(event),
                Err(_) => return,
            },
            _ => match events.recv_timeout(POLL_INTERVAL) {
                Ok(event) => Some(event),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            },
        };

        if let Some(event) = event {
            state = on_event(&app, state, event);
        }
        state = tick(&app, state);
    }
}

fn on_event(app: &AppHandle, state: State, event: MouseEvent) -> State {
    match event {
        MouseEvent::Down(x, y) => {
            if matches!(state, State::Active { .. } | State::Releasing { .. }) {
                wheel::hide(app);
            }
            if wheel::is_visible() {
                // Picker wheel is open; the page handles its own clicks.
                return State::Idle;
            }
            if allowed_source((x, y)) {
                State::Candidate { start: (x, y) }
            } else {
                State::Idle
            }
        }
        MouseEvent::Up => match state {
            State::Active { .. } => State::Releasing {
                until: Instant::now() + RELEASE_GRACE,
            },
            State::Candidate { .. } => State::Idle,
            other => other,
        },
    }
}

fn tick(app: &AppHandle, state: State) -> State {
    match state {
        State::Idle => State::Idle,
        State::Releasing { until } => {
            if Instant::now() >= until {
                wheel::hide(app);
                State::Idle
            } else {
                State::Releasing { until }
            }
        }
        State::Candidate { start } => {
            if !button_down() {
                return State::Idle;
            }
            let Some(cursor) = cursor_position() else {
                return State::Candidate { start };
            };
            match mode_for(read_modifiers()) {
                Some(mode) if mode_enabled(mode) && exceeds_threshold(start, cursor) => {
                    wheel::show(app, mode, cursor, false, Vec::new());
                    State::Active { start, mode }
                }
                _ => State::Candidate { start },
            }
        }
        State::Active { start, mode } => {
            if !button_down() {
                return State::Releasing {
                    until: Instant::now() + RELEASE_GRACE,
                };
            }
            match mode_for(read_modifiers()) {
                Some(next) if mode_enabled(next) => {
                    if next != mode {
                        wheel::set_mode(app, next);
                    }
                    State::Active { start, mode: next }
                }
                _ => {
                    // Modifier released mid-drag: the drag carries on as a
                    // normal Explorer drag.
                    wheel::hide(app);
                    State::Candidate { start }
                }
            }
        }
    }
}

fn allowed_source(point: (i32, i32)) -> bool {
    let Some(app) = APP.get() else {
        return false;
    };
    let Some(settings) = app.try_state::<SettingsStore>() else {
        return false;
    };
    let settings = settings.get();
    if !settings.wheel_enabled {
        return false;
    }
    let Some(root) = root_window(point) else {
        return false;
    };
    if !is_shell_class(&window_class(root)) {
        return false;
    }
    match process_name(root) {
        Some(exe) => !is_excluded(&exe, &settings.excluded_apps),
        None => true,
    }
}

type Hwnd = windows::Win32::Foundation::HWND;

fn root_window(point: (i32, i32)) -> Option<Hwnd> {
    unsafe {
        let hwnd = WindowFromPoint(POINT {
            x: point.0,
            y: point.1,
        });
        if hwnd.0.is_null() {
            return None;
        }
        let root = GetAncestor(hwnd, GA_ROOT);
        Some(if root.0.is_null() { hwnd } else { root })
    }
}

fn window_class(hwnd: Hwnd) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

/// Lower-case executable file name of the process owning `hwnd`.
fn process_name(hwnd: Hwnd) -> Option<String> {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 520];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        path.rsplit(['\\', '/']).next().map(str::to_lowercase)
    }
}

fn key_down(key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(key.0 as i32) as u16 & 0x8000) != 0 }
}

fn button_down() -> bool {
    key_down(VK_LBUTTON)
}

fn read_modifiers() -> Modifiers {
    Modifiers {
        shift: shift_held(),
        alt: key_down(VK_MENU),
        ctrl: key_down(VK_CONTROL),
    }
}

pub fn cursor_position() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    Some((point.x, point.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(shift: bool, alt: bool, ctrl: bool) -> Modifiers {
        Modifiers { shift, alt, ctrl }
    }

    #[test]
    fn maps_modifiers_to_modes() {
        assert_eq!(mode_for(mods(true, false, false)), Some(Mode::Convert));
        assert_eq!(mode_for(mods(true, true, false)), Some(Mode::Tools));
        assert_eq!(mode_for(mods(true, false, true)), Some(Mode::Actions));
    }

    #[test]
    fn other_combinations_open_nothing() {
        assert_eq!(mode_for(mods(false, false, false)), None);
        assert_eq!(mode_for(mods(false, true, false)), None);
        assert_eq!(mode_for(mods(false, false, true)), None);
        assert_eq!(mode_for(mods(true, true, true)), None);
    }

    #[test]
    fn recognises_shell_windows_only() {
        assert!(is_shell_class("CabinetWClass"));
        assert!(is_shell_class("Progman"));
        assert!(is_shell_class("WorkerW"));
        assert!(!is_shell_class("Shell_TrayWnd"));
        assert!(!is_shell_class("Chrome_WidgetWin_1"));
    }

    #[test]
    fn excluded_match_is_case_insensitive() {
        let list = vec!["explorer.exe".to_string()];
        assert!(is_excluded("Explorer.EXE", &list));
        assert!(!is_excluded("code.exe", &list));
    }

    #[test]
    fn threshold_uses_euclidean_distance() {
        assert!(!exceeds_threshold((0, 0), (10, 10)));
        assert!(exceeds_threshold((0, 0), (24, 0)));
        assert!(exceeds_threshold((100, 100), (80, 115)));
        assert!(!exceeds_threshold((100, 100), (100, 123)));
    }
}
