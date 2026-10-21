use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::diag;
use crate::engines::{self, ConversionRequest, ConvertOptions};
use crate::error::ExploitError;
use crate::ffmpeg::{CancelToken, ProgressFn};
use crate::formats::{FileFormat, FormatCategory};
use crate::matrix;
use crate::settings::SettingsStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Waiting,
    Running,
    Done,
    Cancelled,
    Failed,
}

impl JobStatus {
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled | Self::Failed)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub category: Option<FormatCategory>,
    pub status: JobStatus,
    pub progress: f64,
    pub error: Option<String>,
    pub outputs: Vec<String>,
}

struct Entry {
    info: JobInfo,
    cancel: CancelToken,
    destination: Option<PathBuf>,
}

struct Work {
    id: String,
    source: PathBuf,
    source_format: FileFormat,
    target: FileFormat,
    destination: PathBuf,
}

type Entries = Arc<Mutex<Vec<Entry>>>;

/// Serial conversion queue: one worker thread, jobs run in order.
pub struct JobQueue {
    entries: Entries,
    sender: Mutex<Sender<Work>>,
    app: AppHandle,
}

fn lock(entries: &Entries) -> MutexGuard<'_, Vec<Entry>> {
    entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn snapshot(entries: &[Entry]) -> Vec<JobInfo> {
    entries.iter().map(|entry| entry.info.clone()).collect()
}

fn publish(app: &AppHandle, jobs: Vec<JobInfo>) {
    let _ = app.emit("jobs-updated", jobs);
}

impl JobQueue {
    pub fn start(app: AppHandle) -> Self {
        let entries: Entries = Arc::new(Mutex::new(Vec::new()));
        let (sender, receiver) = mpsc::channel::<Work>();

        let worker_entries = Arc::clone(&entries);
        let worker_app = app.clone();
        thread::spawn(move || {
            while let Ok(work) = receiver.recv() {
                run_work(&worker_app, &worker_entries, work);
            }
        });

        Self {
            entries,
            sender: Mutex::new(sender),
            app,
        }
    }

    pub fn list(&self) -> Vec<JobInfo> {
        snapshot(&lock(&self.entries))
    }

    /// Queues one job per file. Finished jobs from the previous batch are
    /// cleared first, as in the macOS app.
    pub fn convert(&self, paths: Vec<PathBuf>, target: FileFormat, settings: &SettingsStore) {
        let settings = settings.get();
        let mut guard = lock(&self.entries);
        guard.retain(|entry| !entry.info.status.is_finished());
        let mut queued: Vec<Work> = Vec::new();

        for source in paths {
            let detail = source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| source.display().to_string());
            let id = uuid::Uuid::new_v4().to_string();
            let source_format = FileFormat::detect(&source);
            let mut info = JobInfo {
                id: id.clone(),
                title: format!("Converting to {}", target.display_name()),
                detail,
                category: source_format.map(FileFormat::category),
                status: JobStatus::Waiting,
                progress: 0.0,
                error: None,
                outputs: Vec::new(),
            };

            let failure = match source_format {
                None => Some(format!("Unsupported file type: {}", info.detail)),
                Some(_) if !source.is_file() => {
                    Some(format!("{} could not be found.", info.detail))
                }
                Some(from) if !engines::supports(from, target) => {
                    Some(ExploitError::UnsupportedConversion(from, target).to_string())
                }
                Some(_) => None,
            };

            if let (None, Some(from)) = (&failure, source_format) {
                let directory = settings.output_directory(&source, &self.app);
                let reserved: HashSet<String> = guard
                    .iter()
                    .filter(|entry| !entry.info.status.is_finished())
                    .filter_map(|entry| entry.destination.as_ref())
                    .filter(|dest| dest.parent() == Some(directory.as_path()))
                    .filter_map(|dest| dest.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .collect();
                let destination = matrix::output_path(&source, target, &directory, &reserved);
                diag::log(&format!(
                    "enqueue {} -> {} at {}",
                    source.display(),
                    target.display_name(),
                    destination.display()
                ));
                queued.push(Work {
                    id: id.clone(),
                    source: source.clone(),
                    source_format: from,
                    target,
                    destination: destination.clone(),
                });
                guard.push(Entry {
                    info,
                    cancel: CancelToken::new(),
                    destination: Some(destination),
                });
            } else {
                info.status = JobStatus::Failed;
                info.error = failure;
                diag::log(&format!("rejected {}: {:?}", info.detail, info.error));
                guard.push(Entry {
                    info,
                    cancel: CancelToken::new(),
                    destination: None,
                });
            }
        }

        let jobs = snapshot(&guard);
        drop(guard);
        publish(&self.app, jobs);

        if let Ok(sender) = self.sender.lock() {
            for work in queued {
                let _ = sender.send(work);
            }
        }
    }

