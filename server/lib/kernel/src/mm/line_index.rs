//! Index of newline byte offsets for O(log n) line lookup.
//!
//! `LineIndex` scans raw bytes once, recording the byte offset of every `\n`.
//! Subsequent line lookups use binary search over this offset vector.
//!
//! # Semantics (line-separator model)
//!
//! - Empty content (0 bytes): 0 lines.
//! - `"hello"` (no newline): 1 line.
//! - `"hello\n"` (trailing newline): 2 lines (last line is empty).
//! - `"a\nb\nc"`: 3 lines.
//!
//! These match the existing `Buffer` / `Rope` semantics exactly.
//!
//! # UTF-8 Validation
//!
//! [`LineIndex::from_bytes`] validates UTF-8 incrementally during the newline
//! scan.  If invalid UTF-8 is found, it returns `Err` — the caller must
//! reject the file.

use std::ops::Range;

/// Index of newline byte offsets for O(log n) line lookup.
///
/// `offsets[i]` = byte offset of the (i+1)-th `\n` in the content.
/// Line 0 starts at byte 0.  Line k (k > 0) starts at `offsets[k-1] + 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    /// Byte offsets of each `\n` character in the content.
    offsets: Vec<u64>,
    /// Total byte length of the indexed content.
    total_bytes: u64,
    /// Whether the content contains any `\r\n` sequences.
    has_crlf: bool,
}

/// Error returned when the content is not valid UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidUtf8;

impl std::fmt::Display for InvalidUtf8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("file is not valid UTF-8")
    }
}

impl std::error::Error for InvalidUtf8 {}

/// Scan bytes for newline offsets and CRLF detection.
fn scan_newlines(bytes: &[u8]) -> (Vec<u64>, bool) {
    let mut offsets = Vec::new();
    let mut has_crlf = false;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            if i > 0 && bytes[i - 1] == b'\r' {
                has_crlf = true;
            }
            offsets.push(i as u64);
        }
    }

    (offsets, has_crlf)
}

impl LineIndex {
    /// Build a line index from raw bytes (single-pass scan).
    ///
    /// Validates UTF-8 up front.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidUtf8`] if the content is not valid UTF-8.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InvalidUtf8> {
        std::str::from_utf8(bytes).map_err(|_| InvalidUtf8)?;

        let total_bytes = bytes.len() as u64;
        let (offsets, has_crlf) = scan_newlines(bytes);

        Ok(Self {
            offsets,
            total_bytes,
            has_crlf,
        })
    }

    /// Build a line index from a string (infallible — already valid UTF-8).
    #[must_use]
    pub fn build_from_str(s: &str) -> Self {
        let bytes = s.as_bytes();
        let total_bytes = bytes.len() as u64;
        let (offsets, has_crlf) = scan_newlines(bytes);

        Self {
            offsets,
            total_bytes,
            has_crlf,
        }
    }

    /// Number of lines in the indexed content.
    ///
    /// Empty content has 0 lines.  Non-empty content has
    /// `newline_count + 1` lines.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        if self.total_bytes == 0 {
            return 0;
        }
        self.offsets.len() + 1
    }

    /// Total byte length of the indexed content.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Whether the content contains CRLF line endings.
    #[must_use]
    pub const fn has_crlf(&self) -> bool {
        self.has_crlf
    }

    /// Byte range for line `idx` (start..end).
    ///
    /// The range includes the line content but excludes the trailing `\n`.
    /// Returns `None` if `idx >= line_count()`.
    #[must_use]
    pub fn line_byte_range(&self, idx: usize) -> Option<Range<u64>> {
        if idx >= self.line_count() {
            return None;
        }

        let start = if idx == 0 {
            0
        } else {
            self.offsets[idx - 1] + 1
        };

        let end = if idx < self.offsets.len() {
            self.offsets[idx]
        } else {
            self.total_bytes
        };

        Some(start..end)
    }

    /// Binary search: which line contains byte offset `byte_off`?
    ///
    /// Clamps to the last line if `byte_off` >= `total_bytes`.
    /// Returns 0 for empty content.
    #[must_use]
    pub fn byte_to_line(&self, byte_off: u64) -> usize {
        if self.total_bytes == 0 {
            return 0;
        }
        let byte_off = byte_off.min(self.total_bytes.saturating_sub(1));
        self.offsets.partition_point(|&off| off < byte_off)
    }

    /// Convert a line number to the byte offset of the start of that line.
    ///
    /// Returns `total_bytes` if `line` >= `line_count()`.
    #[must_use]
    pub fn line_to_byte(&self, line: usize) -> u64 {
        if line == 0 {
            return 0;
        }
        if line > self.offsets.len() {
            return self.total_bytes;
        }
        self.offsets[line - 1] + 1
    }

    /// Rebuild the line index after content changes.
    ///
    /// This is a convenience for `VirtualBuffer` edit operations that
    /// need to recompute the line index from materialized content.
    pub fn rebuild(&mut self, bytes: &[u8]) {
        self.offsets.clear();
        self.total_bytes = bytes.len() as u64;
        let (offsets, has_crlf) = scan_newlines(bytes);
        self.offsets = offsets;
        self.has_crlf = has_crlf;
    }
}
