use super::{pdf, text, ConversionEngine, ConversionRequest};
use crate::error::{ForgeError, Result};
use crate::formats::FileFormat;

/// Text, Markdown, HTML, CSV, and TSV. PDF output is the text. PDF input
/// keeps the text layer. DOCX, Pages, slides, and XLSX are not offered.
pub struct DocumentEngine;

impl ConversionEngine for DocumentEngine {
    fn name(&self) -> &'static str {
        "DocumentEngine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        match (from, to) {
            (FileFormat::Txt, FileFormat::Pdf | FileFormat::Html | FileFormat::Md) => true,
            (FileFormat::Md, FileFormat::Txt | FileFormat::Html | FileFormat::Pdf) => true,
            (FileFormat::Html, FileFormat::Txt | FileFormat::Md | FileFormat::Pdf) => true,
            (FileFormat::Csv, FileFormat::Tsv | FileFormat::Pdf) => true,
            (FileFormat::Tsv, FileFormat::Csv | FileFormat::Pdf) => true,
            (FileFormat::Pdf, FileFormat::Txt | FileFormat::Md) => true,
            _ => false,
        }
    }

    fn convert(&self, request: &ConversionRequest) -> Result<()> {
        let body = if request.source_format == FileFormat::Pdf {
            extract_pdf_text(&request.source)?
        } else {
            let raw = text::read(&request.source)?;
            match request.source_format {
                FileFormat::Csv => table_text(&raw, ',', request.target),
                FileFormat::Tsv => table_text(&raw, '\t', request.target),
                FileFormat::Html => html_to_text(&raw),
                _ => raw,
            }
        };
        (request.progress)(0.4);
        request.cancel.check()?;
        let output = match request.target {
            FileFormat::Pdf => pdf::text_pdf(&body),
            FileFormat::Html => markdown_to_html(&body).into_bytes(),
            FileFormat::Txt | FileFormat::Md | FileFormat::Csv | FileFormat::Tsv => body.into_bytes(),
            _ => {
                return Err(ForgeError::UnsupportedConversion(
                    request.source_format,
                    request.target,
                ));
            }
        };
        if output.is_empty() {
            return Err(ForgeError::InvalidInput(
                "The file has no text FileForge can keep.".into(),
            ));
        }
        std::fs::write(&request.destination, output)?;
        (request.progress)(1.0);
        Ok(())
    }
}

fn extract_pdf_text(path: &std::path::Path) -> Result<String> {
    let text = pdf_extract::extract_text(path).map_err(|error| {
        ForgeError::ToolFailed("pdf".into(), error.to_string())
    })?;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(ForgeError::InvalidInput(
            "This PDF has no text layer. Scanned pages are not read.".into(),
        ));
    }
    Ok(text)
}

fn table_text(source: &str, separator: char, target: FileFormat) -> String {
    let rows = parse_table(source, separator);
    match target {
        FileFormat::Csv => write_table(&rows, ','),
        FileFormat::Tsv => write_table(&rows, '\t'),
        _ => rows
            .iter()
            .map(|row| row.join(" | "))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn parse_table(text: &str, separator: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cell.push('"');
                } else {
                    quoted = false;
                }
            } else {
                cell.push(ch);
            }
        } else if ch == '"' && cell.is_empty() {
            quoted = true;
        } else if ch == separator {
            row.push(std::mem::take(&mut cell));
        } else if ch == '\n' {
            row.push(std::mem::take(&mut cell));
            rows.push(std::mem::take(&mut row));
        } else if ch != '\r' {
            cell.push(ch);
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

fn write_table(rows: &[Vec<String>], separator: char) -> String {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|cell| {
                    if cell.contains(separator) || cell.contains('"') || cell.contains('\n') {
                        format!("\"{}\"", cell.replace('"', "\"\""))
                    } else {
                        cell.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(&separator.to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn markdown_to_html(markdown: &str) -> String {
    let mut html = String::from("<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body>\n");
    for block in markdown.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        if let Some(rest) = block.strip_prefix("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", escape_html(rest)));
        } else if let Some(rest) = block.strip_prefix("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", escape_html(rest)));
        } else if let Some(rest) = block.strip_prefix("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", escape_html(rest)));
        } else {
            html.push_str(&format!("<p>{}</p>\n", escape_html(block).replace('\n', "<br>\n")));
        }
    }
    html.push_str("</body></html>\n");
    html
}

fn html_to_text(html: &str) -> String {
    let mut text = String::new();
    let mut tag = false;
    for ch in html.chars() {
        if ch == '<' {
            tag = true;
        } else if ch == '>' {
            tag = false;
            text.push(' ');
        } else if !tag {
            text.push(ch);
        }
    }
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_text_to_pdf() {
        let engine = DocumentEngine;
        assert!(engine.supports(FileFormat::Txt, FileFormat::Pdf));
        assert!(engine.supports(FileFormat::Pdf, FileFormat::Txt));
        assert!(engine.supports(FileFormat::Csv, FileFormat::Tsv));
        assert!(engine.supports(FileFormat::Md, FileFormat::Html));
        assert!(!engine.supports(FileFormat::Pdf, FileFormat::Docx));
        assert!(!engine.supports(FileFormat::Png, FileFormat::Docx));
    }

    #[test]
    fn csv_quotes_round_trip_to_tsv() {
        let rows = parse_table("name,\"a,b\"\n\"line\n2\",ok", ',');
        assert_eq!(rows[0], vec!["name", "a,b"]);
        assert_eq!(rows[1][0], "line\n2");
        let tsv = write_table(&rows, '\t');
        assert!(tsv.contains("line\n2") || tsv.contains("\"line\n2\""));
    }

    #[test]
    fn markdown_heading_becomes_html() {
        let html = markdown_to_html("# Title\n\nHello");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<p>Hello</p>"));
    }
}
