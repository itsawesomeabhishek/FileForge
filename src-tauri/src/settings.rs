use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::ffmpeg;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputLocation {
    BesideOriginal,
    Downloads,
    Desktop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub output_location: OutputLocation,
    pub reveal_in_explorer: bool,
    pub jpeg_quality: f64,
    pub ffmpeg_path: String,
    /// Master switch for the drag-to-wheel gesture.
    pub wheel_enabled: bool,
    /// Executable names (e.g. `code.exe`) the wheel ignores drags from.
    pub excluded_apps: Vec<String>,
    /// Register with Windows startup. On unless the user turns it off.
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_location: OutputLocation::BesideOriginal,
            reveal_in_explorer: true,
            jpeg_quality: 0.9,
            ffmpeg_path: String::new(),
            wheel_enabled: true,
            excluded_apps: Vec::new(),
            launch_at_login: true,
        }
    }
}

impl Settings {
    /// Clamps values that came from the UI or a hand-edited file.
    pub fn sanitized(mut self) -> Self {
        self.jpeg_quality = if self.jpeg_quality.is_finite() {
            self.jpeg_quality.clamp(0.1, 1.0)
        } else {
            0.9
        };
        self.ffmpeg_path = self.ffmpeg_path.trim().to_string();
        let mut apps: Vec<String> = Vec::new();
        for app in self.excluded_apps.drain(..) {
            let app = app.trim().to_lowercase();
            if !app.is_empty() && !apps.contains(&app) {
                apps.push(app);
            }
        }
        self.excluded_apps = apps;
        self
    }

    /// The directory outputs for `source` should be written to.
    pub fn output_directory(&self, source: &Path, app: &tauri::AppHandle) -> PathBuf {
        let beside = || {
            source
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        };
        match self.output_location {
            OutputLocation::BesideOriginal => beside(),
            OutputLocation::Downloads => app.path().download_dir().unwrap_or_else(|_| beside()),
            OutputLocation::Desktop => app.path().desktop_dir().unwrap_or_else(|_| beside()),
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        let settings = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
            .unwrap_or_default()
            .sanitized();
        ffmpeg::set_configured_path(&settings.ffmpeg_path);
        Self {
            path,
            current: Mutex::new(settings),
        }
    }

    pub fn get(&self) -> Settings {
        self.current
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn update(&self, settings: Settings) -> Result<Settings, String> {
        let settings = settings.sanitized();
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, text).map_err(|e| e.to_string())?;
        ffmpeg::set_configured_path(&settings.ffmpeg_path);
        if let Ok(mut guard) = self.current.lock() {
            *guard = settings.clone();
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_mac_app() {
        let settings = Settings::default();
        assert_eq!(settings.output_location, OutputLocation::BesideOriginal);
        assert!(settings.reveal_in_explorer);
        assert_eq!(settings.jpeg_quality, 0.9);
    }

    #[test]
    fn sanitizes_bad_values() {
        let bad = Settings {
            jpeg_quality: f64::NAN,
            ffmpeg_path: "  C:\\x\\ffmpeg.exe  ".into(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(bad.jpeg_quality, 0.9);
        assert_eq!(bad.ffmpeg_path, "C:\\x\\ffmpeg.exe");
        let low = Settings {
            jpeg_quality: -4.0,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(low.jpeg_quality, 0.1);
    }

    #[test]
    fn normalizes_excluded_apps() {
        let settings = Settings {
            excluded_apps: vec![" Code.EXE ".into(), "code.exe".into(), "".into(), "Figma.exe".into()],
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(settings.excluded_apps, vec!["code.exe", "figma.exe"]);
        assert!(settings.wheel_enabled);
    }

    #[test]
    fn partial_json_fills_defaults() {
        let parsed: Settings = serde_json::from_str(r#"{"jpegQuality":0.5}"#).unwrap();
        assert_eq!(parsed.jpeg_quality, 0.5);
        assert!(parsed.reveal_in_explorer);
        assert!(parsed.launch_at_login);
    }

    #[test]
    fn persists_and_reloads() {
        let dir = std::env::temp_dir().join(format!("fileforge-set-{}", uuid::Uuid::new_v4()));
        let path = dir.join("settings.json");
        let store = SettingsStore::load(path.clone());
        store
            .update(Settings {
                output_location: OutputLocation::Desktop,
                jpeg_quality: 0.7,
                ..Settings::default()
            })
            .unwrap();
        let reloaded = SettingsStore::load(path).get();
        assert_eq!(reloaded.output_location, OutputLocation::Desktop);
        assert_eq!(reloaded.jpeg_quality, 0.7);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
