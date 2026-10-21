//! The radial wheel overlay: a transparent,
//! always-on-top window that is moved over the monitor under the cursor
//! while a drag (or the file picker) is active.
//!
//! The window covers the whole monitor so that a drop anywhere lands on it
//! (and is cancelled) instead of falling through to the app underneath,
//! like the full-screen panel on macOS. The React side draws the wheel.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::diag;

pub const LABEL: &str = "wheel";

/// Radius of the wheel's disc in CSS pixels (matches the macOS view).
const DISC_RADIUS: f64 = 170.0;
/// Keeps the disc fully on screen when the cursor is near an edge.
const EDGE_MARGIN: f64 = DISC_RADIUS + 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Convert,
    Tools,
    /// File actions (copy, reveal, duplicate). Not bound to a key yet.
    #[allow(dead_code)]
    Actions,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowPayload {
    mode: Mode,
    /// Cursor and monitor size in physical pixels, relative to the overlay.
    /// The page divides by its own scale so the wheel and the drop match.
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    /// True when opened from the menu/hotkey (mouse and keyboard driven)
    /// rather than by an active drag.
    picker: bool,
    paths: Vec<String>,
}

struct MonitorBox {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

static VISIBLE: AtomicBool = AtomicBool::new(false);
static PILL: AtomicBool = AtomicBool::new(false);
static CREATING: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
/// One mouse-move prewarm is enough until the window is destroyed.
static NUDGED: AtomicBool = AtomicBool::new(false);
/// Bumped whenever the overlay is shown or asked to stay, so a delayed
/// release does not destroy a window that is in use again.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Separate from GENERATION. Hiding the wheel must not cancel the timer
/// that removes the Done card.
static PILL_EPOCH: AtomicU64 = AtomicU64::new(0);
/// How long a hidden, idle overlay is kept. Long enough that the next
/// drag is instant, then WebView2 is released.
const RELEASE_AFTER: Duration = Duration::from_secs(180);
const PILL_HOLD: Duration = Duration::from_millis(3200);

struct PendingShow {
    mode: Mode,
    cursor: (i32, i32),
    picker: bool,
    paths: Vec<String>,
}

static PENDING: Mutex<Option<PendingShow>> = Mutex::new(None);

pub fn is_visible() -> bool {
    VISIBLE.load(Ordering::SeqCst)
}

pub fn is_ready() -> bool {
    READY.load(Ordering::SeqCst)
}

/// True once, while the overlay does not exist yet. The drag hook uses this
/// to start WebView2 on the first mouse move, before the click.
pub fn claim_nudge() -> bool {
    if READY.load(Ordering::SeqCst) || CREATING.load(Ordering::SeqCst) {
        return false;
    }
    !NUDGED.swap(true, Ordering::SeqCst)
}

/// Builds the hidden overlay at startup so showing it later is instant.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    // Built "visible" far off screen and hidden right after: WebView2 only
    // creates the child windows that receive file drops once its window has
    // been shown, and wry registers its drop targets on those children.
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("FileForge wheel")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .focused(false)
        .inner_size(400.0, 300.0)
        .position(-32000.0, -32000.0)
        .visible(true)
        .build()?;
    let _ = window.hide();
    // Parked on the primary monitor at its real size while hidden, so the
    // first show does not resize the drop target under the cursor.
    if let Some(monitor) = primary_monitor(app) {
        place(&window, &monitor);
    }
    READY.store(true, Ordering::SeqCst);
    Ok(())
}

fn primary_monitor(app: &AppHandle) -> Option<MonitorBox> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let pos = *monitor.position();
    let size = *monitor.size();
    Some(MonitorBox {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    })
}

fn monitor_at(app: &AppHandle, cursor: (i32, i32)) -> Option<MonitorBox> {
    let found = app.available_monitors().ok().and_then(|monitors| {
        monitors.into_iter().find(|monitor| {
            let pos = monitor.position();
            let size = monitor.size();
            cursor.0 >= pos.x
                && cursor.0 < pos.x + size.width as i32
                && cursor.1 >= pos.y
                && cursor.1 < pos.y + size.height as i32
        })
    });
    let monitor = found.or_else(|| app.primary_monitor().ok().flatten())?;
    let pos = *monitor.position();
    let size = *monitor.size();
    Some(MonitorBox {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    })
}

fn place(window: &WebviewWindow, monitor: &MonitorBox) {
    let _ = window.set_position(PhysicalPosition::new(monitor.x, monitor.y));
    let _ = window.set_size(PhysicalSize::new(monitor.width, monitor.height));
}

