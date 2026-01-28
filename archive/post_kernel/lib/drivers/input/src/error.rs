//! Input driver error types.
//!
//! Linux equivalent: `include/linux/errno.h` (input-specific errors)

use std::fmt;

/// Errors that can occur during input operations.
#[derive(Debug)]
pub enum InputError {
    /// Input driver is not initialized.
    NotInitialized,
    /// Input driver is already running.
    AlreadyRunning,
    /// Input driver is not running.
    NotRunning,
    /// Input driver failed to start.
    StartFailed(String),
    /// Input driver failed to stop.
    StopFailed(String),
    /// Key injection failed.
    InjectionFailed(String),
    /// Invalid key sequence.
    InvalidKeySequence(String),
    /// Keymap operation failed.
    KeymapError(String),
    /// Handler registration failed.
    HandlerRegistrationFailed(String),
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "input driver not initialized"),
            Self::AlreadyRunning => write!(f, "input driver already running"),
            Self::NotRunning => write!(f, "input driver not running"),
            Self::StartFailed(msg) => write!(f, "failed to start input driver: {msg}"),
            Self::StopFailed(msg) => write!(f, "failed to stop input driver: {msg}"),
            Self::InjectionFailed(msg) => write!(f, "key injection failed: {msg}"),
            Self::InvalidKeySequence(msg) => write!(f, "invalid key sequence: {msg}"),
            Self::KeymapError(msg) => write!(f, "keymap error: {msg}"),
            Self::HandlerRegistrationFailed(msg) => {
                write!(f, "handler registration failed: {msg}")
            }
        }
    }
}

impl std::error::Error for InputError {}

/// Errors that can occur during clipboard operations.
#[derive(Debug)]
pub enum ClipboardError {
    /// Clipboard is not available on this platform.
    NotAvailable,
    /// Failed to read from clipboard.
    ReadFailed(String),
    /// Failed to write to clipboard.
    WriteFailed(String),
    /// Clipboard content is not text.
    NotText,
    /// Clipboard provider not configured.
    NoProvider,
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "clipboard not available"),
            Self::ReadFailed(msg) => write!(f, "failed to read clipboard: {msg}"),
            Self::WriteFailed(msg) => write!(f, "failed to write clipboard: {msg}"),
            Self::NotText => write!(f, "clipboard content is not text"),
            Self::NoProvider => write!(f, "no clipboard provider configured"),
        }
    }
}

impl std::error::Error for ClipboardError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_error_display() {
        assert_eq!(format!("{}", InputError::NotInitialized), "input driver not initialized");
        assert_eq!(format!("{}", InputError::AlreadyRunning), "input driver already running");
        assert_eq!(format!("{}", InputError::NotRunning), "input driver not running");
        assert!(format!("{}", InputError::StartFailed("test".into())).contains("test"));
        assert!(format!("{}", InputError::StopFailed("test".into())).contains("test"));
        assert!(format!("{}", InputError::InjectionFailed("test".into())).contains("test"));
        assert!(format!("{}", InputError::InvalidKeySequence("test".into())).contains("test"));
        assert!(format!("{}", InputError::KeymapError("test".into())).contains("test"));
        assert!(
            format!("{}", InputError::HandlerRegistrationFailed("test".into())).contains("test")
        );
    }

    #[test]
    fn test_clipboard_error_display() {
        assert_eq!(format!("{}", ClipboardError::NotAvailable), "clipboard not available");
        assert_eq!(format!("{}", ClipboardError::NotText), "clipboard content is not text");
        assert_eq!(format!("{}", ClipboardError::NoProvider), "no clipboard provider configured");
        assert!(format!("{}", ClipboardError::ReadFailed("test".into())).contains("test"));
        assert!(format!("{}", ClipboardError::WriteFailed("test".into())).contains("test"));
    }
}
