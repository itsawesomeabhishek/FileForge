use std::fmt::Display;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::ImageEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::codecs::webp::WebPEncoder;
use serde::{Deserialize, Serialize};
use image::metadata::Orientation;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageFormat, ImageReader, RgbImage, RgbaImage};

use super::{pdf, ConversionEngine, ConversionRequest};
use crate::error::{ForgeError, Result};
use crate::ffmpeg::{self, CancelToken, ProgressFn};
use crate::formats::{FileFormat, FormatCategory};

pub struct ImageEngine;

/// Image tools offered on the Ctrl wheel. Crop, redact, edit, and
/// annotate need their own editor and are not offered until that exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageTool {
    Compress,
    Metadata,
    Background,
}

impl ImageTool {
    /// Extension of the new file. Metadata keeps a format this engine can write.
    pub fn extension(self, source: FileFormat) -> &'static str {
        match self {
            Self::Background => "jpg",
            Self::Compress => match source {
                FileFormat::Png | FileFormat::Webp | FileFormat::Gif => "png",
                _ => "jpg",
            },
            Self::Metadata => match source {
                FileFormat::Jpg | FileFormat::Png | FileFormat::Webp | FileFormat::Tiff | FileFormat::Bmp => {
                    source.file_extension()
                }
                _ => "png",
            },
        }
    }
}

/// Targets this engine can write. HEIC is intentionally absent: neither the
/// image crate nor the bundled FFmpeg can encode it.
fn writable(target: FileFormat) -> bool {
    matches!(
        target,
        FileFormat::Jpg
            | FileFormat::Png
            | FileFormat::Webp
            | FileFormat::Tiff
            | FileFormat::Bmp
            | FileFormat::Avif
            | FileFormat::Pdf
    )
}

impl ConversionEngine for ImageEngine {
    fn name(&self) -> &'static str {
        "ImageEngine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        from.category() == FormatCategory::Image && writable(to)
    }

    fn requires_ffmpeg(&self, from: FileFormat, to: FileFormat) -> bool {
        matches!(from, FileFormat::Heic | FileFormat::Avif) || to == FileFormat::Avif
    }

    fn convert(&self, request: &ConversionRequest) -> Result<()> {
        let progress = &request.progress;
        let cancel = &request.cancel;

        let decoded = decode(&request.source, request.source_format, progress, cancel)?;
        progress(0.4);
        cancel.check()?;

        match request.target {
            FileFormat::Jpg => {
                let rgb = flatten_on_white(&decoded);
                let bytes = encode_jpeg(&rgb, request.options.jpeg_quality)?;
                std::fs::write(&request.destination, bytes)?;
            }
            FileFormat::Png => decoded
                .save_with_format(&request.destination, ImageFormat::Png)
                .map_err(encode_error)?,
            FileFormat::Tiff => decoded
                .save_with_format(&request.destination, ImageFormat::Tiff)
                .map_err(encode_error)?,
            FileFormat::Bmp => decoded
                .save_with_format(&request.destination, ImageFormat::Bmp)
                .map_err(encode_error)?,
            FileFormat::Webp => write_webp(&decoded, &request.destination)?,
            FileFormat::Pdf => {
                let rgb = flatten_on_white(&decoded);
                let bytes = encode_jpeg(&rgb, request.options.jpeg_quality)?;
                let pdf = pdf::image_pdf(&bytes, rgb.width(), rgb.height());
                std::fs::write(&request.destination, pdf)?;
            }
            FileFormat::Avif => write_avif(&decoded, &request.destination, progress, cancel)?,
            other => {
                return Err(ForgeError::UnsupportedConversion(
                    request.source_format,
                    other,
                ));
            }
        }
        progress(1.0);
        Ok(())
    }
}