/// Sizes the hidden overlay to the monitor under the cursor as soon as the
/// button goes down. The progress card uses a small window; leaving it that
/// size made the next drop's coordinates miss every wedge.
pub fn prepare_for_drag(app: &AppHandle, cursor: (i32, i32)) {
    if VISIBLE.load(Ordering::SeqCst) && !PILL.load(Ordering::SeqCst) {
        return;
    }
    let Some(window) = window(app) else {
        prepare(app);
        return;
    };
    if PILL.swap(false, Ordering::SeqCst) {
        let _ = window.emit("wheel-hide", ());
        let _ = window.hide();
        VISIBLE.store(false, Ordering::SeqCst);
    }
    if let Some(monitor) = monitor_at(app, cursor) {
        place(&window, &monitor);
    }
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Where the wheel centre goes inside a monitor of `width` x `height`
/// (CSS pixels) for a cursor at `(x, y)`.
pub fn clamp_center(x: f64, y: f64, width: f64, height: f64) -> (f64, f64) {
    let clamp = |value: f64, extent: f64| {
        if extent <= EDGE_MARGIN * 2.0 {
            extent / 2.0
        } else {
            value.clamp(EDGE_MARGIN, extent - EDGE_MARGIN)
        }
    };
    (clamp(x, width), clamp(y, height))
}

/// Starts the overlay as soon as a drag might become the wheel, so the
/// window exists before the mouse is released. Creating it only after the
/// drag threshold meant the first drop finished before WebView2 was ready.
pub fn prepare(app: &AppHandle) {
    if window(app).is_some() || CREATING.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if window(&handle).is_none() {
            if let Err(error) = create(&handle) {
                diag::log(&format!("wheel: create failed: {error}"));
                CREATING.store(false, Ordering::SeqCst);
                NUDGED.store(false, Ordering::SeqCst);
                return;
            }
        }
        CREATING.store(false, Ordering::SeqCst);
        if let Some(pending) = PENDING.lock().ok().and_then(|mut slot| slot.take()) {
            let cursor = if pending.picker {
                pending.cursor
            } else {
                crate::dragmon::cursor_position().unwrap_or(pending.cursor)
            };
            present(&handle, pending.mode, cursor, pending.picker, pending.paths);
        } else if PILL.load(Ordering::SeqCst) {
            show_pill_now(&handle);
        }
    }) {
        CREATING.store(false, Ordering::SeqCst);
        diag::log(&format!("wheel: {error}"));
    }
}

/// Shows the wheel over the monitor containing `cursor` (physical pixels).
pub fn show(app: &AppHandle, mode: Mode, cursor: (i32, i32), picker: bool, paths: Vec<String>) {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    let pending = PendingShow {
        mode,
        cursor,
        picker,
        paths,
    };
    let ready = window(app).is_some() && !CREATING.load(Ordering::SeqCst);
    if let Ok(mut slot) = PENDING.lock() {
        *slot = Some(pending);
        if ready {
            if let Some(pending) = slot.take() {
                drop(slot);
                let cursor = if pending.picker {
                    pending.cursor
                } else {
                    crate::dragmon::cursor_position().unwrap_or(pending.cursor)
                };
                present(app, pending.mode, cursor, pending.picker, pending.paths);
            }
            return;
        }
    }
    prepare(app);
}

/// Shows or hides the small conversion card. `hold` keeps the card up
/// after the job finishes so the result can be read, then removes it.
/// The timer is armed even while the wheel is still open, because a fast
/// image job finishes before the mouse-up hide.
pub fn sync_progress(app: &AppHandle, show: bool, hold: bool) {
    if show {
        PILL.store(true, Ordering::SeqCst);
        let epoch = PILL_EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
        if !VISIBLE.load(Ordering::SeqCst) {
            if window(app).is_some() && !CREATING.load(Ordering::SeqCst) {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || show_pill_now(&handle));
            } else {
                prepare(app);
            }
        }
        if hold {
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(PILL_HOLD);
                if PILL_EPOCH.load(Ordering::SeqCst) != epoch {
                    return;
                }
                sync_progress(&app, false, false);
            });
        }
    } else if PILL.swap(false, Ordering::SeqCst) && !VISIBLE.load(Ordering::SeqCst) {
        conceal(app);
    }
}

