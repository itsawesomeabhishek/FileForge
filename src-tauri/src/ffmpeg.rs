use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use serde::Serialize;

use crate::error::{ForgeError, Result};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static CONFIGURED_PATH: RwLock<String> = RwLock::new(String::new());

/// Shared cancellation flag observed by running conversions.
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(ForgeError::Cancelled)
        } else {
            Ok(())
        }
    }
}

pub type ProgressFn = Arc<dyn Fn(f64) + Send + Sync>;

/// Sets the user-configured FFmpeg path from Settings (empty means automatic).
pub fn set_configured_path(path: &str) {
    if let Ok(mut guard) = CONFIGURED_PATH.write() {
        *guard = path.trim().to_string();
    }
    if let Ok(mut guard) = ENCODERS.write() {
        *guard = None;
    }
}

fn configured_path() -> String {
    CONFIGURED_PATH
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

fn exe_name(tool: &str) -> String {
    if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_string()
    }
}

/// Searches `PATH` for an executable.
pub fn which(tool: &str) -> Option<PathBuf> {
    let name = exe_name(tool);
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(&name))
        .find(|candidate| candidate.is_file())
}

fn bundled(tool: &str) -> Option<PathBuf> {
    let name = exe_name(tool);
    // Installed app and `tauri dev`: sidecars are copied next to the executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(&name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    // `cargo test`: use the downloaded sidecar in the source tree.
    let triple = format!("{tool}-x86_64-pc-windows-msvc.exe");
    let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(triple);
    dev.is_file().then_some(dev)
}

pub fn ffmpeg_path() -> Option<PathBuf> {
    let configured = configured_path();
    if !configured.is_empty() {
        let path = PathBuf::from(&configured);
        if path.is_file() {
            return Some(path);
        }
    }
    bundled("ffmpeg").or_else(|| which("ffmpeg"))
}

pub fn ffprobe_path() -> Option<PathBuf> {
    if let Some(ffmpeg) = ffmpeg_path() {
        if let Some(dir) = ffmpeg.parent() {
            let sibling = dir.join(exe_name("ffprobe"));
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }
    bundled("ffprobe").or_else(|| which("ffprobe"))
}

pub fn is_available() -> bool {
    ffmpeg_path().is_some()
}

fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Health {
    Missing,
    Broken { path: String, reason: String },
    Ready { path: String, version: String },
}

pub fn check_health() -> Health {
    let Some(ffmpeg) = ffmpeg_path() else {
        return Health::Missing;
    };
    let shown = ffmpeg.display().to_string();
    match command(&ffmpeg)
        .args(["-hide_banner", "-version"])
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let first = stdout.lines().next().unwrap_or("");
            if output.status.success() && first.starts_with("ffmpeg version") {
                let version = first
                    .trim_start_matches("ffmpeg version ")
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                Health::Ready {
                    path: shown,
                    version,
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let text = if stderr.trim().is_empty() { stdout } else { stderr };
                Health::Broken {
                    path: shown,
                    reason: summarize(&text),
                }
            }
        }
        Err(error) => Health::Broken {
            path: shown,
            reason: error.to_string(),
        },
    }
}

/// `ffmpeg -encoders` is a full process start. Image wheels ask for it on
/// every drag because AVIF is in the candidate list, so the result is kept
/// until the configured FFmpeg path changes.
static ENCODERS: std::sync::RwLock<Option<HashSet<String>>> = std::sync::RwLock::new(None);

/// Encoder names reported by `ffmpeg -encoders`.
pub fn available_encoders() -> HashSet<String> {
    if let Ok(guard) = ENCODERS.read() {
        if let Some(cached) = guard.as_ref() {
            return cached.clone();
        }
    }
    let loaded = query_encoders();
    if !loaded.is_empty() {
        if let Ok(mut guard) = ENCODERS.write() {
            *guard = Some(loaded.clone());
        }
    }
    loaded
}

fn query_encoders() -> HashSet<String> {
    let Some(ffmpeg) = ffmpeg_path() else {
        return HashSet::new();
    };
    let Ok(output) = command(&ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .stdin(Stdio::null())
        .output()
    else {
        return HashSet::new();
    };
    if !output.status.success() {
        return HashSet::new();
    }
    parse_encoders(&String::from_utf8_lossy(&output.stdout))
}

fn parse_encoders(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let flags = fields.next()?;
            let name = fields.next()?;
            let kind = flags.chars().next()?;
            (flags.len() == 6 && matches!(kind, 'V' | 'A' | 'S') && name != "=")
                .then(|| name.to_string())
        })
        .collect()
}

