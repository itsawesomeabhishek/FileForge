use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::commands;
use crate::diag;
use crate::picker;
use crate::wheel::Mode;

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let convert = MenuItem::with_id(app, "convert", "Convert Files…", true, Some("Ctrl+Alt+O"))?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let logs = MenuItem::with_id(app, "logs", "Show Logs", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit exploit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&convert, &settings, &logs, &separator, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("exploit")
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
