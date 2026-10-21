mod commands;
mod diag;
mod dragmon;
mod engines;
mod error;
mod ffmpeg;
mod formats;
mod jobs;
mod matrix;
mod picker;
mod settings;
mod tray;
mod wheel;

use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let log_dir = app.path().app_log_dir()?;
            diag::init(log_dir.join("exploit.log"));

            app.manage(settings::SettingsStore::load(config_dir.join("settings.json")));
            app.manage(jobs::JobQueue::start(app.handle().clone()));
            tray::install(app.handle())?;
            wheel::create(app.handle())?;
            picker::register_shortcuts(app.handle());
            dragmon::start(app.handle().clone());

            // Login launches stay quietly in the tray.
            if std::env::args().any(|arg| arg == "--minimized") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app alive in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::probe_files,
            commands::available_targets,
            commands::convert_files,
            commands::list_jobs,
            commands::cancel_job,
            commands::cancel_all_jobs,
            commands::dismiss_finished,
            commands::get_settings,
            commands::update_settings,
            commands::get_launch_at_login,
            commands::set_launch_at_login,
            commands::ffmpeg_health,
            commands::open_output,
            commands::reveal_output,
            commands::wheel_hide,
            commands::show_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
