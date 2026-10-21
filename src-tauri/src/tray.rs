use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

use crate::commands;
use crate::diag;
use crate::picker;
use crate::wheel::Mode;

pub fn show_main(app: &AppHandle) {
    let app = app.clone();
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .cloned();
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || open_settings(&handle, config.as_ref())) {
        diag::log(&format!("settings: {error}"));
    }
}

fn open_settings(app: &AppHandle, config: Option<&tauri::utils::config::WindowConfig>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    let Some(config) = config else {
        diag::log("settings: window config is missing");
        return;
    };
    match WebviewWindowBuilder::from_config(app, config).and_then(|builder| builder.build()) {
        Ok(window) => {
            let _ = window.show();
            let _ = window.set_focus();
        }
        Err(error) => diag::log(&format!("settings: {error}")),
    }
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let convert = MenuItem::with_id(app, "convert", "Convert Files…", true, Some("Ctrl+Alt+O"))?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let logs = MenuItem::with_id(app, "logs", "Show Logs", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit FileForge", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&convert, &settings, &logs, &separator, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("FileForge")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "convert" => picker::begin(app, Mode::Convert),
            "settings" => show_main(app),
            "logs" => {
                if let Err(error) = commands::open_logs() {
                    diag::log(&format!("show logs: {error}"));
                }
            }
            "quit" => app.exit(0),
            _ => {}
        }); 

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
