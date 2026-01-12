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
mod tests {
    use super::*;

    #[test]
    fn test_injection_new() {
        let inj = Injection::new("rust", 100..200, 5, 3, 10, 3);

        assert_eq!(inj.language_id, "rust");
        assert_eq!(inj.byte_range, 100..200);
        assert_eq!(inj.start_row, 5);
        assert_eq!(inj.start_col, 3);
        assert_eq!(inj.end_row, 10);
        assert_eq!(inj.end_col, 3);
    }

    #[test]
    fn test_injection_from_bytes() {
        let inj = Injection::from_bytes("python", 50..150);

        assert_eq!(inj.language_id, "python");
        assert_eq!(inj.byte_range, 50..150);
        assert_eq!(inj.start_row, 0);
        assert_eq!(inj.start_col, 0);
        assert_eq!(inj.end_row, 0);
        assert_eq!(inj.end_col, 0);
    }

    #[test]
    fn test_injection_overlaps_lines() {
        let inj = Injection::new("rust", 100..200, 5, 0, 10, 0);

        // Overlapping line ranges
        assert!(inj.overlaps_lines(5, 10)); // Exact match
        assert!(inj.overlaps_lines(0, 7)); // Overlaps start
        assert!(inj.overlaps_lines(8, 15)); // Overlaps end
        assert!(inj.overlaps_lines(6, 8)); // Fully inside
        assert!(inj.overlaps_lines(0, 20)); // Fully contains

        // Non-overlapping line ranges
        assert!(!inj.overlaps_lines(0, 4));
        assert!(!inj.overlaps_lines(11, 20));
    }

    #[test]
    fn test_injection_contains_line() {
        let inj = Injection::new("rust", 100..200, 5, 0, 10, 0);

        assert!(inj.contains_line(5));
        assert!(inj.contains_line(7));
        assert!(inj.contains_line(10));
        assert!(!inj.contains_line(4));
        assert!(!inj.contains_line(11));
    }

    #[test]
    fn test_injection_overlaps_bytes() {
        let inj = Injection::new("rust", 100..200, 0, 0, 0, 0);

        assert!(inj.overlaps_bytes(&(50..150)));
        assert!(inj.overlaps_bytes(&(150..250)));
        assert!(inj.overlaps_bytes(&(120..180)));
        assert!(inj.overlaps_bytes(&(50..250)));

        assert!(!inj.overlaps_bytes(&(0..100)));
        assert!(!inj.overlaps_bytes(&(200..300)));
    }

    #[test]
    fn test_injection_byte_len() {
        let inj = Injection::new("rust", 100..200, 0, 0, 0, 0);
        assert_eq!(inj.byte_len(), 100);

        let empty = Injection::new("rust", 100..100, 0, 0, 0, 0);
        assert_eq!(empty.byte_len(), 0);
    }

    #[test]
    fn test_injection_is_multiline() {
        let multiline = Injection::new("rust", 100..200, 5, 0, 10, 0);
        assert!(multiline.is_multiline());

        let single = Injection::new("rust", 100..200, 5, 0, 5, 10);
        assert!(!single.is_multiline());
    }

    #[test]
    fn test_injection_line_count() {
        let inj = Injection::new("rust", 100..200, 5, 0, 10, 0);
        assert_eq!(inj.line_count(), 6); // Lines 5, 6, 7, 8, 9, 10

        let single = Injection::new("rust", 100..200, 5, 0, 5, 10);
        assert_eq!(single.line_count(), 1);
    }
}
