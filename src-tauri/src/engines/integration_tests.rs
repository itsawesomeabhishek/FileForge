//! End-to-end conversions through `engines::convert` using generated samples.
//! Tests that need FFmpeg skip themselves when it is not installed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{convert, ConversionRequest, ConvertOptions};
use crate::error::ExploitError;
use crate::ffmpeg::{self, CancelToken, ProgressFn};
use crate::formats::FileFormat;
use crate::matrix;

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("exploit-it-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn noop_progress() -> ProgressFn {
    Arc::new(|_| {})
}

fn ffmpeg_or_skip() -> bool {
    if ffmpeg::is_available() {
        true
    } else {
        eprintln!("skipping: ffmpeg not available");
        false
    }
}

fn make_sample(args: &[&str], output: &Path) {
    let mut full: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    full.push(output.display().to_string());
    ffmpeg::run(&full, "sample", None, &noop_progress(), &CancelToken::new())
        .unwrap_or_else(|e| panic!("could not create sample {}: {e}", output.display()));
}

fn wav_sample(ws: &Workspace) -> PathBuf {
    let path = ws.path("tone.wav");
    make_sample(&["-f", "lavfi", "-i", "sine=frequency=440:duration=2"], &path);
    path
}

fn mp4_sample(ws: &Workspace) -> PathBuf {
    let path = ws.path("clip.mp4");
    make_sample(
        &[
            "-f", "lavfi", "-i", "testsrc=duration=2:size=320x240:rate=15", "-f", "lavfi", "-i",
            "sine=frequency=440:duration=2", "-shortest", "-c:v", "libx264", "-pix_fmt", "yuv420p",
            "-c:a", "aac",
        ],
        &path,
    );
    path
}

fn png_sample(ws: &Workspace) -> PathBuf {
    let path = ws.path("picture.png");
    let mut image = image::RgbaImage::new(64, 48);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = image::Rgba([(x * 4) as u8, (y * 5) as u8, 128, if x < 8 { 0 } else { 255 }]);
    }
    image.save(&path).unwrap();
    path
}

fn run_convert(
    ws: &Workspace,
    source: &Path,
    target: FileFormat,
    cancel: CancelToken,
) -> Result<PathBuf, ExploitError> {
    let source_format = FileFormat::detect(source).expect("known source format");
    let destination = matrix::output_path(source, target, &ws.0, &HashSet::new());
    let request = ConversionRequest {
        source: source.to_path_buf(),
        source_format,
        target,
        destination: destination.clone(),
        progress: noop_progress(),
        cancel,
        options: ConvertOptions::default(),
    };
    convert(&request)?;
    Ok(destination)
}

fn ok(ws: &Workspace, source: &Path, target: FileFormat) -> PathBuf {
    let out = run_convert(ws, source, target, CancelToken::new())
        .unwrap_or_else(|e| panic!("{:?} -> {target:?} failed: {e}", source.file_name()));
    assert!(out.is_file(), "{} missing", out.display());
    assert!(std::fs::metadata(&out).unwrap().len() > 0, "{} empty", out.display());
    out
}

fn no_temp_files_left(ws: &Workspace) {
    let leftovers: Vec<_> = std::fs::read_dir(&ws.0)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".exploit-"))
        .collect();
    assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");
}

#[test]
fn audio_to_every_audio_format() {
    if !ffmpeg_or_skip() {
        return;
    }
    let ws = Workspace::new();
    let wav = wav_sample(&ws);
    for target in matrix::targets_for(FileFormat::Wav) {
        let out = ok(&ws, &wav, target);
        let duration = ffmpeg::probe_duration(&out)
            .unwrap_or_else(|| panic!("{target:?} output is not playable media"));
        assert!(duration > 1.5 && duration < 2.6, "{target:?} duration {duration}");
    }
    no_temp_files_left(&ws);
}

#[test]
fn video_to_every_video_format_gif_and_mp3() {
    if !ffmpeg_or_skip() {
        return;
    }
    let ws = Workspace::new();
    let mp4 = mp4_sample(&ws);
    for target in matrix::targets_for(FileFormat::Mp4) {
        let out = ok(&ws, &mp4, target);
        let duration = ffmpeg::probe_duration(&out)
            .unwrap_or_else(|| panic!("{target:?} output is not playable media"));
        assert!(duration > 1.0 && duration < 3.0, "{target:?} duration {duration}");
    }
    no_temp_files_left(&ws);
}

#[test]
fn gif_output_has_gif_header_and_gif_converts_back() {
    if !ffmpeg_or_skip() {
        return;
    }
    let ws = Workspace::new();
    let mp4 = mp4_sample(&ws);
    let gif = ok(&ws, &mp4, FileFormat::Gif);
    assert_eq!(&std::fs::read(&gif).unwrap()[..3], b"GIF");
    let back = ok(&ws, &gif, FileFormat::Mp4);
    assert!(ffmpeg::probe_duration(&back).is_some());
}

