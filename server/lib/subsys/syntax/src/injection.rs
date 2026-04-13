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
/// # Single vs Combined Injections
///
/// - **Single-range**: A fenced code block in Markdown produces one injection
///   with one byte range (e.g., the code block content).
/// - **Combined**: Consecutive doc comment lines (`///`) produce one injection
///   with multiple byte ranges (one per comment line), merged via
///   `#set! injection.combined`.
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::Injection;
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
    /// Byte ranges in the parent document.
    /// Single-range injections (fenced code blocks) have exactly one element.
    /// Combined injections (doc comment lines) have multiple elements.
    pub ranges: Vec<Range<usize>>,
    /// Start row (0-indexed) of the first range.
    pub start_row: u32,
    /// Start column (0-indexed) of the first range.
    pub start_col: u32,
    /// End row (0-indexed) of the last range.
    pub end_row: u32,
    /// End column (0-indexed) of the last range.
    pub end_col: u32,
}

impl Injection {
    /// Create a new single-range injection.
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
            ranges: vec![byte_range],
            start_row,
            start_col,
            end_row,
            end_col,
        }
    }

    /// Create a combined injection from multiple byte ranges.
    ///
    /// Used for `injection.combined` patterns where multiple source nodes
    /// (e.g., consecutive doc comment lines) map to a single injection.
    #[must_use]
    pub fn combined(
        language_id: impl Into<String>,
        ranges: Vec<Range<usize>>,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Self {
        Self {
            language_id: language_id.into(),
            ranges,
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
            ranges: vec![byte_range],
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 0,
        }
    }

    /// Get the overall byte range (start of first range to end of last range).
    ///
    /// For single-range injections, this is identical to the original range.
    /// For combined injections, this spans the entire region.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> {
        match (self.ranges.first(), self.ranges.last()) {
            (Some(first), Some(last)) => first.start..last.end,
            _ => 0..0,
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
    pub fn overlaps_bytes(&self, range: &Range<usize>) -> bool {
        let overall = self.byte_range();
        overall.start < range.end && overall.end > range.start
    }

    /// Get the total number of bytes across all ranges.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.ranges.iter().map(|r| r.end - r.start).sum()
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

    /// Check if this is a combined injection (multiple ranges).
    #[must_use]
    pub const fn is_combined(&self) -> bool {
        self.ranges.len() > 1
    }
}

#[cfg(test)]
#[path = "injection_tests.rs"]
mod tests;
