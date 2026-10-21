//! Minimal PDF writer: one JPEG image per page, or paginated plain text in
//! Helvetica. Enough for image-to-PDF and text-to-PDF without a PDF crate.

struct Builder {
    bytes: Vec<u8>,
    offsets: Vec<usize>,
}

impl Builder {
    fn new() -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        Self {
            bytes,
            offsets: Vec::new(),
        }
    }

    /// Adds object number `offsets.len() + 1` and returns that number.
    fn add_object(&mut self, body: &[u8]) -> usize {
        let number = self.offsets.len() + 1;
        self.offsets.push(self.bytes.len());
        self.bytes
            .extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        self.bytes.extend_from_slice(body);
        self.bytes.extend_from_slice(b"\nendobj\n");
        number
    }

    fn add_stream(&mut self, dictionary: &str, data: &[u8]) -> usize {
        let mut body = format!("<< {dictionary} /Length {} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\nendstream");
        self.add_object(&body)
    }

    fn finish(mut self, root: usize) -> Vec<u8> {
        let xref_start = self.bytes.len();
        let count = self.offsets.len() + 1;
        self.bytes
            .extend_from_slice(format!("xref\n0 {count}\n0000000000 65535 f \n").as_bytes());
        for offset in &self.offsets {
            self.bytes
                .extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        self.bytes.extend_from_slice(
            format!("trailer\n<< /Size {count} /Root {root} 0 R >>\nstartxref\n{xref_start}\n%%EOF\n")
                .as_bytes(),
        );
        self.bytes
    }
}

/// A one-page PDF whose page is exactly the image (96 dpi to points).
pub fn image_pdf(jpeg: &[u8], width: u32, height: u32) -> Vec<u8> {
    let page_width = (f64::from(width) * 0.75).max(1.0);
    let page_height = (f64::from(height) * 0.75).max(1.0);
    let mut builder = Builder::new();

    // Objects are numbered in creation order: 1 catalog, 2 pages, 3 page,
    // 4 content, 5 image.
    builder.add_object(b"<< /Type /Catalog /Pages 2 0 R >>");
    builder.add_object(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    builder.add_object(
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_width:.2} {page_height:.2}] \
             /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>"
        )
        .as_bytes(),
    );
    let content = format!("q {page_width:.2} 0 0 {page_height:.2} 0 0 cm /Im0 Do Q");
    builder.add_stream("", content.as_bytes());
    builder.add_stream(
        &format!(
            "/Type /XObject /Subtype /Image /Width {width} /Height {height} \
             /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode"
        ),
        jpeg,
    );
    builder.finish(1)
}

const PAGE_WIDTH: f64 = 595.0;
const PAGE_HEIGHT: f64 = 842.0;
const MARGIN: f64 = 54.0;
const FONT_SIZE: f64 = 11.0;
const LEADING: f64 = 14.0;
/// Conservative average glyph width for Helvetica, used for word wrapping.
const AVG_CHAR_WIDTH: f64 = FONT_SIZE * 0.55;

/// Wraps text to the printable width, preserving blank lines.
pub fn wrap_lines(text: &str) -> Vec<String> {
    let max_chars = ((PAGE_WIDTH - 2.0 * MARGIN) / AVG_CHAR_WIDTH).floor() as usize;
    let mut lines = Vec::new();
    for raw in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let raw = raw.replace('\t', "    ");
        if raw.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in raw.split(' ') {
            let mut word = word.to_string();
            // Break words longer than a whole line.
            while word.chars().count() > max_chars {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                let head: String = word.chars().take(max_chars).collect();
                word = word.chars().skip(max_chars).collect();
                lines.push(head);
            }
            let needed = current.chars().count() + word.chars().count() + usize::from(!current.is_empty());
            if needed > max_chars && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(&word);
        }
        lines.push(current);
    }
    lines
}

/// Encodes a character as a WinAnsi byte; unknown characters become `?`.
fn win_ansi(c: char) -> u8 {
    match c {
        '\u{20}'..='\u{7E}' | '\u{A0}'..='\u{FF}' => c as u8,
        '\u{20AC}' => 0x80,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201C}' => 0x93,
        '\u{201D}' => 0x94,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{2026}' => 0x85,
        _ => b'?',
    }
}

