pub mod audio;
pub mod document;
pub mod images;
#[cfg(test)]
mod integration_tests;
pub mod pdf;
pub mod subtitle;
pub mod text;
pub mod video;

use std::path::PathBuf;

use crate::error::{ExploitError, Result};
use crate::ffmpeg::{CancelToken, ProgressFn};
use crate::formats::FileFormat;

/// User-tunable settings an engine may need.
#[derive(Debug, Clone, Copy)]
pub struct ConvertOptions {
    /// JPEG quality from 0.0 to 1.0.
    pub jpeg_quality: f64,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self { jpeg_quality: 0.9 }
    }
}

pub struct ConversionRequest {
    pub source: PathBuf,
    pub source_format: FileFormat,
    pub target: FileFormat,
    pub destination: PathBuf,
    pub progress: ProgressFn,
    pub cancel: CancelToken,
    pub options: ConvertOptions,
}

pub trait ConversionEngine: Sync {
    fn name(&self) -> &'static str;
    fn supports(&self, from: FileFormat, to: FileFormat) -> bool;
    /// Whether this conversion shells out to FFmpeg (and so needs it installed).
    fn requires_ffmpeg(&self, _from: FileFormat, _to: FileFormat) -> bool {
        false
    }
    fn convert(&self, request: &ConversionRequest) -> Result<()>;
}

/// True when the conversion is supported and everything it needs is present
/// on this machine (FFmpeg, and the AVIF encoder for AVIF output).
pub fn is_available(from: FileFormat, to: FileFormat, encoders: &std::collections::HashSet<String>) -> bool {
    let Some(engine) = engine_for(from, to) else {
        return false;
    };
    if !engine.requires_ffmpeg(from, to) {
        return true;
    }
    if !crate::ffmpeg::is_available() {
        return false;
    }
    !(to == FileFormat::Avif && !encoders.contains("libaom-av1"))
}

/// Order matters: the first engine that supports a pair wins.
static ENGINES: [&dyn ConversionEngine; 5] = [
    &images::ImageEngine,
    &audio::AudioEngine,
    &video::VideoEngine,
    &document::DocumentEngine,
    &subtitle::SubtitleEngine,
];

pub fn engine_for(from: FileFormat, to: FileFormat) -> Option<&'static dyn ConversionEngine> {
    ENGINES.iter().copied().find(|engine| engine.supports(from, to))
}

pub fn supports(from: FileFormat, to: FileFormat) -> bool {
    engine_for(from, to).is_some()
}

/// Converts into a hidden temp file beside the destination, then renames it
/// into place. Never overwrites an existing file and removes the temp file on
/// any failure.
pub fn convert(request: &ConversionRequest) -> Result<()> {
    let engine = engine_for(request.source_format, request.target).ok_or(
        ExploitError::UnsupportedConversion(request.source_format, request.target),
    )?;
    crate::diag::log(&format!(
        "engine {} {} -> {} src={}",
        engine.name(),
        request.source_format.raw_value(),
        request.target.raw_value(),
        request.source.display()
    ));
    let directory = request
        .destination
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let extension = request
        .destination
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = directory.join(format!(".exploit-{}.{}", uuid::Uuid::new_v4(), extension));

    let staged = ConversionRequest {
        source: request.source.clone(),
        source_format: request.source_format,
        target: request.target,
        destination: temp.clone(),
        progress: request.progress.clone(),
        cancel: request.cancel.clone(),
        options: request.options,
    };

    let result = engine.convert(&staged).and_then(|()| {
        request.cancel.check()?;
        if !temp.is_file() {
            return Err(ExploitError::EncodeFailed(
                request
                    .destination
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ));
        }
        if request.destination.exists() {
            return Err(ExploitError::Io(format!(
                "{} already exists.",
                request.destination.display()
            )));
        }
        std::fs::rename(&temp, &request.destination)?;
        Ok(())
    });

    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