pub fn apply_image_tool(
    source: &Path,
    format: FileFormat,
    tool: ImageTool,
    destination: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    let decoded = decode(source, format, progress, cancel)?;
    progress(0.55);
    cancel.check()?;
    match tool {
        ImageTool::Compress if destination.extension().and_then(|ext| ext.to_str()) == Some("png") => {
            write_png_compressed(&decoded, destination)?;
        }
        ImageTool::Compress | ImageTool::Background => {
            let rgb = flatten_on_white(&decoded);
            let quality = if tool == ImageTool::Compress { 0.6 } else { 0.92 };
            let bytes = encode_jpeg(&rgb, quality)?;
            std::fs::write(destination, bytes)?;
        }
        ImageTool::Metadata => write_without_metadata(&decoded, destination)?,
    }
    progress(1.0);
    Ok(())
}

fn write_png_compressed(image: &DynamicImage, destination: &Path) -> Result<()> {
    let rgba = image.to_rgba8();
    let file = File::create(destination)?;
    let encoder = PngEncoder::new_with_quality(BufWriter::new(file), CompressionType::Best, FilterType::Adaptive);
    encoder
        .write_image(rgba.as_raw(), rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
        .map_err(encode_error)?;
    Ok(())
}

fn write_without_metadata(image: &DynamicImage, destination: &Path) -> Result<()> {
    match destination.extension().and_then(|ext| ext.to_str()).unwrap_or("png") {
        "jpg" | "jpeg" => {
            let rgb = flatten_on_white(image);
            let bytes = encode_jpeg(&rgb, 0.92)?;
            std::fs::write(destination, bytes)?;
        }
        "webp" => write_webp(image, destination)?,
        "tiff" | "tif" => image.save_with_format(destination, ImageFormat::Tiff).map_err(encode_error)?,
        "bmp" => image.save_with_format(destination, ImageFormat::Bmp).map_err(encode_error)?,
        _ => image.save_with_format(destination, ImageFormat::Png).map_err(encode_error)?,
    }
    Ok(())
}

fn decode_error(error: impl Display) -> ForgeError {
    ForgeError::InvalidInput(format!("Could not read the image: {error}"))
}

fn encode_error(error: impl Display) -> ForgeError {
    ForgeError::ToolFailed("image encoder".into(), error.to_string())
}

fn decode(
    source: &Path,
    format: FileFormat,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<DynamicImage> {
    match format {
        FileFormat::Svg => render_svg(source),
        FileFormat::Heic | FileFormat::Avif => decode_with_ffmpeg(source, progress, cancel),
        _ => decode_native(source),
    }
}

fn decode_native(source: &Path) -> Result<DynamicImage> {
    let reader = ImageReader::open(source)
        .map_err(decode_error)?
        .with_guessed_format()
        .map_err(decode_error)?;
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// HEIC/AVIF are decoded by FFmpeg into a temporary PNG.
fn decode_with_ffmpeg(
    source: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<DynamicImage> {
    let temp = std::env::temp_dir().join(format!("fileforge-{}.png", uuid::Uuid::new_v4()));
    let args = vec![
        "-i".to_string(),
        source.display().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        temp.display().to_string(),
    ];
    let result = ffmpeg::run(&args, "Reading this image", None, progress, cancel)
        .and_then(|()| decode_native(&temp));
    let _ = std::fs::remove_file(&temp);
    result
}

/// Rasterises an SVG; small drawings are scaled up so the output is crisp.
fn render_svg(source: &Path) -> Result<DynamicImage> {
    use resvg::{tiny_skia, usvg};

    let data = std::fs::read(source)?;
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(&data, &options).map_err(decode_error)?;
    let size = tree.size();
    let long_edge = size.width().max(size.height());
    if !(long_edge.is_finite() && long_edge > 0.0) {
        return Err(ForgeError::InvalidInput("The SVG has no size.".into()));
    }
    let scale = (1024.0 / long_edge).clamp(1.0, 64.0);
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| ForgeError::InvalidInput("The SVG is too large to render.".into()))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
    for pixel in pixmap.pixels() {
        let color = pixel.demultiply();
        rgba.extend_from_slice(&[color.red(), color.green(), color.blue(), color.alpha()]);
    }
    let image = RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| ForgeError::InvalidInput("Could not rasterise the SVG.".into()))?;
    Ok(DynamicImage::ImageRgba8(image))
}

/// Composites onto white so formats without alpha do not turn transparency black.
pub fn flatten_on_white(image: &DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    let mut out = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = u32::from(pixel[3]);
        let blend = |channel: u8| -> u8 {
            ((u32::from(channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
        };
        out.put_pixel(x, y, image::Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]));
    }
    out
}

fn encode_jpeg(image: &RgbImage, quality: f64) -> Result<Vec<u8>> {
    let quality = (quality * 100.0).round().clamp(1.0, 100.0) as u8;
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode(image.as_raw(), image.width(), image.height(), ExtendedColorType::Rgb8)
        .map_err(encode_error)?;
    Ok(bytes)
}

fn write_webp(image: &DynamicImage, destination: &Path) -> Result<()> {
    let rgba = image.to_rgba8();
    let mut writer = BufWriter::new(File::create(destination)?);
    WebPEncoder::new_lossless(&mut writer)
        .encode(rgba.as_raw(), rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
        .map_err(encode_error)?;
    writer.flush()?;
    Ok(())
}

/// AVIF is encoded by FFmpeg (libaom) from a temporary PNG.
fn write_avif(
    image: &DynamicImage,
    destination: &Path,
    progress: &ProgressFn,
    cancel: &CancelToken,
) -> Result<()> {
    let encoders = ffmpeg::available_encoders();
    if !encoders.contains("libaom-av1") {
        return Err(ForgeError::ToolFailed(
            "ffmpeg".into(),
            "this FFmpeg build has no AVIF (libaom-av1) encoder".into(),
        ));
    }
    let temp = std::env::temp_dir().join(format!("fileforge-{}.png", uuid::Uuid::new_v4()));
    image
        .save_with_format(&temp, ImageFormat::Png)
        .map_err(encode_error)?;
    let args = vec![
        "-i".to_string(),
        temp.display().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-c:v".to_string(),
        "libaom-av1".to_string(),
        "-crf".to_string(),
        "30".to_string(),
        "-b:v".to_string(),
        "0".to_string(),
        "-cpu-used".to_string(),
        "6".to_string(),
        "-f".to_string(),
        "avif".to_string(),
        destination.display().to_string(),
    ];
    let result = ffmpeg::run(&args, "Creating AVIF", None, progress, cancel);
    let _ = std::fs::remove_file(&temp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn supports_image_targets_but_not_heic_output() {
        let engine = ImageEngine;
        assert!(engine.supports(FileFormat::Png, FileFormat::Jpg));
        assert!(engine.supports(FileFormat::Svg, FileFormat::Pdf));
        assert!(engine.supports(FileFormat::Heic, FileFormat::Png));
        assert!(!engine.supports(FileFormat::Png, FileFormat::Heic));
        assert!(!engine.supports(FileFormat::Png, FileFormat::Docx));
        assert!(!engine.supports(FileFormat::Gif, FileFormat::Png));
    }

    #[test]
    fn transparency_flattens_to_white() {
        let mut image = RgbaImage::new(2, 1);
        image.put_pixel(0, 0, Rgba([255, 0, 0, 0]));
        image.put_pixel(1, 0, Rgba([10, 20, 30, 255]));
        let flat = flatten_on_white(&DynamicImage::ImageRgba8(image));
        assert_eq!(flat.get_pixel(0, 0).0, [255, 255, 255]);
        assert_eq!(flat.get_pixel(1, 0).0, [10, 20, 30]);
    }

    #[test]
    fn jpeg_quality_changes_size() {
        let mut image = RgbImage::new(64, 64);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x * 4) as u8, (y * 4) as u8, ((x * y) % 255) as u8]);
        }
        let high = encode_jpeg(&image, 0.95).unwrap();
        let low = encode_jpeg(&image, 0.2).unwrap();
        assert!(high.len() > low.len());
        assert_eq!(&high[..2], &[0xFF, 0xD8]);
    }
}
