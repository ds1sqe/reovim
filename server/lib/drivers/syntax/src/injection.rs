//! Language injection types.
//!
//! This module defines types for representing embedded language regions
//! within a source file, such as code blocks in markdown or script tags in HTML.

use std::ops::Range;

/// An injection point for embedded languages.
///
/// Injections allow one language to be embedded within another,
/// such as code blocks in markdown or script tags in HTML.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::Injection;
///
/// // A Rust code block in a markdown file
/// let inj = Injection::new("rust", 100..200, 5, 3, 10, 3);
/// assert_eq!(inj.language_id, "rust");
/// assert!(inj.overlaps_lines(6, 8));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Injection {
    /// The language ID to use for the injected region.
    pub language_id: String,
    /// Byte range in the parent document (start..end, exclusive).
    pub byte_range: Range<usize>,
    /// Start row (0-indexed).
    pub start_row: u32,
    /// Start column (0-indexed).
    pub start_col: u32,
    /// End row (0-indexed).
    pub end_row: u32,
    /// End column (0-indexed).
    pub end_col: u32,
}

impl Injection {
    /// Create a new injection.
    #[must_use]
    pub fn new(
        language_id: impl Into<String>,
        byte_range: Range<usize>,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Self {
        Self {
            language_id: language_id.into(),
            byte_range,
            start_row,
            start_col,
            end_row,
            end_col,
        }
    }

    /// Create an injection from byte range only (row/col set to 0).
    ///
    /// Useful when only byte positions are known.
    #[must_use]
    pub fn from_bytes(language_id: impl Into<String>, byte_range: Range<usize>) -> Self {
        Self {
            language_id: language_id.into(),
            byte_range,
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 0,
        }
    }

    /// Check if this injection overlaps with a line range.
    ///
    /// Both `start_line` and `end_line` are inclusive.
    #[must_use]
    pub const fn overlaps_lines(&self, start_line: u32, end_line: u32) -> bool {
        self.start_row <= end_line && self.end_row >= start_line
    }

    /// Check if this injection contains a specific line.
    #[must_use]
    pub const fn contains_line(&self, line: u32) -> bool {
        line >= self.start_row && line <= self.end_row
    }

    /// Check if this injection overlaps with a byte range.
    #[must_use]
    pub const fn overlaps_bytes(&self, range: &Range<usize>) -> bool {
        self.byte_range.start < range.end && self.byte_range.end > range.start
    }

    /// Get the number of bytes in this injection.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_range.end - self.byte_range.start
    }

    /// Check if this injection spans multiple lines.
    #[must_use]
    pub const fn is_multiline(&self) -> bool {
        self.end_row > self.start_row
    }

    /// Get the number of lines spanned by this injection.
    #[must_use]
    pub const fn line_count(&self) -> u32 {
        self.end_row - self.start_row + 1
    }
}

#[cfg(test)]
#[path = "injection_tests.rs"]
mod tests;
