use std::fmt;

use crate::formats::FileFormat;

/// Single error type for all conversion work. Messages are user-readable.
#[derive(Debug)]
pub enum ExploitError {
    UnsupportedConversion(FileFormat, FileFormat),
    InvalidInput(String),
    FfmpegMissing(String),
    ToolFailed(String, String),
    EncodeFailed(String),
    Io(String),
    Cancelled,
}

impl fmt::Display for ExploitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedConversion(from, to) => write!(
                f,
                "Converting {} to {} is not supported.",
                from.display_name(),
                to.display_name()
            ),
            Self::InvalidInput(message) => write!(f, "{message}"),
            Self::FfmpegMissing(purpose) => write!(
                f,
                "{purpose} needs FFmpeg, which was not found. Set its path in Settings."
            ),
            Self::ToolFailed(tool, reason) => write!(f, "{tool} failed: {reason}"),
            Self::EncodeFailed(name) => write!(f, "Could not create {name}."),
            Self::Io(message) => write!(f, "{message}"),
            Self::Cancelled => write!(f, "Cancelled."),
        }
    }
}

impl std::error::Error for ExploitError {}

impl From<std::io::Error> for ExploitError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ExploitError>;
