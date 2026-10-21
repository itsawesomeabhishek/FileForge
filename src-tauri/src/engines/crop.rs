//! Crop rectangle in source pixels, plus the preview the editor draws.

use std::io::Cursor;
use std::path::Path;

use image::ImageFormat;

use super::images;
use crate::error::{ForgeError, Result};
use crate::ffmpeg::{self, CancelToken, ProgressFn};
use crate::formats::{FileFormat, FormatCategory};

/// Inclusive origin and size, in pixels of the oriented picture or video frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Pulls a requested area inside the picture. An empty area is rejected.
pub fn fit_rect(x: i64, y: i64, width: i64, height: i64, bounds_w: u32, bounds_h: u32) -> Result<CropRect> {
    if bounds_w == 0 || bounds_h == 0 {
        return Err(ForgeError::InvalidInput("This file has no picture.".into()));
    }
    if width < 1 || height < 1 {
        return Err(ForgeError::InvalidInput("The crop area is empty.".into()));
    }
    let max_w = i64::from(bounds_w);
    let max_h = i64::from(bounds_h);
    let width = width.min(max_w);
    let height = height.min(max_h);
    let x = x.clamp(0, max_w - width);
    let y = y.clamp(0, max_h - height);
    Ok(CropRect {
        x: x as u32,
        y: y as u32,
        width: width as u32,
        height: height as u32,
    })
}

/// H.264 wants even sizes. Shrinks the rect onto an even grid inside itself,
/// so the result still sits inside the frame the editor measured.
pub fn even_video_rect(rect: CropRect) -> Result<CropRect> {
    let x = rect.x / 2 * 2;
    let y = rect.y / 2 * 2;
    let width = ((rect.x + rect.width) / 2 * 2).saturating_sub(x);
    let height = ((rect.y + rect.height) / 2 * 2).saturating_sub(y);
    if width < 2 || height < 2 {
        return Err(ForgeError::InvalidInput("The crop area is too small for this video.".into()));
    }
    Ok(CropRect { x, y, width, height })
}

pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub png_base64: String,
    pub video: bool,
}

/// A PNG the editor can draw, plus the pixel size the crop numbers use.
pub fn preview(source: &Path, format: FileFormat) -> Result<Preview> {
    let progress: ProgressFn = std::sync::Arc::new(|_| {});
    let cancel = CancelToken::new();
    if format.category() == FormatCategory::Video {
        let frame = video_frame(source, &progress, &cancel)?;
        let image = image::open(&frame).map_err(|error| {
            ForgeError::InvalidInput(format!("Could not read a frame: {error}"))
        })?;
        let _ = std::fs::remove_file(&frame);
        return Ok(Preview {
            width: image.width(),
            height: image.height(),
            png_base64: png_base64(&image)?,
            video: true,
        });
    }
    let decoded = images::decode_for_crop(source, format, &progress, &cancel)?;
    Ok(Preview {
        width: decoded.width(),
        height: decoded.height(),
        png_base64: png_base64(&decoded)?,
        video: false,
    })
}

fn video_frame(source: &Path, progress: &ProgressFn, cancel: &CancelToken) -> Result<std::path::PathBuf> {
    let frame = std::env::temp_dir().join(format!("fileforge-crop-{}.png", uuid::Uuid::new_v4()));
    let duration = ffmpeg::probe_duration(source).unwrap_or(0.0);
    let at = if duration > 1.0 { "1" } else { "0" };
    let args = vec![
        "-ss".into(),
        at.into(),
        "-i".into(),
        source.display().to_string(),
        "-frames:v".into(),
        "1".into(),
        frame.display().to_string(),
    ];
    let result = ffmpeg::run(&args, "Reading a frame", None, progress, cancel);
    if result.is_err() {
        let _ = std::fs::remove_file(&frame);
    }
    result?;
    Ok(frame)
}

fn png_base64(image: &image::DynamicImage) -> Result<String> {
    let preview = if image.width().max(image.height()) > 1600 {
        image.thumbnail(1600, 1600)
    } else {
        image.clone()
    };
    let mut bytes = Vec::new();
    preview
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|error| ForgeError::ToolFailed("image encoder".into(), error.to_string()))?;
    Ok(base64_encode(&bytes))
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= data.len() {
        let n = (u32::from(data[index]) << 16) | (u32::from(data[index + 1]) << 8) | u32::from(data[index + 2]);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        index += 3;
    }
    if index < data.len() {
        let remain = data.len() - index;
        let mut n = u32::from(data[index]) << 16;
        if remain == 2 {
            n |= u32::from(data[index + 1]) << 8;
        }
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if remain == 2 {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
            out.push('=');
        } else {
            out.push('=');
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_rect_pulls_the_area_inside() {
        let rect = fit_rect(-20, 10, 500, 40, 100, 80).unwrap();
        assert_eq!(rect, CropRect { x: 0, y: 10, width: 100, height: 40 });
    }

    #[test]
    fn fit_rect_rejects_an_empty_area() {
        assert!(fit_rect(0, 0, 0, 10, 100, 100).is_err());
    }

    #[test]
    fn video_rect_is_even() {
        let rect = even_video_rect(CropRect { x: 5, y: 7, width: 101, height: 51 }).unwrap();
        assert_eq!(rect.x % 2, 0);
        assert_eq!(rect.y % 2, 0);
        assert_eq!(rect.width % 2, 0);
        assert_eq!(rect.height % 2, 0);
        assert!(rect.x + rect.width <= 5 + 101);
        assert!(rect.y + rect.height <= 7 + 51);
    }

    #[test]
    fn crops_a_png_to_the_rectangle() {
        let dir = std::env::temp_dir().join(format!("fileforge-crop-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("in.png");
        let destination = dir.join("out.png");
        image::RgbaImage::new(10, 8).save(&source).unwrap();
        let progress: ProgressFn = std::sync::Arc::new(|_| {});
        images::crop_image(
            &source,
            FileFormat::Png,
            CropRect { x: 1, y: 2, width: 4, height: 3 },
            &destination,
            &progress,
            &CancelToken::new(),
        )
        .unwrap();
        let cropped = image::open(&destination).unwrap();
        assert_eq!((cropped.width(), cropped.height()), (4, 3));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
