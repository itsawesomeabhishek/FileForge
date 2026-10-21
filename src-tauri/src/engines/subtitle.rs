use super::{text, ConversionEngine, ConversionRequest};
use crate::error::{ForgeError, Result};
use crate::formats::{FileFormat, FormatCategory};

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

pub fn parse(input: &str, hint: FileFormat) -> Vec<Cue> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = normalized.split('\n').collect();

    if hint == FileFormat::Vtt || lines.first().is_some_and(|l| l.starts_with("WEBVTT")) {
        // Skip the header block, then the blank lines that follow it.
        let header_len = lines
            .iter()
            .take_while(|l| !l.trim().is_empty() && !l.contains("-->"))
            .count();
        lines.drain(..header_len);
        let blank_len = lines.iter().take_while(|l| l.trim().is_empty()).count();
        lines.drain(..blank_len);
    }

    let mut cues = Vec::new();
    let mut index = 0;
    let mut saw_timecode = false;
    while index < lines.len() {
        let line = lines[index].trim();
        if line.contains("-->") {
            saw_timecode = true;
            let mut parts = line.split("-->");
            let start = parse_timestamp(parts.next().unwrap_or(""));
            let end_token = parts
                .next()
                .map(|p| p.trim().split(' ').next().unwrap_or("").to_string())
                .unwrap_or_default();
            let end = parse_timestamp(&end_token);
            let mut body: Vec<&str> = Vec::new();
            index += 1;
            while index < lines.len() && !lines[index].trim().is_empty() {
                body.push(lines[index]);
                index += 1;
            }
            if let (Some(start), Some(end)) = (start, end) {
                if end >= start {
                    cues.push(Cue {
                        start,
                        end,
                        text: body.join("\n"),
                    });
                }
            }
        } else {
            index += 1;
        }
    }

    if !saw_timecode {
        return plain_text_cues(&normalized);
    }
    cues
}

/// Splits untimed text into paragraphs and spaces them out by reading speed.
pub fn plain_text_cues(input: &str) -> Vec<Cue> {
    let blocks: Vec<String> = input
        .split("\n\n")
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty())
        .collect();
    let source: Vec<String> = if blocks.is_empty() {
        input
            .split('\n')
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect()
    } else {
        blocks
    };

    let mut cues = Vec::new();
    let mut clock = 0.0;
    for block in source {
        let words = block.split([' ', '\n']).filter(|w| !w.is_empty()).count();
        let duration = (words as f64 * 0.35).clamp(1.5, 8.0);
        cues.push(Cue {
            start: clock,
            end: clock + duration,
            text: block,
        });
        clock += duration;
    }
    cues
}

pub fn parse_timestamp(raw: &str) -> Option<f64> {
    let cleaned = raw.trim().replace(',', ".");
    let parts: Vec<&str> = cleaned.split(':').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for part in parts {
        let value: f64 = part.parse().ok()?;
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        total = total * 60.0 + value;
    }
    Some(total)
}

pub fn format_timestamp(seconds: f64, separator: char) -> String {
    let clamped = if seconds.is_finite() {
        seconds.clamp(0.0, 315_360_000.0)
    } else {
        0.0
    };
    let total_millis = (clamped * 1000.0).round() as u64;
    let hours = total_millis / 3_600_000;
    let minutes = (total_millis / 60_000) % 60;
    let secs = (total_millis / 1000) % 60;
    let millis = (total_millis % 1000).min(999);
    format!("{hours:02}:{minutes:02}:{secs:02}{separator}{millis:03}")
}

pub fn serialize(cues: &[Cue], format: FileFormat) -> String {
    match format {
        FileFormat::Vtt => {
            let mut output = String::from("WEBVTT\n\n");
            for cue in cues {
                output.push_str(&format!(
                    "{} --> {}\n{}\n\n",
                    format_timestamp(cue.start, '.'),
                    format_timestamp(cue.end, '.'),
                    cue.text
                ));
            }
            output
        }
        FileFormat::Srt => {
            let mut output = String::new();
            for (index, cue) in cues.iter().enumerate() {
                output.push_str(&format!(
                    "{}\n{} --> {}\n{}\n\n",
                    index + 1,
                    format_timestamp(cue.start, ','),
                    format_timestamp(cue.end, ','),
                    cue.text
                ));
            }
            output
        }
        _ => {
            let body: Vec<String> = cues.iter().map(|c| strip_tags(&c.text)).collect();
            format!("{}\n", body.join("\n\n"))
        }
    }
}

