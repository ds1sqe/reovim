//! Error types for architecture layer.

use std::fmt;

/// Architecture layer error.
#[derive(Debug)]
pub enum ArchError {
    /// I/O error from platform.
    Io(std::io::Error),
    /// Feature not supported on this platform.
    NotSupported(&'static str),
}

impl fmt::Display for ArchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::NotSupported(feature) => write!(f, "Not supported: {feature}"),
        }
    }
}

impl std::error::Error for ArchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::NotSupported(_) => None,
        }
    }
}

impl From<std::io::Error> for ArchError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
