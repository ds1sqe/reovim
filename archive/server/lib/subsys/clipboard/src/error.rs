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
#[path = "error_tests.rs"]
mod tests;
