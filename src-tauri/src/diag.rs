//! Tiny append-only diagnostic log (the "Show Logs" tray item opens it).

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
const MAX_BYTES: u64 = 512 * 1024;

pub fn init(path: PathBuf) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Start over once the log grows large so it never fills the disk.
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::remove_file(&path);
    }
    let _ = LOG_PATH.set(path);
    log("FileForge started");
}

pub fn path() -> Option<PathBuf> {
    LOG_PATH.get().cloned()
}

pub fn log(message: &str) {
    let Some(path) = LOG_PATH.get() else { return };
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{seconds}] {message}");
    }
}
