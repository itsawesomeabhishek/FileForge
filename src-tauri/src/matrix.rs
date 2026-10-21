use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::engines;
use crate::formats::FileFormat;

/// The conversions the macOS app lists for a source format, before filtering
/// by what this build's engines can actually do.
fn listed_targets(source: FileFormat) -> Vec<FileFormat> {
    use FileFormat::*;
    let mut list: Vec<FileFormat> = match source {
        Jpg | Png => {
            let mut v = FileFormat::IMAGE_TARGETS.to_vec();
            v.push(Docx);
            v
        }
        Webp | Heic | Tiff | Svg | Avif | Bmp => FileFormat::IMAGE_TARGETS.to_vec(),
        Mp3 | M4a | Wav | Flac | Ogg | Opus | Aiff | Wma => FileFormat::AUDIO.to_vec(),
        Mp4 | Mov | Mkv | Webm | Avi | Wmv => {
            let mut v = FileFormat::VIDEO.to_vec();
            v.push(Gif);
            v.push(Mp3);
            v
        }
        Gif => FileFormat::VIDEO.to_vec(),
        Pdf => vec![Txt, Md],
        Txt => vec![Pdf, Html, Md, Srt, Vtt],
        Md => vec![Txt, Html, Pdf],
        Html => vec![Txt, Md, Pdf],
        Csv => vec![Tsv, Pdf],
        Tsv => vec![Csv, Pdf],
        Srt | Vtt => {
            let mut v = FileFormat::SUBTITLES.to_vec();
            v.push(Txt);
            v
        }
        Zip => vec![Tar, Gz],
        Tar => vec![Zip, Gz],
        Gz => vec![Zip, Tar],
        // RAR needs a separate unpacker. DOCX, Pages, slides, and XLSX
        // are not offered until an engine can write them.
        Rar | Docx => vec![],
    };
    list.retain(|target| *target != source);
    list
}

/// Targets offered for one source format: the macOS list restricted to
/// conversions an engine really supports.
pub fn targets_for(source: FileFormat) -> Vec<FileFormat> {
    listed_targets(source)
        .into_iter()
        .filter(|target| engines::supports(source, *target))
        .collect()
}

/// Targets common to every selected source, in the first source's order.
pub fn common_targets(sources: &[FileFormat]) -> Vec<FileFormat> {
    let Some((first, rest)) = sources.split_first() else {
        return Vec::new();
    };
    let mut common = targets_for(*first);
    for source in rest {
        let next: HashSet<FileFormat> = targets_for(*source).into_iter().collect();
        common.retain(|target| next.contains(target));
    }
    common
}

pub fn stripped_base_name(file_name: &str) -> String {
    let lower = file_name.to_lowercase();
    if lower.ends_with(".tar.gz") {
        return file_name[..file_name.len() - 7].to_string();
    }
    if lower.ends_with(".tgz") {
        return file_name[..file_name.len() - 4].to_string();
    }
    match file_name.rfind('.') {
        Some(dot) if dot != 0 => file_name[..dot].to_string(),
        _ => file_name.to_string(),
    }
}

/// `name.ext`, then `name 2.ext`, `name 3.ext`... skipping names that exist on
/// disk or are already reserved by another queued job.
pub fn output_path(
    source: &Path,
    target: FileFormat,
    directory: &Path,
    existing: &HashSet<String>,
) -> PathBuf {
    let file_name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = stripped_base_name(&file_name);
    let extension = target.file_extension();
    let mut candidate = format!("{base}.{extension}");
    let mut counter = 2;
    while existing.contains(&candidate) || directory.join(&candidate).exists() {
        candidate = format!("{base} {counter}.{extension}");
        counter += 1;
    }
    directory.join(candidate)
}

