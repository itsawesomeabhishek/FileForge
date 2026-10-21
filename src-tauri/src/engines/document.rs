use super::{pdf, text, ConversionEngine, ConversionRequest};
use crate::error::Result;
use crate::formats::FileFormat;

/// Phase 1 document support: plain text to PDF. Other document conversions
/// (docx, PDF to text/image) are deliberately unsupported and never offered.
pub struct DocumentEngine;

impl ConversionEngine for DocumentEngine {
    fn name(&self) -> &'static str {
        "DocumentEngine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        from == FileFormat::Txt && to == FileFormat::Pdf
    }

    fn convert(&self, request: &ConversionRequest) -> Result<()> {
        let input = text::read(&request.source)?;
        std::fs::write(&request.destination, pdf::text_pdf(&input))?;
        (request.progress)(1.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_text_to_pdf() {
        let engine = DocumentEngine;
        assert!(engine.supports(FileFormat::Txt, FileFormat::Pdf));
        assert!(!engine.supports(FileFormat::Pdf, FileFormat::Txt));
        assert!(!engine.supports(FileFormat::Png, FileFormat::Docx));
    }
}
