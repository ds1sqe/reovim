//! Edit conversion utilities for incremental parsing
//!
//! Converts buffer edit operations to tree-sitter's `InputEdit` for incremental re-parsing.

use tree_sitter::{InputEdit, Point};

/// Represents a text edit operation on a buffer
#[derive(Debug, Clone)]
pub struct BufferEdit {
    /// Byte offset where the edit starts
    pub start_byte: usize,
    /// Byte offset where the old content ended
    pub old_end_byte: usize,
    /// Byte offset where the new content ends
    pub new_end_byte: usize,
    /// Position (row, column) where the edit starts
    pub start_position: (u32, u32),
    /// Position where the old content ended
    pub old_end_position: (u32, u32),
    /// Position where the new content ends
    pub new_end_position: (u32, u32),
}

impl BufferEdit {
    /// Create an edit for inserting a single character
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn insert_char(
        start_byte: usize,
        start_row: u32,
        start_col: u32,
        char_len: usize,
    ) -> Self {
        Self {
            start_byte,
            old_end_byte: start_byte,
            new_end_byte: start_byte + char_len,
            start_position: (start_row, start_col),
            old_end_position: (start_row, start_col),
            new_end_position: (start_row, start_col + char_len as u32),
        }
    }

    /// Create an edit for inserting a newline
    #[must_use]
    pub const fn insert_newline(start_byte: usize, start_row: u32, start_col: u32) -> Self {
        Self {
            start_byte,
            old_end_byte: start_byte,
            new_end_byte: start_byte + 1, // newline is 1 byte
            start_position: (start_row, start_col),
            old_end_position: (start_row, start_col),
            new_end_position: (start_row + 1, 0),
        }
    }

    /// Create an edit for deleting a character backward (backspace)
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn delete_char_backward(
        cursor_byte: usize,
        cursor_row: u32,
        cursor_col: u32,
        deleted_char_len: usize,
        was_newline: bool,
        prev_line_len: u32,
    ) -> Self {
        let start_byte = cursor_byte.saturating_sub(deleted_char_len);

        if was_newline {
            // Deleting a newline joins lines
            Self {
                start_byte,
                old_end_byte: cursor_byte,
                new_end_byte: start_byte,
                start_position: (cursor_row.saturating_sub(1), prev_line_len),
                old_end_position: (cursor_row, cursor_col),
                new_end_position: (cursor_row.saturating_sub(1), prev_line_len),
            }
        } else {
            Self {
                start_byte,
                old_end_byte: cursor_byte,
                new_end_byte: start_byte,
                start_position: (cursor_row, cursor_col.saturating_sub(deleted_char_len as u32)),
                old_end_position: (cursor_row, cursor_col),
                new_end_position: (cursor_row, cursor_col.saturating_sub(deleted_char_len as u32)),
            }
        }
    }

    /// Create an edit for deleting a character forward (delete key)
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn delete_char_forward(
        cursor_byte: usize,
        cursor_row: u32,
        cursor_col: u32,
        deleted_char_len: usize,
        was_newline: bool,
    ) -> Self {
        if was_newline {
            // Deleting a newline joins current line with next
            Self {
                start_byte: cursor_byte,
                old_end_byte: cursor_byte + deleted_char_len,
                new_end_byte: cursor_byte,
                start_position: (cursor_row, cursor_col),
                old_end_position: (cursor_row + 1, 0),
                new_end_position: (cursor_row, cursor_col),
            }
        } else {
            Self {
                start_byte: cursor_byte,
                old_end_byte: cursor_byte + deleted_char_len,
                new_end_byte: cursor_byte,
                start_position: (cursor_row, cursor_col),
                old_end_position: (cursor_row, cursor_col + deleted_char_len as u32),
                new_end_position: (cursor_row, cursor_col),
            }
        }
    }

    /// Create an edit for deleting an entire line
    #[must_use]
    pub const fn delete_line(
        line_start_byte: usize,
        line_end_byte: usize,
        line_row: u32,
        line_len: u32,
        is_last_line: bool,
    ) -> Self {
        if is_last_line {
            // Last line: just delete content, no newline after
            Self {
                start_byte: line_start_byte,
                old_end_byte: line_end_byte,
                new_end_byte: line_start_byte,
                start_position: (line_row, 0),
                old_end_position: (line_row, line_len),
                new_end_position: (line_row, 0),
            }
        } else {
            // Not last line: delete content + newline
            Self {
                start_byte: line_start_byte,
                old_end_byte: line_end_byte + 1, // +1 for newline
                new_end_byte: line_start_byte,
                start_position: (line_row, 0),
                old_end_position: (line_row + 1, 0),
                new_end_position: (line_row, 0),
            }
        }
    }