fn present(app: &AppHandle, mode: Mode, cursor: (i32, i32), picker: bool, paths: Vec<String>) {
    let Some(window) = window(app) else {
        diag::log("wheel: overlay window is missing");
        return;
    };
    let Some(monitor) = monitor_at(app, cursor) else {
        diag::log("wheel: no monitor found for the cursor");
        return;
    };

    let _ = window.set_ignore_cursor_events(false);
    let _ = window.set_focusable(picker);
    place(&window, &monitor);
    let _ = window.set_always_on_top(true);
    // Physical pixels. The page converts them with the webview scale and
    // clamps the disc on screen, so a drop uses the same point as the wedge.
    let _ = window.emit(
        "wheel-show",
        ShowPayload {
            mode,
            x: (cursor.0 - monitor.x) as f64,
            y: (cursor.1 - monitor.y) as f64,
            width: monitor.width as f64,
            height: monitor.height as f64,
            picker,
            paths,
        },
    );
    if let Err(error) = window.show() {
        diag::log(&format!("wheel: show failed: {error}"));
        return;
    }
    if picker {
        let _ = window.set_focus();
    }
    VISIBLE.store(true, Ordering::SeqCst);
}

pub fn set_mode(app: &AppHandle, mode: Mode) {
    if let Some(window) = window(app) {
        let _ = window.emit("wheel-mode", mode);
    }
}

pub fn hide(app: &AppHandle) {
    if let Ok(mut slot) = PENDING.lock() {
        *slot = None;
    }
    conceal(app);
    if PILL.load(Ordering::SeqCst) && !VISIBLE.load(Ordering::SeqCst) {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || show_pill_now(&handle));
    }
}

fn conceal(app: &AppHandle) {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(window) = window(app) {
        let _ = window.emit("wheel-hide", ());
        let _ = window.hide();
        let _ = window.set_focusable(false);
        let _ = window.set_ignore_cursor_events(false);
    }
    VISIBLE.store(false, Ordering::SeqCst);
    schedule_release(app, generation);
}

fn schedule_release(app: &AppHandle, generation: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(RELEASE_AFTER);
        if GENERATION.load(Ordering::SeqCst) != generation
            || VISIBLE.load(Ordering::SeqCst)
            || PILL.load(Ordering::SeqCst)
        {
            return;
        }
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if GENERATION.load(Ordering::SeqCst) != generation
                || VISIBLE.load(Ordering::SeqCst)
                || PILL.load(Ordering::SeqCst)
            {
                return;
            }
            if let Some(window) = window(&handle) {
                READY.store(false, Ordering::SeqCst);
                NUDGED.store(false, Ordering::SeqCst);
                let _ = window.destroy();
            }
        });
    });
}

fn show_pill_now(app: &AppHandle) {
    if VISIBLE.load(Ordering::SeqCst) {
        return;
    }
    let Some(window) = window(app) else {
        return;
    };
    let monitor = app.primary_monitor().ok().flatten();
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let pos = *monitor.position();
    let size = *monitor.size();
    let width = (400.0 * scale).round() as u32;
    let height = (96.0 * scale).round() as u32;
    let x = pos.x + (size.width as i32 - width as i32) / 2;
    let y = pos.y + size.height as i32 - height as i32 - (32.0 * scale).round() as i32;

    let _ = window.set_ignore_cursor_events(true);
    let _ = window.set_focusable(false);
    let _ = window.set_always_on_top(true);
    let _ = window.set_size(PhysicalSize::new(width, height));
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.emit("wheel-pill", ());
    if let Err(error) = window.show() {
        diag::log(&format!("wheel: progress pill failed: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_centre_inside_the_margin() {
        let (x, y) = clamp_center(5.0, 5.0, 1920.0, 1080.0);
        assert_eq!((x, y), (EDGE_MARGIN, EDGE_MARGIN));
        let (x, y) = clamp_center(1919.0, 1079.0, 1920.0, 1080.0);
        assert_eq!((x, y), (1920.0 - EDGE_MARGIN, 1080.0 - EDGE_MARGIN));
    }

    #[test]
    fn leaves_interior_points_alone() {
        assert_eq!(clamp_center(800.0, 500.0, 1920.0, 1080.0), (800.0, 500.0));
    }

    #[test]
    fn centres_on_tiny_screens() {
        assert_eq!(clamp_center(10.0, 10.0, 300.0, 300.0), (150.0, 150.0));
    }

    #[test]
    fn mode_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Mode::Convert).unwrap(), "\"convert\"");
    }
}
