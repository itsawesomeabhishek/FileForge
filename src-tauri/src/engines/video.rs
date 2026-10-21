use std::path::Path;

use super::{audio, ConversionEngine, ConversionRequest};
use crate::error::Result;
use crate::ffmpeg::{self, CancelToken, ProgressFn};
use crate::formats::{FileFormat, FormatCategory};

pub struct VideoEngine;

impl ConversionEngine for VideoEngine {
    fn name(&self) -> &'static str {
        "VideoEngine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        from.category() == FormatCategory::Video
            && (to.category() == FormatCategory::Video || to == FileFormat::Mp3)
    }

    fn requires_ffmpeg(&self, _from: FileFormat, _to: FileFormat) -> bool {
        true
    }

    fn convert(&self, request: &ConversionRequest) -> Result<()> {
        if request.target == FileFormat::Mp3 {
            return audio::ffmpeg_convert(
                &request.source,
                &request.destination,
                FileFormat::Mp3,
                &[],
                None,
                &request.progress,
                &request.cancel,
            );
        }
        ffmpeg_convert(
            &request.source,
            &request.destination,
            request.target,
            &[],
            None,
            None,
            &request.progress,
            &request.cancel,
        )
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

pub fn codec_arguments(
    target: FileFormat,
    video_bitrate: Option<u32>,
    audio_bitrate: Option<u32>,
) -> Vec<String> {
    let mut args = match target {
        FileFormat::Mp4 => strings(&[
            "-c:v", "libx264", "-preset", "medium", "-pix_fmt", "yuv420p", "-c:a", "aac",
            "-movflags", "+faststart", "-f", "mp4",
        ]),
        FileFormat::Mov => strings(&[
            "-c:v", "libx264", "-preset", "medium", "-pix_fmt", "yuv420p", "-c:a", "aac", "-f",
            "mov",
        ]),
        FileFormat::Mkv => strings(&[
            "-c:v", "libx264", "-preset", "medium", "-pix_fmt", "yuv420p", "-c:a", "aac", "-f",
            "matroska",
        ]),
        FileFormat::Webm => strings(&[
            "-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "32", "-row-mt", "1", "-c:a", "libopus",
            "-f", "webm",
        ]),
        FileFormat::Avi => strings(&[
            "-c:v", "mpeg4", "-vtag", "xvid", "-q:v", "4", "-c:a", "libmp3lame", "-f", "avi",
        ]),
        FileFormat::Wmv => strings(&["-c:v", "wmv2", "-q:v", "4", "-c:a", "wmav2", "-f", "asf"]),
        _ => Vec::new(),
    };
    let uses_bitrate = !matches!(target, FileFormat::Webm | FileFormat::Avi | FileFormat::Wmv);
    if let (Some(bitrate), true) = (video_bitrate, uses_bitrate) {
        args.extend(strings(&[
            "-b:v",
            &bitrate.to_string(),
            "-maxrate",
            &((bitrate as f64 * 1.4) as u64).to_string(),
            "-bufsize",
            &(u64::from(bitrate) * 2).to_string(),
        ]));
    } else if matches!(target, FileFormat::Mp4 | FileFormat::Mov | FileFormat::Mkv) {
        args.extend(strings(&["-crf", "20"]));
    }
    if let (Some(bitrate), false) = (audio_bitrate, matches!(target, FileFormat::Avi | FileFormat::Wmv)) {
        args.extend(strings(&["-b:a", &bitrate.to_string()]));
    }
    args
}

#[allow(clippy::too_many_arguments)]
pub fn ffmpeg_convert(
    source: &Path,
    destination: &Path,
    target: FileFormat,
    video_filters: &[String],
    video_bitrate: Option<u32>,
    audio_bitrate: Option<u32>,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    let duration = ffmpeg::probe_duration(source);

    if target == FileFormat::Gif {
        let mut filters: Vec<String> = video_filters.to_vec();
        filters.push("fps=12".into());
        filters.push("scale='min(640,iw)':-2:flags=lanczos".into());
        filters.push("split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5".into());
        let args = vec![
            "-i".to_string(),
            source.display().to_string(),
            "-vf".to_string(),
            filters.join(","),
            "-loop".to_string(),
            "0".to_string(),
            "-f".to_string(),
            "gif".to_string(),
            destination.display().to_string(),
        ];
        return ffmpeg::run(&args, "Creating GIF", duration, progress, cancel);
    }

    let mut args = vec![
        "-i".to_string(),
        source.display().to_string(),
        "-map_metadata".to_string(),
        "0".to_string(),
    ];
    let mut filters: Vec<String> = video_filters.to_vec();
    if matches!(
        target,
        FileFormat::Mp4 | FileFormat::Mov | FileFormat::Mkv | FileFormat::Webm
    ) {
        filters.push("scale=trunc(iw/2)*2:trunc(ih/2)*2".into());
    }
    if !filters.is_empty() {
        args.push("-vf".into());
        args.push(filters.join(","));
    }
    args.extend(codec_arguments(target, video_bitrate, audio_bitrate));
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
    fn supports_video_pairs_and_mp3() {
        let engine = VideoEngine;
        assert!(engine.supports(FileFormat::Mp4, FileFormat::Gif));
        assert!(engine.supports(FileFormat::Mov, FileFormat::Mp3));
        assert!(engine.supports(FileFormat::Gif, FileFormat::Mp4));
        assert!(!engine.supports(FileFormat::Mp4, FileFormat::Wav));
        assert!(!engine.supports(FileFormat::Png, FileFormat::Mp4));
    }

    #[test]
    fn mp4_uses_x264_with_crf() {
        let args = codec_arguments(FileFormat::Mp4, None, None);
        assert!(args.windows(2).any(|w| w == ["-c:v", "libx264"]));
        assert!(args.windows(2).any(|w| w == ["-crf", "20"]));
    }

    #[test]
    fn bitrate_replaces_crf() {
        let args = codec_arguments(FileFormat::Mp4, Some(1_000_000), Some(96_000));
        assert!(args.contains(&"-maxrate".to_string()));
        assert!(!args.contains(&"-crf".to_string()));
        assert!(args.windows(2).any(|w| w == ["-b:a", "96000"]));
    }
}
