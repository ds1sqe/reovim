//! Clipboard operation error types.

use std::fmt;

/// Errors that can occur during clipboard operations.
#[derive(Debug)]
pub enum ClipboardError {
    /// Clipboard is not available (e.g., no display server on Linux).
    NotAvailable(String),
    /// Failed to read from clipboard.
    ReadFailed(String),
    /// Failed to write to clipboard.
    WriteFailed(String),
    /// Platform-specific error.
    Platform(String),
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAvailable(msg) => write!(f, "Clipboard not available: {msg}"),
            Self::ReadFailed(msg) => write!(f, "Failed to read clipboard: {msg}"),
            Self::WriteFailed(msg) => write!(f, "Failed to write clipboard: {msg}"),
            Self::Platform(msg) => write!(f, "Platform clipboard error: {msg}"),
        }
    }
}

impl std::error::Error for ClipboardError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_not_available() {
        let err = ClipboardError::NotAvailable("no display".into());
        assert!(err.to_string().contains("not available"));
        assert!(err.to_string().contains("no display"));
    }

    #[test]
    fn display_read_failed() {
        let err = ClipboardError::ReadFailed("timeout".into());
        assert!(err.to_string().contains("read clipboard"));
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn display_write_failed() {
        let err = ClipboardError::WriteFailed("locked".into());
        assert!(err.to_string().contains("write clipboard"));
        assert!(err.to_string().contains("locked"));
    }

    #[test]
    fn display_platform() {
        let err = ClipboardError::Platform("wayland error".into());
        assert!(err.to_string().contains("Platform"));
        assert!(err.to_string().contains("wayland error"));
    }

    #[test]
    fn debug() {
        let err = ClipboardError::NotAvailable("test".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("NotAvailable"));
    }

    #[test]
    fn is_error() {
        let err = ClipboardError::ReadFailed("test".into());
        let _: &dyn std::error::Error = &err;
    }
}
