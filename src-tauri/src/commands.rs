use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::diag;
use crate::engines;
use crate::engines::tools::FileTool;
use crate::ffmpeg::{self, Health};
use crate::formats::{FileFormat, FormatCategory};
use crate::jobs::{JobInfo, JobQueue};
use crate::matrix;
use crate::settings::{Settings, SettingsStore};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    path: String,
    name: String,
    format: Option<FileFormat>,
    category: Option<FormatCategory>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    format: FileFormat,
    label: String,
    category: FormatCategory,
}

/// Detects the format of each dropped or picked path. Directories and
/// missing files are skipped.
#[tauri::command]
pub fn probe_files(paths: Vec<String>) -> Vec<FileInfo> {
    paths
        .into_iter()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .map(|path| {
            let format = FileFormat::detect(&path);
            FileInfo {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: path.display().to_string(),
                format,
                category: format.map(FileFormat::category),
            }
        })
        .collect()
}

/// Targets every selected format can convert to, minus anything this
/// machine cannot do (no FFmpeg, no AVIF encoder).
#[tauri::command]
pub async fn available_targets(formats: Vec<FileFormat>) -> Vec<TargetInfo> {
    let candidates = matrix::common_targets(&formats);
    let encoders = if candidates.contains(&FileFormat::Avif) {
        ffmpeg::available_encoders()
    } else {
        Default::default()
    };
    candidates
        .into_iter()
        .filter(|target| {
            formats
                .iter()
                .all(|source| engines::is_available(*source, *target, &encoders))
        })
        .map(|format| TargetInfo {
            label: format.display_name(),
            category: format.category(),
            format,
        })
        .collect()
}

#[tauri::command]
pub fn apply_file_tool(
    paths: Vec<String>,
    tool: FileTool,
    queue: State<'_, JobQueue>,
    settings: State<'_, SettingsStore>,
) {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    queue.apply_file_tool(paths, tool, &settings);
}

#[tauri::command]
pub fn convert_files(
    paths: Vec<String>,
    target: FileFormat,
    queue: State<'_, JobQueue>,
    settings: State<'_, SettingsStore>,
) {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    queue.convert(paths, target, &settings);
}

#[tauri::command]
pub fn list_jobs(queue: State<'_, JobQueue>) -> Vec<JobInfo> {
    queue.list()
}

#[tauri::command]
pub fn cancel_job(id: String, queue: State<'_, JobQueue>) {
    queue.cancel(&id);
}

#[tauri::command]
pub fn cancel_all_jobs(queue: State<'_, JobQueue>) {
    queue.cancel_all();
}

#[tauri::command]
pub fn dismiss_finished(queue: State<'_, JobQueue>) {
    queue.dismiss_finished();
}

#[tauri::command]
pub fn get_settings(settings: State<'_, SettingsStore>) -> Settings {
    settings.get()
}

#[tauri::command]
pub fn update_settings(
    settings: Settings,
    store: State<'_, SettingsStore>,
) -> Result<Settings, String> {
    store.update(settings)
}

#[tauri::command]
pub fn get_launch_at_login(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_launch_at_login(
    enabled: bool,
    app: AppHandle,
    store: State<'_, SettingsStore>,
) -> Result<bool, String> {
    let mut settings = store.get();
    settings.launch_at_login = enabled;
    store.update(settings)?;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    Ok(manager.is_enabled().unwrap_or(enabled))
}

#[tauri::command]
pub async fn ffmpeg_health() -> Health {
    ffmpeg::check_health()
}

/// Opens an output file this app created.
#[tauri::command]
pub fn open_output(path: String, queue: State<'_, JobQueue>) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !queue.is_known_output(&path) {
        return Err("That file was not created by FileForge.".into());
    }
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}

/// Shows an output file in Explorer.
#[tauri::command]
pub fn reveal_output(path: String, queue: State<'_, JobQueue>) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !queue.is_known_output(&path) {
        return Err("That file was not created by FileForge.".into());
    }
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CropPreview {
    width: u32,
    height: u32,
    png_base64: String,
    video: bool,
}

/// Picture the crop editor draws, and the pixel size the numbers use.
/// Runs off the UI thread so a large photo does not freeze the window white.
#[tauri::command]
pub async fn crop_preview(path: String) -> Result<CropPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(&path);
        let format = FileFormat::detect(&path)
            .ok_or_else(|| format!("Unsupported file type: {}", path.display()))?;
        let preview = crate::engines::crop::preview(&path, format).map_err(|error| error.to_string())?;
        Ok(CropPreview {
            width: preview.width,
            height: preview.height,
            png_base64: preview.png_base64,
            video: preview.video,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Opens the crop editor for one image or video. Nothing is written yet.
/// Async so Windows can create the WebView2 window without deadlocking.
#[tauri::command]
pub async fn open_crop_editor(path: String, app: AppHandle) {
    crate::cropwin::open(&app, path);
}

/// The file the crop window should load, if the open event arrived first.
#[tauri::command]
pub fn current_crop() -> Option<String> {
    crate::cropwin::current()
}

/// Writes a new cropped file from the rectangle the editor confirmed.
#[tauri::command]
pub fn apply_crop(
    path: String,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    queue: State<'_, JobQueue>,
    settings: State<'_, SettingsStore>,
) {
    queue.apply_crop(PathBuf::from(path), x, y, width, height, &settings);
}

/// Closes the wheel overlay (after a selection, cancel, or drop).
#[tauri::command]
pub fn wheel_hide(app: AppHandle) {
    crate::wheel::hide(&app);
}

#[tauri::command]
pub fn show_logs() -> Result<(), String> {
    open_logs()
}

pub fn open_logs() -> Result<(), String> {
    let path = diag::path().ok_or("The log file is not available.")?;
    if !path.exists() {
        return Err("The log file has not been created yet.".into());
    }
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}
