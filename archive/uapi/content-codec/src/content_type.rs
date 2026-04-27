//! Content type identifier.
//!
//! A lightweight wrapper around `Arc<str>` identifying the content type
//! of a file (e.g., `"text/utf-8"`, `"binary/raw"`, `"text/euc-kr"`).
//!
//! Content types are assigned by classifiers and used by codec factories
//! to select the appropriate codec for decoding/encoding.

use std::sync::Arc;

/// Identifies the content type of a file.
///
/// Uses `Arc<str>` for efficient cloning and comparison. Content types
/// follow a namespace convention: `"text/utf-8"`, `"binary/raw"`,
/// `"text/euc-kr"`, etc.
///
/// # Examples
///
/// ```
/// use reovim_content_codec::ContentType;
///
/// let utf8 = ContentType::new("text/utf-8");
/// assert_eq!(utf8.as_str(), "text/utf-8");
///
/// let binary = ContentType::new("binary/raw");
/// assert_ne!(utf8, binary);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentType(Arc<str>);

impl ContentType {
    /// Create a new content type.
    #[must_use]
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self(name.into())
    }

    /// Get the content type as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if this is a text content type (starts with `"text/"`).
    #[must_use]
    pub fn is_text(&self) -> bool {
        self.0.starts_with("text/")
    }

    /// Check if this is a binary content type (starts with `"binary/"`).
    #[must_use]
    pub fn is_binary(&self) -> bool {
        self.0.starts_with("binary/")
    }
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Well-known content types.
impl ContentType {
    /// UTF-8 text (the default fallback).
    pub const UTF8: &'static str = "text/utf-8";

    /// Raw binary data (hex dump view).
    pub const BINARY_RAW: &'static str = "binary/raw";
}

#[cfg(test)]
#[path = "content_type_tests.rs"]
mod tests;
