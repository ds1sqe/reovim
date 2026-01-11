//! Buffer data structure for text storage.
//!
//! The buffer is the core abstraction for text editing. It stores
//! text as lines and provides efficient operations for insertion,
//! deletion, and navigation.

use super::{BufferId, Cursor, Edit, Position};

/// A text buffer with line-based storage.
///
/// The buffer stores text as a vector of lines, where each line is a String
/// without the trailing newline character. This provides efficient line-based
/// access while maintaining a simple implementation.
///
/// # Invariants
///
/// - An empty buffer has zero lines (not one empty line)
/// - Lines do not contain newline characters
/// - Positions are clamped to valid ranges on access
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let mut buf = Buffer::from_string("Hello\nWorld");
/// assert_eq!(buf.line_count(), 2);
/// assert_eq!(buf.line(0), Some("Hello"));
///
/// buf.set_position(Position::new(0, 5));
/// buf.insert("!");
/// assert_eq!(buf.line(0), Some("Hello!"));
/// ```
#[derive(Debug, Clone)]
pub struct Buffer {
    /// Unique identifier for this buffer.
    id: BufferId,
    /// Text content stored as lines.
    lines: Vec<String>,
    /// Cursor state.
    cursor: Cursor,
    /// Whether the buffer has unsaved modifications.
    modified: bool,
}

