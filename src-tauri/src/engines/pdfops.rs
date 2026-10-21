//! PDF tools that finish without a page editor: compress, strip metadata, split.

use std::path::{Path, PathBuf};

use lopdf::Document;

use crate::error::{ForgeError, Result};

fn pdf_error(error: impl std::fmt::Display) -> ForgeError {
    ForgeError::ToolFailed("pdf".into(), error.to_string())
}

pub fn page_count(source: &Path) -> Result<usize> {
    let doc = Document::load(source).map_err(pdf_error)?;
    let count = doc.get_pages().len();
    if count == 0 {
        return Err(ForgeError::InvalidInput("This PDF has no pages.".into()));
    }
    Ok(count)
}

/// Rewrites the PDF with compressed streams. The original is left in place.
pub fn compress(source: &Path, destination: &Path) -> Result<()> {
    let mut doc = Document::load(source).map_err(pdf_error)?;
    doc.compress();
    doc.save(destination).map_err(pdf_error)?;
    Ok(())
}

/// Drops the info dictionary and any catalog metadata stream.
pub fn strip_metadata(source: &Path, destination: &Path) -> Result<()> {
    let mut doc = Document::load(source).map_err(pdf_error)?;
    doc.trailer.remove(b"Info");
    if let Ok(catalog) = doc.catalog_mut() {
        catalog.remove(b"Metadata");
    }
    doc.save(destination).map_err(pdf_error)?;
    Ok(())
}

/// One new PDF per page, in order. `destinations` must match the page count.
pub fn split_pages(source: &Path, destinations: &[PathBuf]) -> Result<()> {
    let template = Document::load(source).map_err(pdf_error)?;
    let pages: Vec<u32> = template.get_pages().keys().copied().collect();
    if pages.len() != destinations.len() {
        return Err(ForgeError::InvalidInput(
            "The page count changed before the split finished.".into(),
        ));
    }
    for (index, destination) in destinations.iter().enumerate() {
        let mut copy = Document::load(source).map_err(pdf_error)?;
        let drop_pages: Vec<u32> = pages
            .iter()
            .enumerate()
            .filter(|(page_index, _)| *page_index != index)
            .map(|(_, number)| *number)
            .collect();
        copy.delete_pages(&drop_pages);
        keep_live_kids(&mut copy);
        copy.prune_objects();
        copy.save(destination).map_err(pdf_error)?;
    }
    Ok(())
}

/// `delete_pages` removes the page object and leaves a dangling Kids entry.
/// Drop those entries so a reader only sees the pages that remain.
fn keep_live_kids(doc: &mut Document) {
    let live: std::collections::HashSet<lopdf::ObjectId> = doc.objects.keys().copied().collect();
    let tree_ids: Vec<lopdf::ObjectId> = doc
        .objects
        .iter()
        .filter_map(|(id, object)| {
            let dict = object.as_dict().ok()?;
            dict.type_is(b"Pages").then_some(*id)
        })
        .collect();
    for id in tree_ids {
        let Ok(dict) = doc.get_dictionary(id) else { continue };
        let Ok(kids) = dict.get(b"Kids").and_then(lopdf::Object::as_array) else {
            continue;
        };
        let kept: Vec<lopdf::Object> = kids
            .iter()
            .filter(|kid| kid.as_reference().ok().is_some_and(|kid_id| live.contains(&kid_id)))
            .cloned()
            .collect();
        let leaves = kept.iter().all(|kid| {
            kid.as_reference()
                .ok()
                .and_then(|kid_id| doc.get_dictionary(kid_id).ok())
                .is_some_and(|page| page.type_is(b"Page"))
        });
        let count = i64::try_from(kept.len()).unwrap_or(i64::MAX);
        if let Ok(dict) = doc.get_dictionary_mut(id) {
            dict.set("Kids", lopdf::Object::Array(kept));
            if leaves {
                dict.set("Count", lopdf::Object::Integer(count));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_a_one_page_pdf_without_touching_the_original() {
        let dir = std::env::temp_dir().join(format!("fileforge-pdfops-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("note.pdf");
        let stripped = dir.join("note stripped.pdf");
        std::fs::write(&source, crate::engines::pdf::text_pdf("Hello")).unwrap();
        let original = std::fs::read(&source).unwrap();
        assert_eq!(page_count(&source).unwrap(), 1);
        strip_metadata(&source, &stripped).unwrap();
        compress(&source, &dir.join("note compressed.pdf")).unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert!(stripped.is_file());
        assert!(page_count(&stripped).unwrap() >= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn splits_each_page_into_its_own_pdf() {
        let dir = std::env::temp_dir().join(format!("fileforge-pdfsplit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("two.pdf");
        let text = "line\n".repeat(80);
        std::fs::write(&source, crate::engines::pdf::text_pdf(&text)).unwrap();
        let count = page_count(&source).unwrap();
        assert!(count >= 2, "expected at least two pages, got {count}");
        let destinations: Vec<PathBuf> = (1..=count).map(|index| dir.join(format!("page {index}.pdf"))).collect();
        split_pages(&source, &destinations).unwrap();
        for destination in &destinations {
            assert_eq!(page_count(destination).unwrap(), 1);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
