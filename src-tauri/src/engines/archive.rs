//! ZIP, TAR, and GZIP (tar.gz). RAR is not offered. Paths inside an archive
//! that try to leave the destination are skipped.

use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::Archive;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::{ConversionEngine, ConversionRequest};
use crate::error::{ForgeError, Result};
use crate::formats::FileFormat;

pub struct ArchiveEngine;

impl ConversionEngine for ArchiveEngine {
    fn name(&self) -> &'static str {
        "ArchiveEngine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        matches!(from, FileFormat::Zip | FileFormat::Tar | FileFormat::Gz)
            && matches!(to, FileFormat::Zip | FileFormat::Tar | FileFormat::Gz)
            && from != to
    }

    fn convert(&self, request: &ConversionRequest) -> Result<()> {
        let scratch = std::env::temp_dir().join(format!("fileforge-archive-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&scratch)?;
        let result = (|| {
            request.cancel.check()?;
            extract(&request.source, request.source_format, &scratch)?;
            (request.progress)(0.45);
            request.cancel.check()?;
            pack(&scratch, request.target, &request.destination)?;
            (request.progress)(1.0);
            Ok(())
        })();
        let _ = fs::remove_dir_all(&scratch);
        result
    }
}

fn extract(source: &Path, format: FileFormat, scratch: &Path) -> Result<()> {
    match format {
        FileFormat::Zip => extract_zip(source, scratch),
        FileFormat::Tar => extract_tar_file(source, scratch),
        FileFormat::Gz => extract_gz(source, scratch),
        _ => Err(ForgeError::InvalidInput("This archive cannot be read.".into())),
    }
}

fn extract_zip(source: &Path, scratch: &Path) -> Result<()> {
    let file = File::open(source)?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        ForgeError::InvalidInput(format!("Could not read the ZIP file: {error}"))
    })?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            ForgeError::InvalidInput(format!("Could not read a ZIP entry: {error}"))
        })?;
        let Some(relative) = safe_relative(entry.name()) else {
            continue;
        };
        let target = scratch.join(&relative);
        if entry.is_dir() || entry.name().ends_with('/') {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&target)?;
        io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

fn extract_tar_file(source: &Path, scratch: &Path) -> Result<()> {
    let file = File::open(source)?;
    extract_tar(Archive::new(file), scratch)
}

fn extract_tar<R: Read>(mut archive: Archive<R>, scratch: &Path) -> Result<()> {
    let entries = archive
        .entries()
        .map_err(|error| ForgeError::InvalidInput(format!("Could not read the TAR file: {error}")))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| {
            ForgeError::InvalidInput(format!("Could not read a TAR entry: {error}"))
        })?;
        let path = entry.path().map_err(|error| {
            ForgeError::InvalidInput(format!("A TAR entry has a bad name: {error}"))
        })?;
        let Some(relative) = safe_relative(&path.to_string_lossy()) else {
            continue;
        };
        let target = scratch.join(&relative);
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if !entry.header().entry_type().is_file() {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&target)?;
        io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

fn extract_gz(source: &Path, scratch: &Path) -> Result<()> {
    let file = File::open(source)?;
    let mut decoder = GzDecoder::new(file);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).map_err(|error| {
        ForgeError::InvalidInput(format!("Could not read the GZIP file: {error}"))
    })?;
    if looks_like_tar(&bytes) {
        return extract_tar(Archive::new(Cursor::new(bytes)), scratch);
    }
    let name = single_file_name(source);
    fs::write(scratch.join(name), bytes)?;
    Ok(())
}

fn looks_like_tar(bytes: &[u8]) -> bool {
    bytes.len() > 262 && &bytes[257..262] == b"ustar"
}

fn single_file_name(source: &Path) -> String {
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let lower = name.to_lowercase();
    let stripped = lower
        .strip_suffix(".tar.gz")
        .or_else(|| lower.strip_suffix(".tgz"))
        .or_else(|| lower.strip_suffix(".gz"))
        .map(|rest| name[..rest.len()].to_string());
    let name = stripped.filter(|name| !name.is_empty()).unwrap_or(name);
    if name.contains('.') { name } else { format!("{name}.bin") }
}