/// Boils FFmpeg's error output down to the line that explains the failure.
pub fn summarize(output: &str) -> String {
    let lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let causes = [
        "Unknown encoder",
        "Encoder not found",
        "Invalid data found",
        "Permission denied",
        "No such file",
        "Error opening",
    ];
    if let Some(cause) = lines
        .iter()
        .find(|line| causes.iter().any(|c| line.contains(c)))
    {
        return (*cause).to_string();
    }
    lines
        .last()
        .map(|line| (*line).to_string())
        .unwrap_or_else(|| output.trim().to_string())
}

/// Media duration in seconds via ffprobe, if available.
pub fn probe_duration(source: &Path) -> Option<f64> {
    let ffprobe = ffprobe_path()?;
    let output = command(&ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(source)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|duration| duration.is_finite() && *duration > 0.0)
}

/// Parses an `out_time_us=` / `out_time_ms=` progress line into seconds
/// (FFmpeg reports both in microseconds).
fn parse_progress_seconds(line: &str) -> Option<f64> {
    let value = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))?;
    let micros: f64 = value.trim().parse().ok()?;
    (micros >= 0.0).then_some(micros / 1_000_000.0)
}

/// Runs FFmpeg with the given arguments, streaming progress when the
/// duration is known. Kills the process if `cancel` fires.
pub fn run(
    arguments: &[String],
    purpose: &str,
    duration: Option<f64>,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    let ffmpeg = ffmpeg_path().ok_or_else(|| ForgeError::FfmpegMissing(purpose.to_string()))?;
    cancel.check()?;

    let mut child = command(&ffmpeg)
        .args(["-hide_banner", "-y", "-nostdin", "-progress", "pipe:1", "-loglevel", "error"])
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ForgeError::ToolFailed("ffmpeg".into(), error.to_string()))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let progress_thread = {
        let progress = Arc::clone(progress);
        thread::spawn(move || {
            let Some(stdout) = stdout else { return };
            for line in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
                if let (Some(seconds), Some(total)) = (parse_progress_seconds(&line), duration) {
                    progress((seconds / total).clamp(0.0, 0.99));
                }
            }
        })
    };
    let stderr_thread = thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut stderr) = stderr {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes);
            text = String::from_utf8_lossy(&bytes).into_owned();
        }
        text
    });

    let status = loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = progress_thread.join();
            let _ = stderr_thread.join();
            return Err(ForgeError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(error) => {
                let _ = child.kill();
                return Err(ForgeError::ToolFailed("ffmpeg".into(), error.to_string()));
            }
        }
    };
    let _ = progress_thread.join();
    let errors = stderr_thread.join().unwrap_or_default();

    if status.success() {
        progress(1.0);
        Ok(())
    } else {
        Err(ForgeError::ToolFailed("ffmpeg".into(), summarize(&errors)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_lines() {
        assert_eq!(parse_progress_seconds("out_time_us=2500000"), Some(2.5));
        assert_eq!(parse_progress_seconds("out_time_ms=1000000"), Some(1.0));
        assert_eq!(parse_progress_seconds("out_time_us=N/A"), None);
        assert_eq!(parse_progress_seconds("frame=12"), None);
    }

    #[test]
    fn parses_encoder_list() {
        let text = " Encoders:\n V..... = Video\n ------\n V....D libx264              libx264 H.264\n A....D aac                  AAC\n";
        let encoders = parse_encoders(text);
        assert!(encoders.contains("libx264"));
        assert!(encoders.contains("aac"));
        assert!(!encoders.contains("="));
    }

    #[test]
    fn summarizes_known_causes() {
        let text = "[x] something\nUnknown encoder 'libfoo'\nlast line";
        assert_eq!(summarize(text), "Unknown encoder 'libfoo'");
        assert_eq!(summarize("a\nb"), "b");
    }

    #[test]
    fn cancel_token_flips() {
        let token = CancelToken::new();
        assert!(token.check().is_ok());
        token.clone().cancel();
        assert!(matches!(token.check(), Err(ForgeError::Cancelled)));
    }
}