#[test]
fn images_convert_to_all_offered_targets() {
    let ws = Workspace::new();
    let png = png_sample(&ws);
    let has_avif = ffmpeg::available_encoders().contains("libaom-av1");
    for target in matrix::targets_for(FileFormat::Png) {
        if target == FileFormat::Avif && !has_avif {
            continue;
        }
        let out = ok(&ws, &png, target);
        let bytes = std::fs::read(&out).unwrap();
        match target {
            FileFormat::Jpg => assert_eq!(&bytes[..2], &[0xFF, 0xD8]),
            FileFormat::Png => assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G']),
            FileFormat::Webp => assert_eq!(&bytes[..4], b"RIFF"),
            FileFormat::Bmp => assert_eq!(&bytes[..2], b"BM"),
            FileFormat::Tiff => assert!(&bytes[..2] == b"II" || &bytes[..2] == b"MM"),
            FileFormat::Pdf => assert_eq!(&bytes[..5], b"%PDF-"),
            FileFormat::Avif => assert_eq!(&bytes[4..8], b"ftyp"),
            other => panic!("unexpected target {other:?}"),
        }
    }
    no_temp_files_left(&ws);
}

#[test]
fn jpg_round_trips_through_image_decoder() {
    let ws = Workspace::new();
    let png = png_sample(&ws);
    let jpg = ok(&ws, &png, FileFormat::Jpg);
    let decoded = image::open(&jpg).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (64, 48));
    // Transparent column flattens to white rather than black.
    let pixel = decoded.to_rgb8().get_pixel(0, 0).0;
    assert!(pixel.iter().all(|c| *c > 240), "{pixel:?}");
    let webp = ok(&ws, &jpg, FileFormat::Webp);
    assert!(image::open(&webp).is_ok());
}

#[test]
fn svg_renders_to_png() {
    let ws = Workspace::new();
    let svg = ws.path("shape.svg");
    std::fs::write(
        &svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><rect width="100" height="50" fill="#ff5c00"/></svg>"##,
    )
    .unwrap();
    let png = ok(&ws, &svg, FileFormat::Png);
    let decoded = image::open(&png).unwrap();
    assert!(decoded.width() >= 1024);
    assert_eq!(decoded.width() / decoded.height(), 2);
    assert_eq!(decoded.to_rgba8().get_pixel(5, 5).0, [255, 92, 0, 255]);
}

#[test]
fn subtitle_and_text_conversions() {
    let ws = Workspace::new();
    let srt = ws.path("movie.srt");
    std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n").unwrap();
    let vtt = ok(&ws, &srt, FileFormat::Vtt);
    assert!(std::fs::read_to_string(&vtt).unwrap().starts_with("WEBVTT"));
    let txt = ok(&ws, &vtt, FileFormat::Txt);
    assert_eq!(std::fs::read_to_string(&txt).unwrap(), "Hello\n");
    let srt_again = ok(&ws, &txt, FileFormat::Srt);
    assert!(std::fs::read_to_string(&srt_again).unwrap().contains("-->"));
    let pdf = ok(&ws, &txt, FileFormat::Pdf);
    assert_eq!(&std::fs::read(&pdf).unwrap()[..5], b"%PDF-");
    no_temp_files_left(&ws);
}

#[test]
fn originals_are_untouched_and_never_overwritten() {
    let ws = Workspace::new();
    let png = png_sample(&ws);
    let before = std::fs::read(&png).unwrap();
    let first = ok(&ws, &png, FileFormat::Jpg);
    let second = ok(&ws, &png, FileFormat::Jpg);
    assert_ne!(first, second);
    assert_eq!(second.file_name().unwrap(), "picture 2.jpg");
    assert_eq!(std::fs::read(&png).unwrap(), before);
}

#[test]
fn failure_cleans_up_and_reports() {
    let ws = Workspace::new();
    let bad = ws.path("broken.png");
    std::fs::write(&bad, b"definitely not a png").unwrap();
    let error = run_convert(&ws, &bad, FileFormat::Jpg, CancelToken::new()).unwrap_err();
    assert!(matches!(error, ExploitError::InvalidInput(_)), "{error}");
    no_temp_files_left(&ws);
    assert!(!ws.path("broken.jpg").exists());
}

#[test]
fn cancelled_job_leaves_nothing_behind() {
    if !ffmpeg_or_skip() {
        return;
    }
    let ws = Workspace::new();
    let mp4 = mp4_sample(&ws);
    let cancel = CancelToken::new();
    cancel.cancel();
    let error = run_convert(&ws, &mp4, FileFormat::Mkv, cancel).unwrap_err();
    assert!(matches!(error, ExploitError::Cancelled));
    no_temp_files_left(&ws);
    assert!(!ws.path("clip.mkv").exists());
}

#[test]
fn unsupported_pair_is_rejected() {
    let ws = Workspace::new();
    let png = png_sample(&ws);
    let error = run_convert(&ws, &png, FileFormat::Docx, CancelToken::new()).unwrap_err();
    assert!(matches!(error, ExploitError::UnsupportedConversion(_, _)));
}