fn pack(scratch: &Path, format: FileFormat, destination: &Path) -> Result<()> {
    let files = collect(scratch, scratch)?;
    match format {
        FileFormat::Zip => write_zip(&files, scratch, destination),
        FileFormat::Tar => write_tar(&files, scratch, destination),
        FileFormat::Gz => write_tar_gz(&files, scratch, destination),
        _ => Err(ForgeError::InvalidInput("This archive cannot be written.".into())),
    }
}

struct Item {
    relative: PathBuf,
    directory: bool,
}

fn collect(dir: &Path, root: &Path) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        if path.is_dir() {
            items.push(Item { relative, directory: true });
            items.extend(collect(&path, root)?);
        } else if path.is_file() {
            items.push(Item { relative, directory: false });
        }
    }
    Ok(items)
}

fn archive_name(relative: &Path, directory: bool) -> String {
    let mut name = relative.to_string_lossy().replace('\\', "/");
    if directory && !name.ends_with('/') {
        name.push('/');
    }
    name
}

fn write_zip(items: &[Item], root: &Path, destination: &Path) -> Result<()> {
    let file = File::create(destination)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for item in items {
        let name = archive_name(&item.relative, item.directory);
        if item.directory {
            zip.add_directory(&name, options).map_err(zip_error)?;
        } else {
            zip.start_file(&name, options).map_err(zip_error)?;
            let bytes = fs::read(root.join(&item.relative))?;
            zip.write_all(&bytes)?;
        }
    }
    zip.finish().map_err(zip_error)?;
    Ok(())
}

fn write_tar(items: &[Item], root: &Path, destination: &Path) -> Result<()> {
    let file = File::create(destination)?;
    let mut builder = tar::Builder::new(file);
    append_tar(&mut builder, items, root)?;
    builder.finish()?;
    Ok(())
}

fn write_tar_gz(items: &[Item], root: &Path, destination: &Path) -> Result<()> {
    let file = File::create(destination)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);
    append_tar(&mut builder, items, root)?;
    builder.finish()?;
    Ok(())
}

fn append_tar<W: Write>(builder: &mut tar::Builder<W>, items: &[Item], root: &Path) -> Result<()> {
    for item in items {
        let path = root.join(&item.relative);
        if item.directory {
            builder.append_dir(archive_name(&item.relative, true), &path)?;
        } else {
            let mut file = File::open(&path)?;
            builder.append_file(archive_name(&item.relative, false), &mut file)?;
        }
    }
    Ok(())
}

fn safe_relative(name: &str) -> Option<PathBuf> {
    let name = name.replace('\\', "/");
    if name.is_empty() || name.starts_with('/') {
        return None;
    }
    let path = PathBuf::from(name.trim_end_matches('/'));
    if path.as_os_str().is_empty() {
        return None;
    }
    if path.components().any(|component| {
        !matches!(component, Component::Normal(_))
    }) {
        return None;
    }
    Some(path)
}

fn zip_error(error: zip::result::ZipError) -> ForgeError {
    ForgeError::ToolFailed("zip".into(), error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_that_escape() {
        assert!(safe_relative("../secret").is_none());
        assert!(safe_relative("/etc/passwd").is_none());
        assert!(safe_relative("ok/note.txt").is_some());
    }

    #[test]
    fn zip_round_trips_through_tar_gz() {
        let dir = std::env::temp_dir().join(format!("fileforge-zip-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("notes.zip");
        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();
            zip.start_file("notes/hello.txt", options).unwrap();
            zip.write_all(b"hello archive").unwrap();
            zip.finish().unwrap();
        }
        let gz_path = dir.join("notes.tar.gz");
        let engine = ArchiveEngine;
        let request = ConversionRequest {
            source: zip_path,
            source_format: FileFormat::Zip,
            target: FileFormat::Gz,
            destination: gz_path.clone(),
            progress: std::sync::Arc::new(|_| {}),
            cancel: crate::ffmpeg::CancelToken::new(),
            options: Default::default(),
        };
        engine.convert(&request).unwrap();
        let scratch = dir.join("out");
        fs::create_dir_all(&scratch).unwrap();
        extract_gz(&gz_path, &scratch).unwrap();
        let text = fs::read_to_string(scratch.join("notes/hello.txt")).unwrap();
        assert_eq!(text, "hello archive");
        let _ = fs::remove_dir_all(&dir);
    }
}