/// `name label.ext`, then `name label 2.ext`, without touching the original.
pub fn named_output(
    source: &Path,
    label: &str,
    extension: &str,
    directory: &Path,
    existing: &HashSet<String>,
) -> PathBuf {
    let file_name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = stripped_base_name(&file_name);
    let mut candidate = format!("{base} {label}.{extension}");
    let mut counter = 2;
    while existing.contains(&candidate) || directory.join(&candidate).exists() {
        candidate = format!("{base} {label} {counter}.{extension}");
        counter += 1;
    }
    directory.join(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use FileFormat::*;

    #[test]
    fn strips_names() {
        assert_eq!(stripped_base_name("photo.jpg"), "photo");
        assert_eq!(stripped_base_name("a.b.png"), "a.b");
        assert_eq!(stripped_base_name("bundle.tar.gz"), "bundle");
        assert_eq!(stripped_base_name("bundle.TGZ"), "bundle");
        assert_eq!(stripped_base_name(".hidden"), ".hidden");
        assert_eq!(stripped_base_name("noext"), "noext");
    }

    #[test]
    fn never_offers_source_as_target() {
        for format in FileFormat::ALL {
            assert!(!targets_for(format).contains(&format), "{format:?}");
        }
    }

    #[test]
    fn archives_convert_between_zip_tar_and_gzip() {
        assert_eq!(targets_for(Zip), vec![Tar, Gz]);
        assert_eq!(targets_for(Tar), vec![Zip, Gz]);
        assert_eq!(targets_for(Gz), vec![Zip, Tar]);
        assert!(targets_for(Rar).is_empty());
        assert!(targets_for(Docx).is_empty());
    }

    #[test]
    fn documents_and_spreadsheets_offer_text_targets() {
        assert!(targets_for(Pdf).contains(&Txt));
        assert!(targets_for(Md).contains(&Html));
        assert!(targets_for(Csv).contains(&Tsv));
        assert!(targets_for(Tsv).contains(&Pdf));
        assert!(!targets_for(Pdf).contains(&Docx));
    }

    #[test]
    fn audio_targets_match_mac_list() {
        let targets = targets_for(Wav);
        assert_eq!(targets, vec![Mp3, M4a, Flac, Ogg, Opus, Aiff, Wma]);
    }

    #[test]
    fn video_offers_gif_and_mp3() {
        let targets = targets_for(Mp4);
        assert!(targets.contains(&Gif));
        assert!(targets.contains(&Mp3));
        assert!(targets.contains(&Mkv));
        assert!(!targets.contains(&Mp4));
    }

    #[test]
    fn gif_offers_only_video() {
        let targets = targets_for(Gif);
        assert!(targets.iter().all(|t| t.category() == crate::formats::FormatCategory::Video));
        assert!(!targets.is_empty());
    }

    #[test]
    fn subtitles_convert_between_each_other_and_text() {
        assert_eq!(targets_for(Srt), vec![Vtt, Txt]);
        assert_eq!(targets_for(Vtt), vec![Srt, Txt]);
    }

    #[test]
    fn text_offers_subtitles_and_pdf() {
        let targets = targets_for(Txt);
        assert!(targets.contains(&Pdf));
        assert!(targets.contains(&Srt));
        assert!(targets.contains(&Vtt));
    }

    #[test]
    fn common_targets_intersect() {
        let common = common_targets(&[Mp3, Wav]);
        assert!(common.contains(&Flac));
        assert!(!common.contains(&Mp3));
        assert!(!common.contains(&Wav));
        assert!(common_targets(&[Mp3, Png]).is_empty());
        assert!(common_targets(&[]).is_empty());
    }

    #[test]
    fn output_path_avoids_collisions() {
        let dir = std::env::temp_dir().join(format!("fileforge-matrix-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("song.mp3"), b"x").unwrap();
        let mut reserved = HashSet::new();
        let first = output_path(Path::new("song.wav"), Mp3, &dir, &reserved);
        assert_eq!(first.file_name().unwrap(), "song 2.mp3");
        reserved.insert("song 2.mp3".to_string());
        let second = output_path(Path::new("song.wav"), Mp3, &dir, &reserved);
        assert_eq!(second.file_name().unwrap(), "song 3.mp3");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn named_output_adds_a_label_and_avoids_collisions() {
        let dir = std::env::temp_dir().join(format!("fileforge-named-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("photo compressed.jpg"), b"x").unwrap();
        let first = named_output(Path::new("photo.png"), "compressed", "jpg", &dir, &HashSet::new());
        assert_eq!(first.file_name().unwrap(), "photo compressed 2.jpg");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
