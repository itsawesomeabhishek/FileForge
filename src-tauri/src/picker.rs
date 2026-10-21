//! Menu/hotkey entry point: choose files, then pick an action from the
//! wheel at the cursor (the macOS "Convert Files" menu command).

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::diag;
use crate::dragmon;
use crate::wheel::{self, Mode};

/// Asks for files, then opens the wheel for `mode` at the cursor.
pub fn begin(app: &AppHandle, mode: Mode) {
    if wheel::is_visible() {
        wheel::hide(app);
        return;
    }
    wheel::prepare(app);
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(files) = app.dialog().file().blocking_pick_files() else {
            return;
        };
        let paths: Vec<String> = files
            .into_iter()
            .filter_map(|file| file.into_path().ok())
            .map(|path| path.display().to_string())
            .collect();
        if paths.is_empty() {
            return;
        }
        let cursor = dragmon::cursor_position().unwrap_or((0, 0));
        wheel::show(&app, mode, cursor, true, paths);
    });
}

/// Ctrl+Alt+O opens the convert wheel. Tools and file actions get their own
/// shortcuts when those wheels exist.
pub fn register_shortcuts(app: &AppHandle) {
    let convert = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyO);
    let result = app
        .global_shortcut()
        .on_shortcut(convert, |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                begin(app, Mode::Convert);
            }
        });
    if let Err(error) = result {
        diag::log(&format!("hotkey Ctrl+Alt+O not registered: {error}"));
    }
}