    /// Create an edit for a range deletion (visual mode, etc.)
    #[must_use]
    pub const fn delete_range(
        start_byte: usize,
        end_byte: usize,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Self {
        Self {
            start_byte,
            old_end_byte: end_byte,
            new_end_byte: start_byte,
            start_position: (start_row, start_col),
            old_end_position: (end_row, end_col),
            new_end_position: (start_row, start_col),
        }
    }

    /// Create an edit for replacing a range with new text
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn replace_range(
        start_byte: usize,
        old_end_byte: usize,
        new_text_len: usize,
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
            new_end_byte: start_byte + new_text_len,
            start_position: (start_row, start_col),
            old_end_position: (old_end_row, old_end_col),
            new_end_position: (new_end_row, new_end_col),
        }
    }

    /// Convert to tree-sitter's `InputEdit`
    #[must_use]
    pub const fn to_input_edit(&self) -> InputEdit {
        InputEdit {
            start_byte: self.start_byte,
            old_end_byte: self.old_end_byte,
            new_end_byte: self.new_end_byte,
            start_position: Point {
                row: self.start_position.0 as usize,
                column: self.start_position.1 as usize,
            },
            old_end_position: Point {
                row: self.old_end_position.0 as usize,
                column: self.old_end_position.1 as usize,
            },
            new_end_position: Point {
                row: self.new_end_position.0 as usize,
                column: self.new_end_position.1 as usize,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_char() {
        let edit = BufferEdit::insert_char(10, 0, 10, 1);
        let input_edit = edit.to_input_edit();

        assert_eq!(input_edit.start_byte, 10);
        assert_eq!(input_edit.old_end_byte, 10);
        assert_eq!(input_edit.new_end_byte, 11);
        assert_eq!(input_edit.start_position, Point { row: 0, column: 10 });
        assert_eq!(
            input_edit.old_end_position,
            Point { row: 0, column: 10 }
        );
        assert_eq!(
            input_edit.new_end_position,
            Point { row: 0, column: 11 }
        );
    }

    #[test]
    fn test_insert_newline() {
        let edit = BufferEdit::insert_newline(15, 1, 5);
        let input_edit = edit.to_input_edit();

        assert_eq!(input_edit.start_byte, 15);
        assert_eq!(input_edit.old_end_byte, 15);
        assert_eq!(input_edit.new_end_byte, 16);
        assert_eq!(input_edit.start_position, Point { row: 1, column: 5 });
        assert_eq!(input_edit.new_end_position, Point { row: 2, column: 0 });
    }

    #[test]
    fn test_delete_char_backward() {
        let edit = BufferEdit::delete_char_backward(11, 0, 11, 1, false, 0);
        let input_edit = edit.to_input_edit();

        assert_eq!(input_edit.start_byte, 10);
        assert_eq!(input_edit.old_end_byte, 11);
        assert_eq!(input_edit.new_end_byte, 10);
        assert_eq!(input_edit.start_position, Point { row: 0, column: 10 });
        assert_eq!(
            input_edit.old_end_position,
            Point { row: 0, column: 11 }
        );
        assert_eq!(
            input_edit.new_end_position,
            Point { row: 0, column: 10 }
        );
    }

    #[test]
    fn test_delete_newline_backward() {
        // Cursor at start of line 1, deleting the newline from line 0
        let edit = BufferEdit::delete_char_backward(11, 1, 0, 1, true, 10);
        let input_edit = edit.to_input_edit();

        assert_eq!(input_edit.start_byte, 10);
        assert_eq!(input_edit.old_end_byte, 11);
        assert_eq!(input_edit.new_end_byte, 10);
        assert_eq!(input_edit.start_position, Point { row: 0, column: 10 });
        assert_eq!(input_edit.old_end_position, Point { row: 1, column: 0 });
        assert_eq!(
            input_edit.new_end_position,
            Point { row: 0, column: 10 }
        );
    }
}
