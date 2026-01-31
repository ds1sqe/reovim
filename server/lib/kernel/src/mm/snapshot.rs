//! Read-only buffer snapshot for lock-free access.
//!
//! This module provides `BufferSnapshot`, a read-only capture of buffer state
//! that can be safely shared across threads without locking.
//!
//! # Design Philosophy
//!
//! Following the kernel "mechanism, not policy" principle:
//! - Snapshot captures state at a point in time
//! - No modification methods - purely read-only
//! - Thread-safe by construction (Clone, Send, Sync)
//!
//! # Use Cases
//!
//! - RPC handlers need buffer content without blocking edits
//! - Syntax highlighting processes snapshot while user edits
//! - Undo/redo can capture state before operations
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! let mut buffer = Buffer::from_string("Hello\nWorld");
//! buffer.set_position(Position::new(0, 5));
//!
//! // Capture state
//! let snapshot = BufferSnapshot::from_buffer(&buffer);
//!
//! // Snapshot is independent of buffer changes
//! buffer.insert("!");
//!
//! assert_eq!(snapshot.line_count(), 2);
//! assert_eq!(snapshot.content(), "Hello\nWorld"); // Original content
//! ```

use super::{BufferId, Cursor, Position, Selection};

/// Read-only snapshot of buffer state.
///
/// A `BufferSnapshot` captures the complete state of a buffer at a point
/// in time. It's cheap to clone and safe to share across threads.
///
/// # Immutability
///
/// All methods are read-only. The snapshot cannot be modified after
/// creation, ensuring thread safety without locks.
///
/// # Fields Captured
///
/// - `id`: Buffer identifier
/// - `lines`: All text lines
/// - `cursor`: Cursor state (position, anchor, preferred column)
/// - `selection`: Selection state
/// - `file_path`: Associated file path (if any)
/// - `modified`: Whether buffer had unsaved changes
#[derive(Debug, Clone)]
pub struct BufferSnapshot {
    /// Buffer identifier.
    pub id: BufferId,
    /// Text content as lines.
    pub lines: Vec<String>,
    /// Cursor state.
    pub cursor: Cursor,
    /// Selection state.
    pub selection: Selection,
    /// File path (if buffer is associated with a file).
    pub file_path: Option<String>,
    /// Whether buffer has unsaved modifications.
    pub modified: bool,
}

impl BufferSnapshot {
    /// Create a snapshot from a buffer.
    ///
    /// This clones all buffer state, so the snapshot is independent
    /// of subsequent buffer modifications.
    #[must_use]
    pub fn from_buffer(buffer: &super::Buffer) -> Self {
        Self {
            id: buffer.id(),
            lines: buffer.lines().to_vec(),
            cursor: *buffer.cursor(),
            selection: buffer.selection(),
            file_path: buffer.file_path().map(String::from),
            modified: buffer.is_modified(),
        }
    }

    /// Create a snapshot from individual components.
    ///
    /// This is useful for testing or when constructing a snapshot
    /// without a buffer.
    #[must_use]
    pub const fn new(
        id: BufferId,
        lines: Vec<String>,
        cursor: Cursor,
        selection: Selection,
        file_path: Option<String>,
        modified: bool,
    ) -> Self {
        Self {
            id,
            lines,
            cursor,
            selection,
            file_path,
            modified,
        }
    }

    // === Line Access ===

    /// Get the number of lines.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Check if the snapshot is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get a specific line by index.
    ///
    /// Returns `None` if index is out of bounds.
    #[must_use]
    pub fn line(&self, idx: usize) -> Option<&str> {
        self.lines.get(idx).map(String::as_str)
    }