fn escape_line(line: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(line.len());
    for c in line.chars() {
        match win_ansi(c) {
            b'(' => out.extend_from_slice(b"\\("),
            b')' => out.extend_from_slice(b"\\)"),
            b'\\' => out.extend_from_slice(b"\\\\"),
            byte => out.push(byte),
        }
    }
    out
}

/// A multi-page A4 PDF of the given plain text.
pub fn text_pdf(text: &str) -> Vec<u8> {
    let lines = wrap_lines(text);
    let per_page = (((PAGE_HEIGHT - 2.0 * MARGIN) / LEADING).floor() as usize).max(1);
    let pages: Vec<&[String]> = if lines.is_empty() {
        vec![&[][..]]
    } else {
        lines.chunks(per_page).collect()
    };

    let mut builder = Builder::new();
    // 1 catalog, 2 pages, 3 font; then (page, content) pairs from 4.
    builder.add_object(b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len()).map(|i| format!("{} 0 R", 4 + i * 2)).collect();
    builder.add_object(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .as_bytes(),
    );
    builder.add_object(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");

    for (index, page_lines) in pages.iter().enumerate() {
        let content_number = 5 + index * 2;
        builder.add_object(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] \
                 /Resources << /Font << /F1 3 0 R >> >> /Contents {content_number} 0 R >>"
            )
            .as_bytes(),
        );
        let mut content: Vec<u8> = format!(
            "BT /F1 {FONT_SIZE} Tf {LEADING} TL {MARGIN} {} Td\n",
            PAGE_HEIGHT - MARGIN - FONT_SIZE
        )
        .into_bytes();
        for line in page_lines.iter() {
            content.push(b'(');
            content.extend_from_slice(&escape_line(line));
            content.extend_from_slice(b") Tj T*\n");
        }
        content.extend_from_slice(b"ET");
        builder.add_stream("", &content);
    }
    builder.finish(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn text_pdf_is_well_formed() {
        let pdf = text_pdf("Hello (world) \\ caf\u{e9}\n\nSecond paragraph");
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        assert!(contains(&pdf, b"(Hello \\(world\\) \\\\ caf\xE9) Tj"));
        assert!(contains(&pdf, b"/Count 1"));
    }

    #[test]
    fn long_text_paginates() {
        let text = (0..200).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let pdf = text_pdf(&text);
        assert!(contains(&pdf, b"/Count 4"));
    }

    #[test]
    fn xref_offsets_point_at_objects() {
        let pdf = text_pdf("x");
        let marker = b"startxref\n";
        let marker_at = pdf
            .windows(marker.len())
            .rposition(|w| w == marker)
            .expect("startxref present");
        let number: Vec<u8> = pdf[marker_at + marker.len()..]
            .iter()
            .copied()
            .take_while(u8::is_ascii_digit)
            .collect();
        let start: usize = String::from_utf8(number).unwrap().parse().unwrap();
        assert!(pdf[start..].starts_with(b"xref\n"));

        // Entry 0 is the free list head; entry 1 is object 1.
        let table = &pdf[start..];
        let first_line_end = table.iter().position(|&b| b == b'\n').unwrap();
        let second_line_end = first_line_end
            + 1
            + table[first_line_end + 1..].iter().position(|&b| b == b'\n').unwrap();
        let entry_one = second_line_end + 1 + 20; // skip the 20-byte free entry
        let offset: usize = std::str::from_utf8(&table[entry_one..entry_one + 10])
            .unwrap()
            .parse()
            .unwrap();
        assert!(pdf[offset..].starts_with(b"1 0 obj"));
    }

    #[test]
    fn wraps_and_breaks_long_words() {
        let lines = wrap_lines(&"a".repeat(300));
        assert!(lines.len() > 2);
        assert!(lines.iter().all(|l| l.chars().count() <= 80));
        assert_eq!(wrap_lines("a\n\nb"), vec!["a", "", "b"]);
    }

    #[test]
    fn image_pdf_embeds_jpeg() {
        let pdf = image_pdf(&[0xFF, 0xD8, 0xFF, 0xD9], 100, 40);
        assert!(contains(&pdf, b"/Filter /DCTDecode"));
        assert!(contains(&pdf, b"/MediaBox [0 0 75.00 30.00]"));
    }
}
