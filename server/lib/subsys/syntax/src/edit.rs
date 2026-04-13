//! Edit types for incremental parsing.
//!
//! This module defines the [`SyntaxEdit`] type used to describe text
//! modifications for efficient incremental re-parsing.

use std::ops::Range;

/// Edit information for incremental parsing.
///
/// Describes a text modification in terms that parsers can use for
/// efficient incremental re-parsing. Mirrors tree-sitter's `InputEdit`
/// structure but without the tree-sitter dependency.
///
/// # Fields
///
/// The edit is described in terms of both byte offsets and row/column positions:
/// - `start_*`: Where the edit begins
/// - `old_end_*`: Where the old (replaced) content ended
/// - `new_end_*`: Where the new (inserted) content ends
///
/// # Example
///
/// ```
/// use reovim_subsys_syntax::SyntaxEdit;
///
/// // Inserting "Hello" at position 0
/// let edit = SyntaxEdit::insert(0, 0, 0, 5, 0, 5);
/// assert_eq!(edit.new_end_byte, 5);
/// assert_eq!(edit.old_end_byte, 0); // Nothing was replaced
///
/// // Deleting 10 bytes starting at position 5
/// let edit = SyntaxEdit::delete(5, 0, 5, 15, 0, 15);
/// assert_eq!(edit.start_byte, 5);
/// assert_eq!(edit.old_end_byte, 15);
/// assert_eq!(edit.new_end_byte, 5); // Nothing was inserted
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxEdit {
    /// Byte offset where the edit starts.
    pub start_byte: usize,
    /// Byte offset where the old content ended.
    pub old_end_byte: usize,
    /// Byte offset where the new content ends.
    pub new_end_byte: usize,
    /// Row (line) where the edit starts (0-indexed).
    pub start_row: u32,
    /// Column where the edit starts (0-indexed).
    pub start_col: u32,
    /// Row where old content ended.
    pub old_end_row: u32,
    /// Column where old content ended.
    pub old_end_col: u32,
    /// Row where new content ends.
    pub new_end_row: u32,
    /// Column where new content ends.
    pub new_end_col: u32,
}

impl SyntaxEdit {
    /// Create a new syntax edit with all fields specified.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        start_byte: usize,
        old_end_byte: usize,
        new_end_byte: usize,
        start_row: u32,
        start_col: u32,
        old_end_row: u32,
        old_end_col: u32,
        new_end_row: u32,
        new_end_col: u32,
    ) -> Self {
        Self {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_row,
            start_col,
            old_end_row,
            old_end_col,
            new_end_row,
            new_end_col,
        }
    }

    /// Create an edit for an insertion (no content replaced).
    ///
    /// When inserting, `old_end` equals `start` since nothing was removed.
    #[must_use]
    pub const fn insert(
        start_byte: usize,
        start_row: u32,
        start_col: u32,
        new_end_byte: usize,
        new_end_row: u32,
        new_end_col: u32,
    ) -> Self {
        Self {
            start_byte,
            old_end_byte: start_byte,
            new_end_byte,
            start_row,
            start_col,
            old_end_row: start_row,
            old_end_col: start_col,
            new_end_row,
            new_end_col,
        }
    }

    /// Create an edit for a deletion (no content inserted).
    ///
    /// When deleting, `new_end` equals `start` since nothing was inserted.
    #[must_use]
    pub const fn delete(
        start_byte: usize,
        start_row: u32,
        start_col: u32,
        old_end_byte: usize,
        old_end_row: u32,
        old_end_col: u32,
    ) -> Self {
        Self {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte,
            start_row,
            start_col,
            old_end_row,
            old_end_col,
            new_end_row: start_row,
            new_end_col: start_col,
        }
    }

    /// Get the byte range that was affected by this edit.
    ///
    /// Returns the range from start to the maximum of old and new end.
    #[must_use]
    pub const fn affected_range(&self) -> Range<usize> {
        let end = if self.old_end_byte > self.new_end_byte {
            self.old_end_byte
        } else {
            self.new_end_byte
        };
        self.start_byte..end
    }

    /// Get the number of bytes that were removed.
    #[must_use]
    pub const fn bytes_removed(&self) -> usize {
        self.old_end_byte - self.start_byte
    }

    /// Get the number of bytes that were inserted.
    #[must_use]
    pub const fn bytes_inserted(&self) -> usize {
        self.new_end_byte - self.start_byte
    }

    /// Get the net change in bytes (positive = growth, negative = shrink).
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub const fn byte_delta(&self) -> isize {
        self.new_end_byte as isize - self.old_end_byte as isize
    }

    /// Check if this edit is an insertion (no removal).
    #[must_use]
    pub const fn is_insert(&self) -> bool {
        self.old_end_byte == self.start_byte
    }

    /// Check if this edit is a deletion (no insertion).
    #[must_use]
    pub const fn is_delete(&self) -> bool {
        self.new_end_byte == self.start_byte
    }

    /// Check if this edit is a replacement (both removal and insertion).
    #[must_use]
    pub const fn is_replace(&self) -> bool {
        self.old_end_byte != self.start_byte && self.new_end_byte != self.start_byte
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
