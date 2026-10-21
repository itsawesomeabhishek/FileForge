use std::path::Path;

use crate::error::{ExploitError, Result};

/// Windows-1252 characters for bytes 0x80-0x9F (0 marks an undefined byte).
const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
    0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
];

fn decode_cp1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| match byte {
            0x80..=0x9F => char::from_u32(u32::from(CP1252_HIGH[usize::from(byte - 0x80)]))
                .filter(|c| *c != '\0')
                .unwrap_or('\u{FFFD}'),
            other => char::from(other),
        })
        .collect()
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Decodes text from raw bytes: UTF-8/UTF-16 BOMs, then UTF-8, then Windows-1252.
pub fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, false);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => decode_cp1252(bytes),
    }
}

pub fn read(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|error| {
        ExploitError::InvalidInput(format!("Could not read {}: {error}", path.display()))
    })?;
    Ok(decode(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_with_bom() {
        assert_eq!(decode(&[0xEF, 0xBB, 0xBF, b'h', b'i']), "hi");
    }

    #[test]
    fn decodes_utf16_le() {
        assert_eq!(decode(&[0xFF, 0xFE, b'h', 0, b'i', 0]), "hi");
    }

    #[test]
    fn falls_back_to_cp1252() {
        assert_eq!(decode(&[b'c', b'a', b'f', 0xE9]), "caf\u{e9}");
        assert_eq!(decode(&[0x93, b'x', 0x94]), "\u{201C}x\u{201D}");
    }
}
