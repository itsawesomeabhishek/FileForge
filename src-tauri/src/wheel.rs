//! The radial wheel overlay: one pre-created, hidden, transparent,
//! always-on-top window that is moved over the monitor under the cursor
//! while a drag (or the file picker) is active.
//!
//! The window covers the whole monitor so that a drop anywhere lands on it
//! (and is cancelled) instead of falling through to the app underneath,
//! like the full-screen panel on macOS. The React side draws the wheel.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    Actions,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowPayload {
    mode: Mode,
    /// Wheel centre in CSS pixels, relative to the overlay window.
    x: f64,
    y: f64,
    /// True when opened from the menu/hotkey (mouse and keyboard driven)
    /// rather than by an active drag.
    picker: bool,
    paths: Vec<String>,
}

static VISIBLE: AtomicBool = AtomicBool::new(false);
/// Bumped on every show and hide so a late create or destroy cannot
/// revive a window the user has already dismissed.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// How long the hidden wheel stays allocated so an in-flight drop can
/// still reach the page. Then the webview is destroyed and WebView2 exits.
const RELEASE_AFTER: Duration = Duration::from_millis(1500);

pub fn is_visible() -> bool {
    VISIBLE.load(Ordering::SeqCst)
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
    Ok(())
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

/// Shows the wheel over the monitor containing `cursor` (physical pixels).
///
/// The overlay is created on demand and destroyed after it closes, so
/// WebView2 is not resident while FileForge sits in the tray. The first
/// show after idle waits for that startup.
pub fn show(app: &AppHandle, mode: Mode, cursor: (i32, i32), picker: bool, paths: Vec<String>) {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    if window(app).is_some() {
        present(app, mode, cursor, picker, paths);
        return;
    }

    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        if window(&handle).is_none() {
            if let Err(error) = create(&handle) {
                diag::log(&format!("wheel: create failed: {error}"));
                return;
            }
        }
        if GENERATION.load(Ordering::SeqCst) != generation {
            if let Some(window) = window(&handle) {
                let _ = window.destroy();
            }
            return;
        }
        let cursor = if picker {
            cursor
        } else {
            crate::dragmon::cursor_position().unwrap_or(cursor)
        };
        present(&handle, mode, cursor, picker, paths);
    }) {
        diag::log(&format!("wheel: {error}"));
    }
}

fn present(app: &AppHandle, mode: Mode, cursor: (i32, i32), picker: bool, paths: Vec<String>) {
    let Some(window) = window(app) else {
        diag::log("wheel: overlay window is missing");
        return;
    };
    let monitor = app
        .available_monitors()
        .ok()
        .and_then(|monitors| {
            monitors.into_iter().find(|monitor| {
                let pos = monitor.position();
                let size = monitor.size();
                cursor.0 >= pos.x
                    && cursor.0 < pos.x + size.width as i32
                    && cursor.1 >= pos.y
                    && cursor.1 < pos.y + size.height as i32
            })
        })
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        diag::log("wheel: no monitor found for the cursor");
        return;
    };

    let scale = monitor.scale_factor();
    let pos = *monitor.position();
    let size = *monitor.size();
    let (cx, cy) = clamp_center(
        (cursor.0 - pos.x) as f64 / scale,
        (cursor.1 - pos.y) as f64 / scale,
        size.width as f64 / scale,
        size.height as f64 / scale,
    );

    let _ = window.set_focusable(picker);
    let _ = window.set_position(PhysicalPosition::new(pos.x, pos.y));
    let _ = window.set_size(PhysicalSize::new(size.width, size.height));
    let _ = window.set_always_on_top(true);
    // Tell the page first so it can reset before it becomes visible.
    let _ = window.emit(
        "wheel-show",
        ShowPayload {
            mode,
            x: cx,
            y: cy,
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
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(window) = window(app) {
        let _ = window.emit("wheel-hide", ());
        let _ = window.hide();
        let _ = window.set_focusable(false);
    }
    VISIBLE.store(false, Ordering::SeqCst);

    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(RELEASE_AFTER);
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            if let Some(window) = window(&handle) {
                let _ = window.destroy();
            }
        });
    });
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