/// Removes `<...>` markup and `{\...}` styling overrides.
pub fn strip_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        if c == '<' {
            if let Some(close) = chars[index + 1..].iter().position(|&x| x == '>') {
                if close > 0 {
                    index += close + 2;
                    continue;
                }
            }
        } else if c == '{' && chars.get(index + 1) == Some(&'\\') {
            if let Some(close) = chars[index + 2..].iter().position(|&x| x == '}') {
                index += close + 3;
                continue;
            }
        }
        output.push(c);
        index += 1;
    }
    output
}

pub struct SubtitleEngine;

impl ConversionEngine for SubtitleEngine {
    fn name(&self) -> &'static str {
        "SubtitleEngine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        let from_subtitle = from.category() == FormatCategory::Subtitle
            && (to.category() == FormatCategory::Subtitle || to == FileFormat::Txt);
        let from_text = from == FileFormat::Txt && to.category() == FormatCategory::Subtitle;
        from_subtitle || from_text
    }

    fn convert(&self, request: &ConversionRequest) -> Result<()> {
        let input = text::read(&request.source)?;
        let cues = parse(&input, request.source_format);
        if cues.is_empty() {
            return Err(ForgeError::InvalidInput(
                "No valid subtitle cues were found.".into(),
            ));
        }
        let output = serialize(&cues, request.target);
        std::fs::write(&request.destination, output)?;
        (request.progress)(1.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str = "1\n00:00:01,000 --> 00:00:03,500\nHello <i>world</i>\n\n2\n00:00:04,000 --> 00:00:05,000\nBye\n\n";

    #[test]
    fn parses_srt() {
        let cues = parse(SRT, FileFormat::Srt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start, 1.0);
        assert_eq!(cues[0].end, 3.5);
        assert_eq!(cues[0].text, "Hello <i>world</i>");
    }

    #[test]
    fn srt_to_vtt_round_trip() {
        let cues = parse(SRT, FileFormat::Srt);
        let vtt = serialize(&cues, FileFormat::Vtt);
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:01.000 --> 00:00:03.500"));
        let back = parse(&vtt, FileFormat::Vtt);
        assert_eq!(back, cues);
        let srt = serialize(&back, FileFormat::Srt);
        assert!(srt.contains("1\n00:00:01,000 --> 00:00:03,500"));
    }

    #[test]
    fn vtt_header_with_notes_is_skipped() {
        let vtt = "WEBVTT - title\nKind: captions\n\n00:01.000 --> 00:02.000 align:start\nHi\n\n";
        let cues = parse(vtt, FileFormat::Vtt);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].start, 1.0);
        assert_eq!(cues[0].end, 2.0);
    }

    #[test]
    fn txt_output_strips_tags() {
        let cues = parse(SRT, FileFormat::Srt);
        let text = serialize(&cues, FileFormat::Txt);
        assert_eq!(text, "Hello world\n\nBye\n");
    }

    #[test]
    fn strips_styling_overrides() {
        assert_eq!(strip_tags("{\\an8}Top <b>bold</b> a < b"), "Top bold a < b");
    }

    #[test]
    fn plain_text_becomes_timed_cues() {
        let cues = parse("First line here\n\nSecond one", FileFormat::Txt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start, 0.0);
        assert!(cues[0].end >= 1.5);
        assert_eq!(cues[1].start, cues[0].end);
    }

    #[test]
    fn rejects_reversed_and_invalid_times() {
        assert_eq!(parse_timestamp("00:00:01,5"), Some(1.5));
        assert_eq!(parse_timestamp("abc"), None);
        assert_eq!(parse_timestamp("1:2:3:4"), None);
        let cues = parse("1\n00:00:05,000 --> 00:00:01,000\nx\n\n", FileFormat::Srt);
        assert!(cues.is_empty());
    }

    #[test]
    fn formats_timestamps() {
        assert_eq!(format_timestamp(3661.5, ','), "01:01:01,500");
        assert_eq!(format_timestamp(f64::NAN, '.'), "00:00:00.000");
    }

    #[test]
    fn engine_support() {
        let engine = SubtitleEngine;
        assert!(engine.supports(FileFormat::Srt, FileFormat::Vtt));
        assert!(engine.supports(FileFormat::Vtt, FileFormat::Txt));
        assert!(engine.supports(FileFormat::Txt, FileFormat::Srt));
        assert!(!engine.supports(FileFormat::Txt, FileFormat::Pdf));
    }
}