    /// Get the length of a line in characters.
    #[must_use]
    pub fn line_len(&self, idx: usize) -> Option<usize> {
        self.lines.get(idx).map(|l| l.chars().count())
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

    // === Text Extraction ===

    /// Extract text within a range.
    ///
    /// Returns the text between `start` and `end` positions.
    /// Positions are clamped to valid bounds.
    #[must_use]
    pub fn text_in_range(&self, start: Position, end: Position) -> String {
        if self.lines.is_empty() {
            return String::new();
        }

        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        // Clamp positions
        let start_line = start.line.min(self.lines.len() - 1);
        let end_line = end.line.min(self.lines.len() - 1);

        if start_line == end_line {
            // Single line extraction
            if let Some(line) = self.line(start_line) {
                let chars: Vec<char> = line.chars().collect();
                let start_col = start.column.min(chars.len());
                let end_col = end.column.min(chars.len());
                return chars[start_col..end_col].iter().collect();
            }
            return String::new();
        }

        // Multi-line extraction
        let mut result = String::new();

        for line_idx in start_line..=end_line {
            if let Some(line) = self.line(line_idx) {
                let chars: Vec<char> = line.chars().collect();

                if line_idx == start_line {
                    // First line: from start column to end
                    let start_col = start.column.min(chars.len());
                    result.extend(&chars[start_col..]);
                    result.push('\n');
                } else if line_idx == end_line {
                    // Last line: from start to end column
                    let end_col = end.column.min(chars.len());
                    result.extend(&chars[..end_col]);
                } else {
                    // Middle line: entire line
                    result.push_str(line);
                    result.push('\n');
                }
            }
        }

        result
    }

    // === Position Queries ===

    /// Get the cursor position.
    #[must_use]
    pub const fn position(&self) -> Position {
        self.cursor.position
    }

    /// Check if a position is valid within this snapshot.
    #[must_use]
    pub fn is_valid_position(&self, pos: Position) -> bool {
        if pos.line >= self.lines.len() {
            return false;
        }
        self.line(pos.line)
            .is_some_and(|line| pos.column <= line.chars().count())
    }

    // === Selection Queries ===

    /// Get the selection bounds if selection is active.
    #[must_use]
    pub fn selection_bounds(&self) -> Option<(Position, Position)> {
        self.selection.bounds(self.cursor.position)
    }

    /// Check if selection is active.
    #[must_use]
    pub const fn has_selection(&self) -> bool {
        self.selection.is_active()
    }

    /// Get the selected text, if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_bounds()?;
        Some(self.text_in_range(start, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_buffer() -> super::super::Buffer {
        let mut buffer = super::super::Buffer::from_string("Hello\nWorld\nTest");
        buffer.set_position(Position::new(1, 2));
        buffer
    }

    #[test]
    fn test_snapshot_from_buffer() {
        let buffer = make_test_buffer();
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        assert_eq!(snapshot.id, buffer.id());
        assert_eq!(snapshot.line_count(), 3);
        assert_eq!(snapshot.position(), Position::new(1, 2));
    }

    #[test]
    fn test_snapshot_line_count() {
        let buffer = make_test_buffer();
        let snapshot = BufferSnapshot::from_buffer(&buffer);
        assert_eq!(snapshot.line_count(), 3);
    }

    #[test]
    fn test_snapshot_line_access() {
        let buffer = make_test_buffer();
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        assert_eq!(snapshot.line(0), Some("Hello"));
        assert_eq!(snapshot.line(1), Some("World"));
        assert_eq!(snapshot.line(2), Some("Test"));
    }

    #[test]
    fn test_snapshot_content() {
        let buffer = make_test_buffer();
        let snapshot = BufferSnapshot::from_buffer(&buffer);
        assert_eq!(snapshot.content(), "Hello\nWorld\nTest");
    }

    #[test]
    fn test_snapshot_text_in_range_single_line() {
        let buffer = super::super::Buffer::from_string("Hello World");
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        let text = snapshot.text_in_range(Position::new(0, 0), Position::new(0, 5));
        assert_eq!(text, "Hello");
    }

    #[test]
    fn test_snapshot_text_in_range_multi_line() {
        let buffer = make_test_buffer();
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        let text = snapshot.text_in_range(Position::new(0, 3), Position::new(1, 3));
        assert_eq!(text, "lo\nWor");
    }

    #[test]
    fn test_snapshot_empty_buffer() {
        let buffer = super::super::Buffer::new();
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        assert!(snapshot.is_empty());
        assert_eq!(snapshot.line_count(), 0);
        assert_eq!(snapshot.content(), "");
    }

    #[test]
    fn test_snapshot_line_access_out_of_bounds() {
        let buffer = make_test_buffer();
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        assert!(snapshot.line(100).is_none());
    }

    #[test]
    fn test_snapshot_text_in_range_boundary() {
        let buffer = super::super::Buffer::from_string("Hello");
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        // End column exceeds line length - should clamp
        let text = snapshot.text_in_range(Position::new(0, 0), Position::new(0, 100));
        assert_eq!(text, "Hello");
    }

    #[test]
    fn test_snapshot_is_valid_position() {
        let buffer = make_test_buffer();
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        assert!(snapshot.is_valid_position(Position::new(0, 0)));
        assert!(snapshot.is_valid_position(Position::new(0, 5))); // At end of "Hello"
        assert!(!snapshot.is_valid_position(Position::new(0, 100))); // Past end
        assert!(!snapshot.is_valid_position(Position::new(100, 0))); // Past last line
    }

    #[test]
    fn test_snapshot_selection_text() {
        let mut buffer = super::super::Buffer::from_string("Hello World");
        buffer
            .selection_mut()
            .start(Position::new(0, 0), super::super::SelectionMode::Character);
        buffer.set_position(Position::new(0, 5));

        let snapshot = BufferSnapshot::from_buffer(&buffer);

        assert!(snapshot.has_selection());
        assert_eq!(snapshot.selected_text(), Some("Hello".to_string()));
    }

    #[test]
    fn test_snapshot_no_selection() {
        let buffer = make_test_buffer();
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        assert!(!snapshot.has_selection());
        assert!(snapshot.selected_text().is_none());
    }

    #[test]
    fn test_snapshot_immutability() {
        let mut buffer = super::super::Buffer::from_string("Original");
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        // Modify the buffer
        buffer.set_content("Modified");

        // Snapshot should still have original content
        assert_eq!(snapshot.content(), "Original");
    }

    #[test]
    fn test_snapshot_new() {
        let snapshot = BufferSnapshot::new(
            BufferId::new(),
            vec!["Line 1".to_string(), "Line 2".to_string()],
            Cursor::origin(),
            Selection::new(),
            Some("/path/to/file".to_string()),
            true,
        );

        assert_eq!(snapshot.line_count(), 2);
        assert_eq!(snapshot.file_path, Some("/path/to/file".to_string()));
        assert!(snapshot.modified);
    }

    #[test]
    fn test_snapshot_line_len() {
        let buffer = super::super::Buffer::from_string("Hello\nWorld!");
        let snapshot = BufferSnapshot::from_buffer(&buffer);

        assert_eq!(snapshot.line_len(0), Some(5));
        assert_eq!(snapshot.line_len(1), Some(6));
        assert_eq!(snapshot.line_len(99), None);
    }
}
