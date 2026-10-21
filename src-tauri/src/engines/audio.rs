use std::path::Path;

use super::{ConversionEngine, ConversionRequest};
use crate::error::Result;
use crate::ffmpeg::{self, CancelToken, ProgressFn};
use crate::formats::{FileFormat, FormatCategory};

pub struct AudioEngine;

impl ConversionEngine for AudioEngine {
    fn name(&self) -> &'static str {
        "AudioEngine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        from.category() == FormatCategory::Audio && to.category() == FormatCategory::Audio
    }

    fn requires_ffmpeg(&self, _from: FileFormat, _to: FileFormat) -> bool {
        true
    }

    fn convert(&self, request: &ConversionRequest) -> Result<()> {
        ffmpeg_convert(
            &request.source,
            &request.destination,
            request.target,
            &[],
            None,
            &request.progress,
            &request.cancel,
        )
    }
}

pub fn codec_arguments(target: FileFormat, bitrate: Option<u32>) -> Vec<String> {
    let strings = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match target {
        FileFormat::Mp3 => strings(&["-c:a", "libmp3lame", "-b:a", &bitrate.unwrap_or(192_000).to_string()]),
        FileFormat::M4a => strings(&[
            "-c:a",
            "aac",
            "-b:a",
            &bitrate.unwrap_or(192_000).to_string(),
            "-movflags",
            "+faststart",
        ]),
        FileFormat::Wav => strings(&["-c:a", "pcm_s16le"]),
        FileFormat::Aiff => strings(&["-c:a", "pcm_s16be"]),
        FileFormat::Flac => strings(&["-c:a", "flac"]),
        FileFormat::Ogg => strings(&["-c:a", "libvorbis", "-q:a", "5"]),
        FileFormat::Opus => strings(&["-c:a", "libopus", "-b:a", &bitrate.unwrap_or(128_000).to_string()]),
        FileFormat::Wma => strings(&["-c:a", "wmav2", "-b:a", &bitrate.unwrap_or(192_000).to_string()]),
        _ => Vec::new(),
    }
}

/// Like `codec_arguments`, but falls back to Opus in an Ogg container when
/// this FFmpeg build has no Vorbis encoder.
pub fn resolved_codec_arguments(target: FileFormat, bitrate: Option<u32>) -> Vec<String> {
    if target == FileFormat::Ogg {
        let encoders = ffmpeg::available_encoders();
        if !encoders.contains("libvorbis") && encoders.contains("libopus") {
            return ["-c:a", "libopus", "-b:a", &bitrate.unwrap_or(128_000).to_string()]
                .iter()
                .map(|s| s.to_string())
                .collect();
        }
    }
    codec_arguments(target, bitrate)
}

pub fn container_arguments(target: FileFormat) -> Vec<String> {
    let format = match target {
        FileFormat::M4a => "ipod",
        FileFormat::Ogg => "ogg",
        FileFormat::Opus => "opus",
        FileFormat::Wma => "asf",
        FileFormat::Aiff => "aiff",
        FileFormat::Wav => "wav",
        FileFormat::Mp3 => "mp3",
        FileFormat::Flac => "flac",
        _ => return Vec::new(),
    };
    vec!["-f".to_string(), format.to_string()]
}

pub fn ffmpeg_convert(
    source: &Path,
    destination: &Path,
    target: FileFormat,
    extra_filters: &[String],
    bitrate: Option<u32>,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    let duration = ffmpeg::probe_duration(source);
    let mut args: Vec<String> = vec![
        "-i".into(),
        source.display().to_string(),
        "-vn".into(),
        "-map_metadata".into(),
        "0".into(),
    ];
    if !extra_filters.is_empty() {
        args.push("-af".into());
        args.push(extra_filters.join(","));
    }
    args.extend(resolved_codec_arguments(target, bitrate));
    args.extend(container_arguments(target));
    args.push(destination.display().to_string());
    ffmpeg::run(
        &args,
        &format!("Converting to {}", target.display_name()),
        duration,
        progress,
        cancel,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_only_audio_pairs() {
        let engine = AudioEngine;
        assert!(engine.supports(FileFormat::Wav, FileFormat::Mp3));
        assert!(!engine.supports(FileFormat::Mp4, FileFormat::Mp3));
        assert!(!engine.supports(FileFormat::Wav, FileFormat::Png));
    }

    #[test]
    fn mp3_uses_lame_at_192k() {
        assert_eq!(
            codec_arguments(FileFormat::Mp3, None),
            vec!["-c:a", "libmp3lame", "-b:a", "192000"]
        );
    }

    #[test]
    fn containers_are_explicit() {
        assert_eq!(container_arguments(FileFormat::M4a), vec!["-f", "ipod"]);
        assert!(container_arguments(FileFormat::Mp4).is_empty());
    }
}