    pub fn cancel(&self, id: &str) {
        let mut guard = lock(&self.entries);
        if let Some(entry) = guard.iter_mut().find(|e| e.info.id == id) {
            if !entry.info.status.is_finished() {
                entry.cancel.cancel();
                entry.info.status = JobStatus::Cancelled;
                diag::log(&format!("cancel {}", entry.info.detail));
            }
        }
        let jobs = snapshot(&guard);
        drop(guard);
        publish(&self.app, jobs);
    }

    pub fn cancel_all(&self) {
        let mut guard = lock(&self.entries);
        for entry in guard.iter_mut().filter(|e| !e.info.status.is_finished()) {
            entry.cancel.cancel();
            entry.info.status = JobStatus::Cancelled;
        }
        let jobs = snapshot(&guard);
        drop(guard);
        publish(&self.app, jobs);
    }

    pub fn dismiss_finished(&self) {
        let mut guard = lock(&self.entries);
        guard.retain(|entry| !entry.info.status.is_finished());
        let jobs = snapshot(&guard);
        drop(guard);
        publish(&self.app, jobs);
    }

    /// Only files this app produced may be opened or revealed from the UI.
    pub fn is_known_output(&self, path: &Path) -> bool {
        lock(&self.entries).iter().any(|entry| {
            entry
                .info
                .outputs
                .iter()
                .any(|output| Path::new(output) == path)
        })
    }
}

fn run_work(app: &AppHandle, entries: &Entries, work: Work) {
    // Skip jobs cancelled while waiting; mark the rest running.
    let cancel = {
        let mut guard = lock(entries);
        let Some(entry) = guard.iter_mut().find(|e| e.info.id == work.id) else {
            return;
        };
        if entry.info.status != JobStatus::Waiting {
            return;
        }
        entry.info.status = JobStatus::Running;
        let cancel = entry.cancel.clone();
        let jobs = snapshot(&guard);
        drop(guard);
        publish(app, jobs);
        cancel
    };

    let (jpeg_quality, reveal) = {
        let store = app.state::<SettingsStore>();
        let settings = store.get();
        (settings.jpeg_quality, settings.reveal_in_explorer)
    };

    let progress: ProgressFn = {
        let app = app.clone();
        let entries = Arc::clone(entries);
        let id = work.id.clone();
        Arc::new(move |value: f64| {
            if !value.is_finite() {
                return;
            }
            let value = value.clamp(0.0, 1.0);
            let mut guard = lock(&entries);
            let Some(entry) = guard.iter_mut().find(|e| e.info.id == id) else {
                return;
            };
            if entry.info.status != JobStatus::Running || (value - entry.info.progress).abs() < 0.005 {
                return;
            }
            entry.info.progress = value;
            let jobs = snapshot(&guard);
            drop(guard);
            publish(&app, jobs);
        })
    };

    let request = ConversionRequest {
        source: work.source.clone(),
        source_format: work.source_format,
        target: work.target,
        destination: work.destination.clone(),
        progress,
        cancel,
        options: ConvertOptions { jpeg_quality },
    };
    let result = engines::convert(&request);

    let mut revealed: Option<PathBuf> = None;
    let jobs = {
        let mut guard = lock(entries);
        if let Some(entry) = guard.iter_mut().find(|e| e.info.id == work.id) {
            // A cancel request already set the final state; keep it.
            if entry.info.status != JobStatus::Cancelled {
                match &result {
                    Ok(()) => {
                        entry.info.status = JobStatus::Done;
                        entry.info.progress = 1.0;
                        entry.info.outputs = vec![work.destination.display().to_string()];
                        revealed = Some(work.destination.clone());
                        diag::log(&format!("done {}", work.destination.display()));
                    }
                    Err(ExploitError::Cancelled) => entry.info.status = JobStatus::Cancelled,
                    Err(error) => {
                        entry.info.status = JobStatus::Failed;
                        entry.info.error = Some(error.to_string());
                        diag::log(&format!("failed {}: {error}", entry.info.detail));
                    }
                }
            }
        }
        snapshot(&guard)
    };
    publish(app, jobs);

    if reveal {
        if let Some(path) = revealed {
            let _ = tauri_plugin_opener::reveal_item_in_dir(path);
        }
    }
}
