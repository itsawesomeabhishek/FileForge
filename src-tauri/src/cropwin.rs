//! The crop editor window. The wheel asks for it; the page draws the area.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::diag;

pub const LABEL: &str = "crop";

static TARGET: Mutex<Option<String>> = Mutex::new(None);

pub fn current() -> Option<String> {
    TARGET.lock().ok().and_then(|guard| guard.clone())
}

pub fn open(app: &AppHandle, path: String) {
    if let Ok(mut guard) = TARGET.lock() {
        *guard = Some(path.clone());
    }
    let app = app.clone();
    if let Err(error) = app.clone().run_on_main_thread(move || show(&app, path)) {
        diag::log(&format!("crop: {error}"));
    }
}

fn show(app: &AppHandle, path: String) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.emit("crop-open", &path);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    match WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Crop")
        .inner_size(980.0, 720.0)
        .min_inner_size(720.0, 520.0)
        .center()
        .build()
    {
        Ok(window) => {
            let _ = window.emit("crop-open", &path);
            let _ = window.set_focus();
        }
        Err(error) => diag::log(&format!("crop: {error}")),
    }
}