impl Buffer {
    /// Create a new empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: BufferId::new(),
            lines: Vec::new(),
            cursor: Cursor::origin(),
            modified: false,
        }
    }

    /// Create a buffer with a specific ID.
    ///
    /// This is primarily useful for testing.
    #[must_use]
    pub const fn with_id(id: BufferId) -> Self {
        Self {
            id,
            lines: Vec::new(),
            cursor: Cursor::origin(),
            modified: false,
        }
    }

    /// Create a buffer from a string.
    ///
    /// The string is split by newlines into lines.
    /// An empty string results in a buffer with zero lines.
    #[must_use]
    pub fn from_string(content: &str) -> Self {
        let lines = if content.is_empty() {
            Vec::new()
        } else {
            content.lines().map(String::from).collect()
        };
        Self {
            id: BufferId::new(),
            lines,
            cursor: Cursor::origin(),
            modified: false,
        }
    }

    // === Accessors ===

    /// Get the buffer ID.
    #[must_use]
    pub const fn id(&self) -> BufferId {
        self.id
    }

    /// Get the cursor.
    #[must_use]
    pub const fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Get a mutable reference to the cursor.
    pub const fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursor
    }

    /// Get the cursor position.
    #[must_use]
    pub const fn position(&self) -> Position {
        self.cursor.position
    }

    /// Set the cursor position.
    ///
    /// The position is clamped to valid buffer coordinates.
    pub fn set_position(&mut self, pos: Position) {
        self.cursor.position = self.clamp_position(pos);
    }

    /// Check if the buffer has unsaved modifications.
    #[must_use]
    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    /// Mark the buffer as modified or unmodified.
    pub const fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    // === Line Access ===

    /// Get the number of lines in the buffer.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Check if the buffer is empty (has no lines).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get a specific line by index.
    ///
    /// Returns `None` if the index is out of bounds.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(String::as_str)
    }

    /// Get the length of a specific line in characters.
    ///
    /// Returns `None` if the index is out of bounds.
    #[must_use]
    pub fn line_len(&self, index: usize) -> Option<usize> {
        self.lines.get(index).map(|l| l.chars().count())
    }

    /// Get all lines as a slice.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Get the full content as a string (lines joined with newlines).
    #[must_use]
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// Set the full content from a string.
    ///
    /// This replaces all existing content and resets the cursor.
    pub fn set_content(&mut self, content: &str) {
        self.lines = if content.is_empty() {
            Vec::new()
        } else {
            content.lines().map(String::from).collect()
        };
        self.cursor = Cursor::origin();
        self.modified = true;
    }

    // === Edit Operations ===

    /// Insert text at the current cursor position.
    ///
    /// Returns an [`Edit`] that can be used for undo support.
    /// The cursor is moved to the end of the inserted text.
    pub fn insert(&mut self, text: &str) -> Edit {
        let pos = self.cursor.position;
        self.insert_at(pos, text);
        Edit::insert(pos, text)
    }

    /// Insert text at a specific position.
    ///
    /// The cursor is moved to the end of the inserted text.
    pub fn insert_at(&mut self, pos: Position, text: &str) {
        if text.is_empty() {
            return;
        }

        // Ensure we have at least one line
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }

        let pos = self.clamp_position(pos);
        let line_idx = pos.line;
        let col = pos.column;

        // Get the current line and split at insertion point
        let current_line = &self.lines[line_idx];
        let byte_offset = char_to_byte_offset(current_line, col);
        let (before, after) = current_line.split_at(byte_offset);
        let before = before.to_string();
        let after = after.to_string();

        // Handle single-line vs multi-line insertion
        let insert_lines: Vec<&str> = text.split('\n').collect();

        if insert_lines.len() == 1 {
            // Single line: just insert in place
            self.lines[line_idx] = format!("{before}{text}{after}");
            self.cursor.position = Position::new(line_idx, col + text.chars().count());
        } else {
            // Multi-line: split and insert
            // First line gets before + first insert part
            let first_insert = insert_lines[0];
            self.lines[line_idx] = format!("{before}{first_insert}");

            // Last line gets last insert part + after
            let last_insert = insert_lines[insert_lines.len() - 1];
            let last_line = format!("{last_insert}{after}");

            // Insert middle lines and last line
            let insert_pos = line_idx + 1;
            self.lines.splice(
                insert_pos..insert_pos,
                insert_lines[1..insert_lines.len() - 1]
                    .iter()
                    .map(|s| (*s).to_string())
                    .chain(std::iter::once(last_line)),
            );

            // Update cursor to end of inserted text
            let new_line_idx = line_idx + insert_lines.len() - 1;
            self.cursor.position = Position::new(new_line_idx, last_insert.chars().count());
        }

        self.modified = true;
    }

    /// Delete text from the current cursor position.
    ///
    /// Deletes `count` characters forward.
    /// Returns an [`Edit`] containing the deleted text for undo support.
    pub fn delete(&mut self, count: usize) -> Edit {
        let pos = self.cursor.position;
        let text = self.delete_at(pos, count);
        Edit::delete(pos, text)
    }

    /// Delete text at a specific position.
    ///
    /// Returns the deleted text.
    pub fn delete_at(&mut self, pos: Position, count: usize) -> String {
        if count == 0 || self.lines.is_empty() {
            return String::new();
        }

        let pos = self.clamp_position(pos);
        let mut deleted = String::new();
        let mut remaining = count;
        let current_line = pos.line;
        let current_col = pos.column;

        while remaining > 0 && current_line < self.lines.len() {
            let line = &self.lines[current_line];
            let chars: Vec<char> = line.chars().collect();
            let chars_in_line = chars.len();

            if current_col >= chars_in_line {
                // At end of line, delete the newline (merge with next line)
                if current_line + 1 < self.lines.len() {
                    deleted.push('\n');
                    let next_line = self.lines.remove(current_line + 1);
                    self.lines[current_line].push_str(&next_line);
                    remaining -= 1;
                } else {
                    // Nothing more to delete
                    break;
                }
            } else {
                // Delete characters in current line
                let chars_to_delete = remaining.min(chars_in_line - current_col);
                let delete_chars: String = chars[current_col..current_col + chars_to_delete]
                    .iter()
                    .collect();
                deleted.push_str(&delete_chars);

                // Rebuild the line without deleted chars
                let new_line: String = chars[..current_col]
                    .iter()
                    .chain(chars[current_col + chars_to_delete..].iter())
                    .collect();
                self.lines[current_line] = new_line;

                remaining -= chars_to_delete;
            }
        }

        // Update cursor position to deletion start
        self.cursor.position = self.clamp_position(pos);

        if !deleted.is_empty() {
            self.modified = true;
        }

        deleted
    }

    /// Delete a range of text.
    ///
    /// Returns the deleted text.
    pub fn delete_range(&mut self, start: Position, end: Position) -> String {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        let start = self.clamp_position(start);
        let end = self.clamp_position(end);

        // Calculate character count between positions
        let count = self.char_count_between(start, end);
        self.delete_at(start, count)
    }

    // === Position Conversion ===

    /// Convert a position to a byte offset in the full content.
    ///
    /// This is useful for tree-sitter and other byte-based APIs.
    #[must_use]
    pub fn position_to_byte(&self, pos: Position) -> usize {
        let pos = self.clamp_position(pos);
        let mut offset = 0;

        for (i, line) in self.lines.iter().enumerate() {
            if i < pos.line {
                offset += line.len() + 1; // +1 for newline
            } else if i == pos.line {
                // Add bytes up to column
                offset += char_to_byte_offset(line, pos.column);
                break;
            }
        }

        offset
    }

    /// Convert a byte offset to a position.
    #[must_use]
    pub fn byte_to_position(&self, byte_offset: usize) -> Position {
        let mut remaining = byte_offset;

        for (line_idx, line) in self.lines.iter().enumerate() {
            let line_bytes = line.len();
            let line_total = line_bytes + 1; // +1 for newline

            if remaining <= line_bytes {
                // Position is within this line
                let col = byte_to_char_offset(line, remaining);
                return Position::new(line_idx, col);
            } else if remaining < line_total {
                // Position is at end of line (on the newline)
                return Position::new(line_idx, line.chars().count());
            }

            remaining -= line_total;
        }

        // Past end of buffer
        let last_line = self.lines.len().saturating_sub(1);
        let last_col = self.lines.last().map_or(0, |l| l.chars().count());
        Position::new(last_line, last_col)
    }

    // === Helper Methods ===

    /// Clamp a position to valid buffer coordinates.
    #[must_use]
    fn clamp_position(&self, pos: Position) -> Position {
        if self.lines.is_empty() {
            return Position::origin();
        }

        let line = pos.line.min(self.lines.len() - 1);
        let max_col = self.lines[line].chars().count();
        let column = pos.column.min(max_col);

        Position::new(line, column)
    }

    /// Count characters between two positions.
    fn char_count_between(&self, start: Position, end: Position) -> usize {
        if start >= end {
            return 0;
        }

        if start.line == end.line {
            return end.column.saturating_sub(start.column);
        }

        let mut count = 0;

        // Characters from start to end of first line + newline
        if let Some(first_line) = self.lines.get(start.line) {
            count += first_line.chars().count() - start.column + 1; // +1 for newline
        }

        // Full lines in between
        for line_idx in start.line + 1..end.line {
            if let Some(line) = self.lines.get(line_idx) {
                count += line.chars().count() + 1; // +1 for newline
            }
        }

        // Characters in last line
        count += end.column;

        count
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

// === Helper Functions ===

/// Convert a character offset to a byte offset within a line.
fn char_to_byte_offset(line: &str, char_offset: usize) -> usize {
    line.char_indices()
        .nth(char_offset)
        .map_or(line.len(), |(byte_idx, _)| byte_idx)
}

/// Convert a byte offset to a character offset within a line.
fn byte_to_char_offset(line: &str, byte_offset: usize) -> usize {
    line.char_indices()
        .take_while(|(byte_idx, _)| *byte_idx < byte_offset)
        .count()
}
