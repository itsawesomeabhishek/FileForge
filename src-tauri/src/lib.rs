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

use tauri::Manager;
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// Puts FileForge in the Windows startup apps unless the user turned that off.
fn register_startup(app: &tauri::AppHandle, enabled: bool) {
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    if let Err(error) = result {
        diag::log(&format!("startup registration: {error}"));
    }
}

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
            diag::init(log_dir.join("fileforge.log"));

            let store = settings::SettingsStore::load(config_dir.join("settings.json"));
            register_startup(app.handle(), store.get().launch_at_login);
            app.manage(store);
            app.manage(jobs::JobQueue::start(app.handle().clone()));
            tray::install(app.handle())?;
            picker::register_shortcuts(app.handle());
            dragmon::start(app.handle().clone());
            // WebView2 has to exist before the first drop, or that drag
            // finishes while the overlay is still starting.
            wheel::prepare(app.handle());
            // The first image wheel asks which encoders exist. Do that
            // before the drag so the wedges are already listed.
            std::thread::spawn(ffmpeg::available_encoders);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::probe_files,
            commands::available_targets,
            commands::convert_files,
            commands::apply_file_tool,
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
        .build(tauri::generate_context!())
        .expect("error while building FileForge")
        .run(|_app, event| {
            // Closing Settings or the wheel must not quit the tray app.
            // Quit FileForge calls exit(), which sets an exit code.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
