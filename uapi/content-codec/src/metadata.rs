//! Codec metadata for round-trip encoding.
//!
//! Stores information about how a file was decoded, so it can be
//! re-encoded faithfully on save. Metadata is stored per-buffer
//! in [`CodecSessionState`](crate::CodecSessionState).

use std::collections::HashMap;

use crate::ContentType;

/// Metadata from a codec decode operation.
///
/// Captures everything needed for round-trip encoding: the content type,
/// and a flexible key-value store for codec-specific data (e.g., BOM
/// presence, original line ending style, encoding name).
///
/// # Examples
///
/// ```
/// use reovim_content_codec::{CodecMetadata, ContentType};
///
/// let mut metadata = CodecMetadata::new(ContentType::new("text/utf-8"));
/// metadata.set("bom", "true");
/// metadata.set("line_ending", "crlf");
///
/// assert_eq!(metadata.get("bom"), Some("true"));
/// assert_eq!(metadata.content_type().as_str(), "text/utf-8");
/// ```
#[derive(Debug, Clone)]
pub struct CodecMetadata {
    /// The content type that produced this metadata.
    content_type: ContentType,
    /// Codec-specific key-value data.
    data: HashMap<String, String>,
}

impl CodecMetadata {
    /// Create new metadata for the given content type.
    #[must_use]
    pub fn new(content_type: ContentType) -> Self {
        Self {
            content_type,
            data: HashMap::new(),
        }
    }

    /// Get the content type.
    #[must_use]
    pub const fn content_type(&self) -> &ContentType {
        &self.content_type
    }

    /// Get a metadata value by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }

    /// Set a metadata value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.data.insert(key.into(), value.into());
    }

    /// Remove a metadata value, returning it if present.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.data.remove(key)
    }

    /// Check if a metadata key exists.
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Get the underlying data map (for serialization).
    #[must_use]
    pub const fn data(&self) -> &HashMap<String, String> {
        &self.data
    }

    /// Get the number of metadata entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if there are no metadata entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl PartialEq for CodecMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.content_type == other.content_type && self.data == other.data
    }
}

impl Eq for CodecMetadata {}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
