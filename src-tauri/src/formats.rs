use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatCategory {
    Image,
    Audio,
    Video,
    Document,
    Subtitle,
    Archive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Jpg,
    Png,
    Webp,
    Heic,
    Tiff,
    Svg,
    Avif,
    Bmp,
    Gif,
    Mp3,
    M4a,
    Wav,
    Flac,
    Ogg,
    Opus,
    Aiff,
    Wma,
    Mp4,
    Mov,
    Mkv,
    Webm,
    Avi,
    Wmv,
    Pdf,
    Docx,
    Txt,
    Srt,
    Vtt,
    Zip,
    Tar,
    Gz,
    Rar,
}

impl FileFormat {
    pub const ALL: [FileFormat; 32] = [
        Self::Jpg,
        Self::Png,
        Self::Webp,
        Self::Heic,
        Self::Tiff,
        Self::Svg,
        Self::Avif,
        Self::Bmp,
        Self::Gif,
        Self::Mp3,
        Self::M4a,
        Self::Wav,
        Self::Flac,
        Self::Ogg,
        Self::Opus,
        Self::Aiff,
        Self::Wma,
        Self::Mp4,
        Self::Mov,
        Self::Mkv,
        Self::Webm,
        Self::Avi,
        Self::Wmv,
        Self::Pdf,
        Self::Docx,
        Self::Txt,
        Self::Srt,
        Self::Vtt,
        Self::Zip,
        Self::Tar,
        Self::Gz,
        Self::Rar,
    ];

    pub const IMAGE_TARGETS: [FileFormat; 8] = [
        Self::Jpg,
        Self::Png,
        Self::Webp,
        Self::Heic,
        Self::Tiff,
        Self::Avif,
        Self::Bmp,
        Self::Pdf,
    ];
    pub const AUDIO: [FileFormat; 8] = [
        Self::Mp3,
        Self::M4a,
        Self::Wav,
        Self::Flac,
        Self::Ogg,
        Self::Opus,
        Self::Aiff,
        Self::Wma,
    ];
    pub const VIDEO: [FileFormat; 6] = [
        Self::Mp4,
        Self::Mov,
        Self::Mkv,
        Self::Webm,
        Self::Avi,
        Self::Wmv,
    ];
    pub const SUBTITLES: [FileFormat; 2] = [Self::Srt, Self::Vtt];

    pub fn category(self) -> FormatCategory {
        match self {
            Self::Jpg | Self::Png | Self::Webp | Self::Heic | Self::Tiff | Self::Svg
            | Self::Avif | Self::Bmp => FormatCategory::Image,
            Self::Gif => FormatCategory::Video,
            Self::Mp3 | Self::M4a | Self::Wav | Self::Flac | Self::Ogg | Self::Opus
            | Self::Aiff | Self::Wma => FormatCategory::Audio,
            Self::Mp4 | Self::Mov | Self::Mkv | Self::Webm | Self::Avi | Self::Wmv => {
                FormatCategory::Video
            }
            Self::Pdf | Self::Docx | Self::Txt => FormatCategory::Document,
            Self::Srt | Self::Vtt => FormatCategory::Subtitle,
            Self::Zip | Self::Tar | Self::Gz | Self::Rar => FormatCategory::Archive,
        }
    }

    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Jpg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Heic => "heic",
            Self::Tiff => "tiff",
            Self::Svg => "svg",
            Self::Avif => "avif",
            Self::Bmp => "bmp",
            Self::Gif => "gif",
            Self::Mp3 => "mp3",
            Self::M4a => "m4a",
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
            Self::Aiff => "aiff",
            Self::Wma => "wma",
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::Mkv => "mkv",
            Self::Webm => "webm",
            Self::Avi => "avi",
            Self::Wmv => "wmv",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Txt => "txt",
            Self::Srt => "srt",
            Self::Vtt => "vtt",
            Self::Zip => "zip",
            Self::Tar => "tar",
            Self::Gz => "gz",
            Self::Rar => "rar",
        }
    }

    pub fn display_name(self) -> String {
        match self {
            Self::Gz => "GZIP".to_string(),
            other => other.raw_value().to_uppercase(),
        }
    }

    pub fn file_extension(self) -> &'static str {
        match self {
            Self::Gz => "tar.gz",
            other => other.raw_value(),
        }
    }

    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Jpg => &["jpg", "jpeg", "jpe", "jfif"],
            Self::Tiff => &["tiff", "tif"],
            Self::Heic => &["heic", "heif"],
            Self::M4a => &["m4a", "aac", "m4b"],
            Self::Mp4 => &["mp4", "m4v"],
            Self::Mov => &["mov", "qt"],
            Self::Gz => &["gz", "tgz"],
            Self::Txt => &["txt", "text", "md", "log"],
            Self::Aiff => &["aiff", "aif"],
            Self::Png => &["png"],
            Self::Webp => &["webp"],
            Self::Svg => &["svg"],
            Self::Avif => &["avif"],
            Self::Bmp => &["bmp"],
            Self::Gif => &["gif"],
            Self::Mp3 => &["mp3"],
            Self::Wav => &["wav"],
            Self::Flac => &["flac"],
            Self::Ogg => &["ogg"],
            Self::Opus => &["opus"],
            Self::Wma => &["wma"],
            Self::Mkv => &["mkv"],
            Self::Webm => &["webm"],
            Self::Avi => &["avi"],
            Self::Wmv => &["wmv"],
            Self::Pdf => &["pdf"],
            Self::Docx => &["docx"],
            Self::Srt => &["srt"],
            Self::Vtt => &["vtt"],
            Self::Zip => &["zip"],
            Self::Tar => &["tar"],
            Self::Rar => &["rar"],
        }
    }

    pub fn from_extension(extension: &str) -> Option<FileFormat> {
        let lowered = extension.trim_start_matches('.').to_lowercase();
        Self::ALL
            .into_iter()
            .find(|format| format.aliases().contains(&lowered.as_str()))
    }

    /// Detects a format from a file name; recognises `.tar.gz`.
    pub fn detect(path: &Path) -> Option<FileFormat> {
        let name = path.file_name()?.to_str()?.to_lowercase();
        if name.ends_with(".tar.gz") {
            return Some(Self::Gz);
        }
        let extension = path.extension()?.to_str()?;
        Self::from_extension(extension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aliases() {
        assert_eq!(FileFormat::detect(Path::new("a.JPEG")), Some(FileFormat::Jpg));
        assert_eq!(FileFormat::detect(Path::new("a.tif")), Some(FileFormat::Tiff));
        assert_eq!(FileFormat::detect(Path::new("a.m4v")), Some(FileFormat::Mp4));
        assert_eq!(FileFormat::detect(Path::new("a.md")), Some(FileFormat::Txt));
        assert_eq!(FileFormat::detect(Path::new("a.tar.gz")), Some(FileFormat::Gz));
        assert_eq!(FileFormat::detect(Path::new("a.xyz")), None);
        assert_eq!(FileFormat::detect(Path::new("noext")), None);
    }

    #[test]
    fn every_format_has_a_matching_alias() {
        for format in FileFormat::ALL {
            assert!(
                format.aliases().contains(&format.raw_value()),
                "{format:?} missing own alias"
            );
        }
    }

    #[test]
    fn gif_is_video_category() {
        assert_eq!(FileFormat::Gif.category(), FormatCategory::Video);
    }

    #[test]
    fn gz_uses_tar_gz_extension() {
        assert_eq!(FileFormat::Gz.file_extension(), "tar.gz");
        assert_eq!(FileFormat::Gz.display_name(), "GZIP");
    }
}
