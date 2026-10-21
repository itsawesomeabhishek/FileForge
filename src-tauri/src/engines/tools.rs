//! One-step file tools for the Ctrl wheel. Each tool writes a new file.
//! Crop, trim, redact, speed, split, and bleep need an editor and are not here.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::images::{self, ImageTool};
use super::{audio, video};
use crate::error::{ForgeError, Result};
use crate::ffmpeg::{self, CancelToken, ProgressFn};
use crate::formats::{FileFormat, FormatCategory};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileTool {
    Compress,
    Metadata,
    Background,
    Mute,
    Snapshot,
    Normalize,
    Mono,
    Visualize,
}

impl FileTool {
    pub fn title(self) -> &'static str {
        match self {
            Self::Compress => "Compressing",
            Self::Metadata => "Removing metadata",
            Self::Background => "Adding a white background",
            Self::Mute => "Removing audio",
            Self::Snapshot => "Saving a frame",
            Self::Normalize => "Normalizing volume",
            Self::Mono => "Mixing to mono",
            Self::Visualize => "Drawing a waveform",
        }
    }

    pub fn file_label(self) -> &'static str {
        match self {
            Self::Compress => "compressed",
            Self::Metadata => "stripped",
            Self::Background => "background",
            Self::Mute => "muted",
            Self::Snapshot => "snapshot",
            Self::Normalize => "normalized",
            Self::Mono => "mono",
            Self::Visualize => "visualizer",
        }
    }

    pub fn extension(self, source: FileFormat) -> &'static str {
        match (self, source.category()) {
            (Self::Background | Self::Snapshot | Self::Visualize, _) => {
                if self == Self::Visualize { "mp4" } else { "jpg" }
            }
            (Self::Compress | Self::Metadata, FormatCategory::Image) => {
                let image = match self {
                    Self::Compress => ImageTool::Compress,
                    _ => ImageTool::Metadata,
                };
                image.extension(source)
            }
            _ => source.file_extension(),
        }
    }
}

pub fn accepts(tool: FileTool, format: FileFormat) -> bool {
    let category = format.category();
    match tool {
        FileTool::Compress | FileTool::Metadata => matches!(
            category,
            FormatCategory::Image | FormatCategory::Video | FormatCategory::Audio
        ),
        FileTool::Background => category == FormatCategory::Image,
        FileTool::Mute | FileTool::Snapshot => category == FormatCategory::Video,
        FileTool::Normalize | FileTool::Mono | FileTool::Visualize => category == FormatCategory::Audio,
    }
}

pub fn apply(
    source: &Path,
    format: FileFormat,
    tool: FileTool,
    destination: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    if !accepts(tool, format) {
        return Err(ForgeError::InvalidInput(format!(
            "{} does not work on {} files.",
            tool.title(),
            format.display_name()
        )));
    }
    match (tool, format.category()) {
        (FileTool::Compress | FileTool::Metadata | FileTool::Background, FormatCategory::Image) => {
            let image = match tool {
                FileTool::Compress => ImageTool::Compress,
                FileTool::Metadata => ImageTool::Metadata,
                _ => ImageTool::Background,
            };
            images::apply_image_tool(source, format, image, destination, progress, cancel)
        }
        (FileTool::Compress, FormatCategory::Video) => compress_video(source, format, destination, progress, cancel),
        (FileTool::Mute, _) => mute(source, format, destination, progress, cancel),
        (FileTool::Snapshot, _) => snapshot(source, destination, progress, cancel),
        (FileTool::Metadata, _) => strip_metadata(source, format, destination, progress, cancel),
        (FileTool::Compress, FormatCategory::Audio) => compress_audio(source, format, destination, progress, cancel),
        (FileTool::Normalize, _) => filter_audio(source, format, "loudnorm=I=-16:TP=-1.5:LRA=11", destination, progress, cancel),
        (FileTool::Mono, _) => filter_audio(source, format, "aformat=channel_layouts=mono", destination, progress, cancel),
        (FileTool::Visualize, _) => visualize(source, destination, progress, cancel),
        _ => Err(ForgeError::InvalidInput("This tool is not available.".into())),
    }
}

fn compress_video(
    source: &Path,
    format: FileFormat,
    destination: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    if format == FileFormat::Gif {
        let args = args(&[
            "-i",
            &source.display().to_string(),
            "-vf",
            "fps=8,scale='min(480,iw)':-2:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse",
            "-loop",
            "0",
            "-f",
            "gif",
            &destination.display().to_string(),
        ]);
        return ffmpeg::run(&args, "Compressing GIF", ffmpeg::probe_duration(source), progress, cancel);
    }
    video::ffmpeg_convert(
        source,
        destination,
        format,
        &[],
        Some(800_000),
        Some(96_000),
        progress,
        cancel,
    )
}

fn mute(
    source: &Path,
    format: FileFormat,
    destination: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    let duration = ffmpeg::probe_duration(source);
    let copy = args(&[
        "-i",
        &source.display().to_string(),
        "-an",
        "-c:v",
        "copy",
        &destination.display().to_string(),
    ]);
    if ffmpeg::run(&copy, "Removing audio", duration, progress, cancel).is_ok() {
        return Ok(());
    }
    cancel.check()?;
    let _ = std::fs::remove_file(destination);
    let mut encode = vec!["-i".into(), source.display().to_string(), "-an".into(), "-map_metadata".into(), "-1".into()];
    encode.extend(video_only(format));
    encode.push(destination.display().to_string());
    finish(destination, ffmpeg::run(&encode, "Removing audio", duration, progress, cancel))
}

