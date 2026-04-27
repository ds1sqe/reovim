//! Formatter provider trait.

use std::path::Path;

use crate::error::FormatError;

/// A formatter that can format source code content.
///
/// Implementations include external CLI formatters (rustfmt, prettier, etc.)
/// and LSP-based formatters (in the format module, not this driver).
pub trait FormatterProvider: Send + Sync {
    /// Format the entire content string. Returns formatted content.
    ///
    /// # Errors
    ///
    /// Returns [`FormatError`] if formatting fails.
    fn format(&self, content: &str, path: &Path) -> Result<String, FormatError>;

    /// Format a range within content. Returns full content with range formatted.
    ///
    /// Default implementation formats the entire content (ignoring range).
    ///
    /// # Errors
    ///
    /// Returns [`FormatError`] if formatting fails.
    fn format_range(
        &self,
        content: &str,
        path: &Path,
        _start_line: u32,
        _end_line: u32,
    ) -> Result<String, FormatError> {
        self.format(content, path)
    }

    /// Whether this formatter supports range formatting.
    fn supports_range(&self) -> bool {
        false
    }

    /// Display name for status messages.
    fn name(&self) -> &str;
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
