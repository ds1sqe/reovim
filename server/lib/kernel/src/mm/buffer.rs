//! Buffer data structure for text storage.
//!
//! The buffer is the core abstraction for text editing. It stores
//! text as lines and provides efficient operations for insertion,
//! deletion, and navigation.
//!
//! # Cursor Isolation (#471)
//!
//! **Buffer does NOT track cursor position.** Cursor is per-client UI state
//! that lives in `Window`, not in the shared kernel Buffer. This follows the
//! mechanism vs policy principle:
//! - **Mechanism**: Buffer provides text storage operations
//! - **Policy**: Window/Session decides cursor position (per-client)
//!
//! Use `SessionRuntime::cursor_position(buffer_id)` to get cursor from Window.

use std::hash::{Hash, Hasher};

use super::{BufferId, Position};

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
/// # Cursor Isolation (#471)
///
/// Buffer does NOT have a cursor field. Cursor is per-client state in Window.
/// All edit operations take explicit positions instead of using an internal cursor.
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
/// buf.insert_at(Position::new(0, 5), "!");
/// assert_eq!(buf.line(0), Some("Hello!"));
/// ```
#[derive(Debug, Clone)]
pub struct Buffer {
    /// Unique identifier for this buffer.
    id: BufferId,
    /// Text content stored as lines.
    lines: Vec<String>,
    /// Whether the buffer has unsaved modifications.
    modified: bool,
    /// File path associated with this buffer.
    file_path: Option<String>,
    // NOTE: Cursor removed in #471 (per-client cursor isolation).
    // Cursor now lives in Window (per-client state), not Buffer.
    // See: server/lib/drivers/session/src/types.rs - Window.cursor
    //
    // NOTE: Selection removed in Phase 8 (#465).
    // Selection now lives in Window (per-window state), not Buffer.
    // See: server/lib/drivers/session/src/types.rs - Window.selection
}

impl Buffer {
    /// Create a new empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: BufferId::new(),
            lines: Vec::new(),
            modified: false,
            file_path: None,
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
            modified: false,
            file_path: None,
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
            modified: false,
            file_path: None,
        }
    }

    // === Accessors ===

    /// Get the buffer ID.
    #[must_use]
    pub const fn id(&self) -> BufferId {
        self.id
    }

    // NOTE: cursor(), cursor_mut(), position(), set_position() removed in #471.
    // Cursor is per-client state in Window, not Buffer.
    // Use SessionRuntime::cursor_position(buffer_id) via BufferApi trait.

    /// Check if the buffer has unsaved modifications.
    #[must_use]
    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    /// Mark the buffer as modified or unmodified.
    pub const fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    // NOTE: Selection methods removed in Phase 8 (#465).
    // Selection now lives in Window (per-window state), not Buffer.
    // Use SessionRuntime::selection(buffer_id) via BufferApi trait.

    // === File Path ===

    /// Get the file path associated with this buffer.
    #[must_use]
    pub fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    /// Set the file path for this buffer.
    pub fn set_file_path(&mut self, path: Option<String>) {
        self.file_path = path;
    }

    // === Line Hashing ===

    /// Compute hash of a line for cache validation.
    ///
    /// Uses `DefaultHasher` for speed over cryptographic strength.
    /// Returns `None` if line index is out of bounds.
    #[must_use]
    pub fn line_hash(&self, line_idx: usize) -> Option<u64> {
        use std::collections::hash_map::DefaultHasher;

        self.line(line_idx).map(|line| {
            let mut hasher = DefaultHasher::new();
            line.hash(&mut hasher);
            hasher.finish()
        })
    }

    /// Get all line hashes (for saturator requests).
    #[must_use]
    pub fn line_hashes(&self) -> Vec<u64> {
        (0..self.line_count())
            .filter_map(|idx| self.line_hash(idx))
            .collect()
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
        self.modified = true;
    }

    // === Edit Operations ===
    //
    // NOTE: insert() and delete() convenience methods removed in #471.
    // These methods relied on internal cursor which is now per-client in Window.
    // Use insert_at(pos, text) and delete_at(pos, count) with explicit positions.

    /// Insert text at a specific position.
    ///
    /// This is a pure text operation - cursor management is the caller's responsibility.
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
        }

        self.modified = true;
    }

    /// Delete text at a specific position.
    ///
    /// Returns the deleted text.
    /// This is a pure text operation - cursor management is the caller's responsibility.
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