fn snapshot(source: &Path, destination: &Path, progress: &ProgressFn, cancel: &CancelToken) -> Result<()> {
    let duration = ffmpeg::probe_duration(source).unwrap_or(0.0);
    let at = if duration > 1.0 { "1" } else { "0" };
    let args = args(&[
        "-ss",
        at,
        "-i",
        &source.display().to_string(),
        "-frames:v",
        "1",
        "-q:v",
        "3",
        &destination.display().to_string(),
    ]);
    ffmpeg::run(&args, "Saving a frame", None, progress, cancel)
}

fn strip_metadata(
    source: &Path,
    format: FileFormat,
    destination: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    let duration = ffmpeg::probe_duration(source);
    let copy = args(&[
        "-i",
        &source.display().to_string(),
        "-map_metadata",
        "-1",
        "-c",
        "copy",
        &destination.display().to_string(),
    ]);
    if ffmpeg::run(&copy, "Removing metadata", duration, progress, cancel).is_ok() {
        return Ok(());
    }
    cancel.check()?;
    let _ = std::fs::remove_file(destination);
    match format.category() {
        FormatCategory::Audio => strip_audio(source, format, destination, progress, cancel),
        FormatCategory::Video => {
            let mut encode = vec!["-i".into(), source.display().to_string(), "-map_metadata".into(), "-1".into()];
            encode.extend(video::codec_arguments(format, Some(800_000), Some(128_000)));
            encode.push(destination.display().to_string());
            finish(destination, ffmpeg::run(&encode, "Removing metadata", duration, progress, cancel))
        }
        _ => Err(ForgeError::InvalidInput("This file has no removable metadata.".into())),
    }
}

fn compress_audio(
    source: &Path,
    format: FileFormat,
    destination: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    let filters = if matches!(format, FileFormat::Wav | FileFormat::Aiff | FileFormat::Flac) {
        vec!["aformat=channel_layouts=mono,aresample=22050".to_string()]
    } else {
        Vec::new()
    };
    let bitrate = if filters.is_empty() { Some(96_000) } else { None };
    audio::ffmpeg_convert(source, destination, format, &filters, bitrate, progress, cancel)
}

fn strip_audio(
    source: &Path,
    format: FileFormat,
    destination: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    let mut encode = vec![
        "-i".into(),
        source.display().to_string(),
        "-vn".into(),
        "-map_metadata".into(),
        "-1".into(),
    ];
    encode.extend(audio::resolved_codec_arguments(format, None));
    encode.extend(audio::container_arguments(format));
    encode.push(destination.display().to_string());
    finish(
        destination,
        ffmpeg::run(
            &encode,
            "Removing metadata",
            ffmpeg::probe_duration(source),
            progress,
            cancel,
        ),
    )
}

fn filter_audio(
    source: &Path,
    format: FileFormat,
    filter: &str,
    destination: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    audio::ffmpeg_convert(
        source,
        destination,
        format,
        &[filter.to_string()],
        None,
        progress,
        cancel,
    )
}

fn visualize(source: &Path, destination: &Path, progress: &ProgressFn, cancel: &CancelToken) -> Result<()> {
    let args = args(&[
        "-i",
        &source.display().to_string(),
        "-filter_complex",
        "[0:a]aformat=channel_layouts=stereo,showwaves=s=1280x720:mode=line:rate=25:colors=0xFF5C00,format=yuv420p[v]",
        "-map",
        "[v]",
        "-map",
        "0:a",
        "-c:v",
        "libx264",
        "-preset",
        "fast",
        "-crf",
        "23",
        "-c:a",
        "aac",
        "-b:a",
        "160000",
        "-shortest",
        "-f",
        "mp4",
        &destination.display().to_string(),
    ]);
    ffmpeg::run(&args, "Drawing a waveform", ffmpeg::probe_duration(source), progress, cancel)
}

fn video_only(format: FileFormat) -> Vec<String> {
    match format {
        FileFormat::Webm => args(&["-c:v", "libvpx-vp9", "-crf", "40", "-b:v", "0", "-f", "webm"]),
        FileFormat::Avi => args(&["-c:v", "mpeg4", "-q:v", "8", "-f", "avi"]),
        FileFormat::Wmv => args(&["-c:v", "wmv2", "-q:v", "8", "-f", "asf"]),
        FileFormat::Gif => args(&["-f", "gif"]),
        FileFormat::Mkv => args(&["-c:v", "libx264", "-preset", "fast", "-crf", "28", "-pix_fmt", "yuv420p", "-f", "matroska"]),
        FileFormat::Mov => args(&["-c:v", "libx264", "-preset", "fast", "-crf", "28", "-pix_fmt", "yuv420p", "-f", "mov"]),
        _ => args(&["-c:v", "libx264", "-preset", "fast", "-crf", "28", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-f", "mp4"]),
    }
}

fn finish(destination: &Path, result: Result<()>) -> Result<()> {
    if result.is_err() {
        let _ = std::fs::remove_file(destination);
    }
    result
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_follow_the_file_kind() {
        assert!(accepts(FileTool::Background, FileFormat::Png));
        assert!(!accepts(FileTool::Background, FileFormat::Mp4));
        assert!(accepts(FileTool::Mute, FileFormat::Mp4));
        assert!(accepts(FileTool::Mute, FileFormat::Gif));
        assert!(!accepts(FileTool::Mute, FileFormat::Mp3));
        assert!(accepts(FileTool::Normalize, FileFormat::Wav));
        assert!(!accepts(FileTool::Compress, FileFormat::Pdf));
        assert_eq!(FileTool::Snapshot.extension(FileFormat::Mov), "jpg");
        assert_eq!(FileTool::Visualize.extension(FileFormat::Mp3), "mp4");
        assert_eq!(FileTool::Mute.extension(FileFormat::Mkv), "mkv");
        assert_eq!(FileTool::Compress.file_label(), "compressed");
    }
}