#[cfg(test)]
mod tests {
    use super::*;

    // === Construction ===

    #[test]
    fn new_buffer_is_empty() {
        let buf = Buffer::new();
        assert_eq!(buf.line_count(), 0);
        assert!(buf.is_empty());
        assert!(!buf.is_modified());
        assert!(buf.file_path().is_none());
    }

    #[test]
    fn with_id() {
        let id = BufferId::from_raw(42);
        let buf = Buffer::with_id(id);
        assert_eq!(buf.id(), id);
        assert_eq!(buf.line_count(), 0);
        assert!(buf.is_empty());
        assert!(!buf.is_modified());
    }


    #[test]
    fn from_string_multi_line() {
        let buf = Buffer::from_string("Hello\nWorld\nTest");
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.line(0), Some("Hello"));
        assert_eq!(buf.line(1), Some("World"));
        assert_eq!(buf.line(2), Some("Test"));
    }


    #[test]
    fn from_string_not_modified() {
        let buf = Buffer::from_string("content");
        assert!(!buf.is_modified());
    }

    #[test]
    fn default_is_new() {
        let buf = Buffer::default();
        assert_eq!(buf.line_count(), 0);
        assert!(buf.is_empty());
    }

    // === Accessors ===

    #[test]
    fn modified_flag() {
        let mut buf = Buffer::from_string("test");
        assert!(!buf.is_modified());

        buf.set_modified(true);
        assert!(buf.is_modified());

        buf.set_modified(false);
        assert!(!buf.is_modified());
    }

    #[test]
    fn file_path_none_by_default() {
        let buf = Buffer::new();
        assert!(buf.file_path().is_none());
    }

    #[test]
    fn set_and_get_file_path() {
        let mut buf = Buffer::new();
        buf.set_file_path(Some("/tmp/test.txt".to_string()));
        assert_eq!(buf.file_path(), Some("/tmp/test.txt"));

        buf.set_file_path(None);
        assert!(buf.file_path().is_none());
    }

    // === Line Access ===

    #[test]
    fn line_out_of_bounds() {
        let buf = Buffer::from_string("Hello");
        assert!(buf.line(1).is_none());
        assert!(buf.line(100).is_none());
    }

    #[test]
    fn line_len_valid() {
        let buf = Buffer::from_string("Hello\nWorld!");
        assert_eq!(buf.line_len(0), Some(5));
        assert_eq!(buf.line_len(1), Some(6));
    }

    #[test]
    fn line_len_out_of_bounds() {
        let buf = Buffer::from_string("Hello");
        assert!(buf.line_len(1).is_none());
    }

    #[test]
    fn line_len_unicode() {
        // "H\u{00e9}llo" = H + e-acute + l + l + o = 5 chars
        let buf = Buffer::from_string("H\u{00e9}llo");
        assert_eq!(buf.line_len(0), Some(5));
    }

    #[test]
    fn lines_accessor() {
        let buf = Buffer::from_string("A\nB\nC");
        let lines = buf.lines();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "A");
        assert_eq!(lines[1], "B");
        assert_eq!(lines[2], "C");
    }


    #[test]
    fn content_empty() {
        let buf = Buffer::new();
        assert_eq!(buf.content(), "");
    }

    #[test]
    fn content_single_line() {
        let buf = Buffer::from_string("Hello");
        assert_eq!(buf.content(), "Hello");
    }

    // === set_content ===

    #[test]
    fn set_content_replaces_all() {
        let mut buf = Buffer::from_string("old");
        buf.set_content("new\ncontent");
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.line(0), Some("new"));
        assert_eq!(buf.line(1), Some("content"));
        assert!(buf.is_modified());
    }

    #[test]
    fn set_content_empty_clears_lines() {
        let mut buf = Buffer::from_string("Hello\nWorld");
        buf.set_content("");
        assert_eq!(buf.line_count(), 0);
        assert!(buf.is_empty());
        assert!(buf.is_modified());
    }

    // === Line Hashing ===

    #[test]
    fn line_hash_valid_line() {
        let buf = Buffer::from_string("Hello\nWorld");
        let hash0 = buf.line_hash(0);
        let hash1 = buf.line_hash(1);
        assert!(hash0.is_some());
        assert!(hash1.is_some());
        // Different content should produce different hashes (probabilistically)
        assert_ne!(hash0, hash1);
    }

    #[test]
    fn line_hash_out_of_bounds() {
        let buf = Buffer::from_string("Hello");
        assert!(buf.line_hash(1).is_none());
    }

    #[test]
    fn line_hash_consistent() {
        let buf = Buffer::from_string("Hello");
        let hash1 = buf.line_hash(0);
        let hash2 = buf.line_hash(0);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn line_hashes_returns_all() {
        let buf = Buffer::from_string("A\nB\nC");
        let hashes = buf.line_hashes();
        assert_eq!(hashes.len(), 3);
    }

    #[test]
    fn line_hashes_empty_buffer() {
        let buf = Buffer::new();
        let hashes = buf.line_hashes();
        assert!(hashes.is_empty());
    }

    // === insert_at ===

    #[test]
    fn insert_at_empty_text_noop() {
        let mut buf = Buffer::from_string("Hello");
        buf.insert_at(Position::new(0, 0), "");
        assert_eq!(buf.line(0), Some("Hello"));
        assert!(!buf.is_modified());
    }

    #[test]
    fn insert_at_empty_buffer() {
        let mut buf = Buffer::new();
        buf.insert_at(Position::origin(), "Hello");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line(0), Some("Hello"));
        assert!(buf.is_modified());
    }

    #[test]
    fn insert_at_end() {
        let mut buf = Buffer::from_string("Hello");
        buf.insert_at(Position::new(0, 5), " World");
        assert_eq!(buf.line(0), Some("Hello World"));
    }

    #[test]
    fn insert_at_clamped_position() {
        let mut buf = Buffer::from_string("Hello");
        // Column past end of line should clamp
        buf.insert_at(Position::new(0, 100), "!");
        assert_eq!(buf.line(0), Some("Hello!"));
    }

    #[test]
    fn insert_at_clamped_line() {
        let mut buf = Buffer::from_string("Hello");
        // Line past end of buffer clamps to last line; col 0 inserts at start
        buf.insert_at(Position::new(100, 0), "!");
        assert_eq!(buf.line(0), Some("!Hello"));
    }

    #[test]
    fn insert_marks_modified() {
        let mut buf = Buffer::from_string("test");
        buf.insert_at(Position::new(0, 0), "x");
        assert!(buf.is_modified());
    }

    #[test]
    fn insert_unicode() {
        let mut buf = Buffer::from_string("H\u{00e9}llo");
        buf.insert_at(Position::new(0, 5), " World");
        assert_eq!(buf.line(0), Some("H\u{00e9}llo World"));
    }

    // === delete_at ===

    #[test]
    fn delete_at_zero_count() {
        let mut buf = Buffer::from_string("Hello");
        let deleted = buf.delete_at(Position::new(0, 0), 0);
        assert_eq!(deleted, "");
        assert_eq!(buf.line(0), Some("Hello"));
        assert!(!buf.is_modified());
    }

    #[test]
    fn delete_at_empty_buffer() {
        let mut buf = Buffer::new();
        let deleted = buf.delete_at(Position::origin(), 5);
        assert_eq!(deleted, "");
    }

    #[test]
    fn delete_at_single_char() {
        let mut buf = Buffer::from_string("Hello");
        let deleted = buf.delete_at(Position::new(0, 0), 1);
        assert_eq!(deleted, "H");
        assert_eq!(buf.line(0), Some("ello"));
    }

    #[test]
    fn delete_at_multiple_chars() {
        let mut buf = Buffer::from_string("Hello World");
        let deleted = buf.delete_at(Position::new(0, 0), 6);
        assert_eq!(deleted, "Hello ");
        assert_eq!(buf.line(0), Some("World"));
    }

    #[test]
    fn delete_at_newline_merges_lines() {
        let mut buf = Buffer::from_string("Hello\nWorld");
        let deleted = buf.delete_at(Position::new(0, 5), 1);
        assert_eq!(deleted, "\n");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line(0), Some("HelloWorld"));
    }

    #[test]
    fn delete_at_across_lines() {
        let mut buf = Buffer::from_string("Hello\nWorld");
        let deleted = buf.delete_at(Position::new(0, 3), 5);
        assert_eq!(deleted, "lo\nWo");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line(0), Some("Helrld"));
    }


    #[test]
    fn delete_at_end_of_line_no_next() {
        let mut buf = Buffer::from_string("Hello");
        // At end of last line, nothing more to delete
        let deleted = buf.delete_at(Position::new(0, 5), 1);
        assert_eq!(deleted, "");
    }

    #[test]
    fn delete_at_marks_modified() {
        let mut buf = Buffer::from_string("test");
        buf.delete_at(Position::new(0, 0), 1);
        assert!(buf.is_modified());
    }

    #[test]
    fn delete_at_nothing_deleted_not_modified() {
        let mut buf = Buffer::from_string("test");
        let deleted = buf.delete_at(Position::new(0, 4), 0);
        assert_eq!(deleted, "");
        assert!(!buf.is_modified());
    }

    // === delete_range ===


    // === position_to_byte ===

    #[test]
    fn position_to_byte_start() {
        let buf = Buffer::from_string("Hello\nWorld");
        assert_eq!(buf.position_to_byte(Position::new(0, 0)), 0);
    }

    #[test]
    fn position_to_byte_mid_line() {
        let buf = Buffer::from_string("Hello\nWorld");
        assert_eq!(buf.position_to_byte(Position::new(0, 5)), 5);
    }

    #[test]
    fn position_to_byte_second_line() {
        let buf = Buffer::from_string("Hello\nWorld");
        assert_eq!(buf.position_to_byte(Position::new(1, 0)), 6);
        assert_eq!(buf.position_to_byte(Position::new(1, 5)), 11);
    }

    #[test]
    fn position_to_byte_empty_buffer() {
        let buf = Buffer::new();
        assert_eq!(buf.position_to_byte(Position::new(0, 0)), 0);
    }

    #[test]
    fn position_to_byte_unicode() {
        // "H\u{00e9}llo" -> H(1 byte) + e-acute(2 bytes) + l(1) + l(1) + o(1) = 6 bytes
        let buf = Buffer::from_string("H\u{00e9}llo");
        assert_eq!(buf.position_to_byte(Position::new(0, 0)), 0); // H
        assert_eq!(buf.position_to_byte(Position::new(0, 1)), 1); // start of e-acute
        assert_eq!(buf.position_to_byte(Position::new(0, 2)), 3); // after 2-byte char
    }

    // === byte_to_position ===

    #[test]
    fn byte_to_position_start() {
        let buf = Buffer::from_string("Hello\nWorld");
        assert_eq!(buf.byte_to_position(0), Position::new(0, 0));
    }

    #[test]
    fn byte_to_position_mid_line() {
        let buf = Buffer::from_string("Hello\nWorld");
        assert_eq!(buf.byte_to_position(3), Position::new(0, 3));
    }

    #[test]
    fn byte_to_position_at_newline() {
        let buf = Buffer::from_string("Hello\nWorld");
        // byte 5 is at end of "Hello" (at the newline)
        assert_eq!(buf.byte_to_position(5), Position::new(0, 5));
    }

    #[test]
    fn byte_to_position_second_line() {
        let buf = Buffer::from_string("Hello\nWorld");
        assert_eq!(buf.byte_to_position(6), Position::new(1, 0));
        assert_eq!(buf.byte_to_position(11), Position::new(1, 5));
    }

    #[test]
    fn byte_to_position_past_end() {
        let buf = Buffer::from_string("Hello\nWorld");
        // Past end should clamp to last position
        let result = buf.byte_to_position(100);
        assert_eq!(result, Position::new(1, 5));
    }

    #[test]
    fn byte_to_position_empty_buffer() {
        let buf = Buffer::new();
        assert_eq!(buf.byte_to_position(0), Position::new(0, 0));
    }

    #[test]
    fn byte_position_roundtrip() {
        let buf = Buffer::from_string("Hello\nWorld\nTest");
        for line in 0..buf.line_count() {
            for col in 0..=buf.line_len(line).unwrap() {
                let pos = Position::new(line, col);
                let byte = buf.position_to_byte(pos);
                let back = buf.byte_to_position(byte);
                assert_eq!(pos, back, "Roundtrip failed for {pos:?} (byte={byte})");
            }
        }
    }

    #[test]
    fn byte_position_roundtrip_unicode() {
        let buf = Buffer::from_string("H\u{00e9}llo\nWorld");
        for line in 0..buf.line_count() {
            for col in 0..=buf.line_len(line).unwrap() {
                let pos = Position::new(line, col);
                let byte = buf.position_to_byte(pos);
                let back = buf.byte_to_position(byte);
                assert_eq!(pos, back, "Unicode roundtrip failed for {pos:?}");
            }
        }
    }

    // === Helper functions ===

    #[test]
    fn char_to_byte_offset_ascii() {
        assert_eq!(char_to_byte_offset("hello", 0), 0);
        assert_eq!(char_to_byte_offset("hello", 3), 3);
        assert_eq!(char_to_byte_offset("hello", 5), 5); // past end
    }

    #[test]
    fn char_to_byte_offset_unicode() {
        // "H\u{00e9}llo" -> H(1) + e-acute(2) + l(1) + l(1) + o(1)
        assert_eq!(char_to_byte_offset("H\u{00e9}llo", 0), 0);
        assert_eq!(char_to_byte_offset("H\u{00e9}llo", 1), 1);
        assert_eq!(char_to_byte_offset("H\u{00e9}llo", 2), 3); // skip 2-byte char
    }

    #[test]
    fn char_to_byte_offset_past_end() {
        assert_eq!(char_to_byte_offset("hi", 10), 2); // returns line.len()
    }

    #[test]
    fn char_to_byte_offset_empty() {
        assert_eq!(char_to_byte_offset("", 0), 0);
    }

    #[test]
    fn byte_to_char_offset_ascii() {
        assert_eq!(byte_to_char_offset("hello", 0), 0);
        assert_eq!(byte_to_char_offset("hello", 3), 3);
        assert_eq!(byte_to_char_offset("hello", 5), 5);
    }

    #[test]
    fn byte_to_char_offset_unicode() {
        // "H\u{00e9}llo" -> byte 3 is char index 2
        assert_eq!(byte_to_char_offset("H\u{00e9}llo", 0), 0);
        assert_eq!(byte_to_char_offset("H\u{00e9}llo", 1), 1);
        assert_eq!(byte_to_char_offset("H\u{00e9}llo", 3), 2); // past the 2-byte char
    }

    #[test]
    fn byte_to_char_offset_empty() {
        assert_eq!(byte_to_char_offset("", 0), 0);
    }

    // === Clone ===

    #[test]
    fn buffer_clone() {
        let mut buf = Buffer::from_string("Hello");
        buf.set_file_path(Some("/test".to_string()));
        buf.set_modified(true);

        let cloned = buf.clone();
        assert_eq!(cloned.id(), buf.id());
        assert_eq!(cloned.content(), buf.content());
        assert_eq!(cloned.file_path(), buf.file_path());
        assert_eq!(cloned.is_modified(), buf.is_modified());
    }

    // === Clamp position edge cases ===

    #[test]
    fn insert_at_clamped_empty_buffer_with_text() {
        let mut buf = Buffer::new();
        // Inserting into empty buffer at (10, 10) should clamp to (0, 0)
        buf.insert_at(Position::new(10, 10), "text");
        assert_eq!(buf.line(0), Some("text"));
    }

    #[test]
    fn delete_at_clamped() {
        let mut buf = Buffer::from_string("Hello");
        // Delete from past-end position: should clamp
        let deleted = buf.delete_at(Position::new(0, 100), 1);
        // Clamped to (0, 5) which is end of line, nothing to delete on current line
        // but it won't crash
        assert_eq!(deleted, "");
    }

    // === Multiple operations ===

    #[test]
    fn insert_then_delete() {
        let mut buf = Buffer::from_string("Hello");
        buf.insert_at(Position::new(0, 5), " World");
        assert_eq!(buf.content(), "Hello World");

        buf.delete_at(Position::new(0, 5), 6);
        assert_eq!(buf.content(), "Hello");
    }

    #[test]
    fn multiple_newline_inserts() {
        let mut buf = Buffer::new();
        buf.insert_at(Position::origin(), "Line1\nLine2\nLine3");
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.line(0), Some("Line1"));
        assert_eq!(buf.line(1), Some("Line2"));
        assert_eq!(buf.line(2), Some("Line3"));
    }
}
