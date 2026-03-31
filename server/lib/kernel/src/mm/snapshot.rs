//! Read-only buffer snapshot for lock-free access.
//!
//! This module provides `BufferSnapshot`, a read-only capture of buffer state
//! that can be safely shared across threads without locking.
//!
//! # Cursor Isolation (#471)
//!
//! Cursor must be passed explicitly to `from_buffer()` since cursor is now
//! per-client state in Window, not Buffer.
//!
//! # Design Philosophy
//!
//! Following the kernel "mechanism, not policy" principle:
//! - Snapshot captures state at a point in time
//! - No modification methods - purely read-only
//! - Thread-safe by construction (Clone, Send, Sync)
//!
//! # Structural Sharing (#711)
//!
//! Snapshots store a rope clone, which is O(1) via `Arc` sharing.
//! The snapshot shares unchanged text nodes with the original buffer.
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
//! let cursor = Cursor::new(Position::new(0, 5)); // Get from Window
//!
//! // Capture state with explicit cursor
//! let snapshot = BufferSnapshot::from_buffer(&buffer, cursor);
//!
//! // Snapshot is independent of buffer changes
//! buffer.insert_at(Position::new(0, 5), "!");
//!
//! assert_eq!(snapshot.line_count(), 2);
//! assert_eq!(snapshot.content(), "Hello\nWorld"); // Original content
//! ```

use super::rope::Rope;
use super::{BufferId, Cursor, Position};

/// Read-only snapshot of buffer state.
///
/// A `BufferSnapshot` captures the complete state of a buffer at a point
/// in time. Clone is O(1) via `Arc` structural sharing.
///
/// # Cursor Isolation (#471)
///
/// Cursor is passed explicitly to `from_buffer()` - get it from Window.
///
/// # Immutability
///
/// All methods are read-only. The snapshot cannot be modified after
/// creation, ensuring thread safety without locks.
///
/// # Fields Captured
///
/// - `id`: Buffer identifier
/// - `text`: Text content (rope clone, O(1))
/// - `cursor`: Cursor state (passed from Window)
/// - `file_path`: Associated file path (if any)
/// - `modified`: Whether buffer had unsaved changes
///
/// Note: Selection removed in Phase 8 (#465) - it now lives in Window.
/// Note: Cursor removed from Buffer in #471 - it now lives in Window.
#[derive(Debug, Clone)]
pub struct BufferSnapshot {
    /// Buffer identifier.
    pub id: BufferId,
    /// Text content as a rope (O(1) clone via Arc sharing).
    text: Rope,
    /// Cursor state (from Window, not Buffer).
    pub cursor: Cursor,
    /// File path (if buffer is associated with a file).
    pub file_path: Option<String>,
    /// Whether buffer has unsaved modifications.
    pub modified: bool,
}

impl BufferSnapshot {
    /// Create a snapshot from a buffer.
    ///
    /// Cursor must be passed explicitly - get it from Window.
    /// This is O(1) — the rope is cloned via `Arc` sharing.
    #[must_use]
    pub fn from_buffer(buffer: &super::Buffer, cursor: Cursor) -> Self {
        Self {
            id: buffer.id(),
            text: buffer.clone_rope(),
            cursor,
            file_path: buffer.file_path().map(String::from),
            modified: buffer.is_modified(),
        }
    }

    /// Create a snapshot from individual components.
    ///
    /// This is useful for testing or when constructing a snapshot
    /// without a buffer.
    #[must_use]
    pub fn new(
        id: BufferId,
        lines: &[String],
        cursor: Cursor,
        file_path: Option<String>,
        modified: bool,
    ) -> Self {
        let content = lines.join("\n");
        let text = if content.is_empty() {
            Rope::new()
        } else {
            Rope::from_str(&content)
        };
        Self {
            id,
            text,
            cursor,
            file_path,
            modified,
        }
    }

    // === Line Access ===

    /// Get the number of lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.text.line_count()
    }

    /// Check if the snapshot is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Get a specific line by index.
    ///
    /// Returns `None` if index is out of bounds.
    #[must_use]
    pub fn line(&self, idx: usize) -> Option<&str> {
        self.text.line(idx)
    }

    /// Get the length of a line in characters.
    #[must_use]
    pub fn line_len(&self, idx: usize) -> Option<usize> {
        self.text.line_len(idx)
    }

    /// Collect all lines as a `Vec<String>`.
    ///
    /// This allocates — prefer `line(idx)` for individual access.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        (0..self.text.line_count())
            .filter_map(|i| self.text.line(i).map(String::from))
            .collect()
    }

    /// Get the full content as a string (lines joined with newlines).
    #[must_use]
    pub fn content(&self) -> String {
        self.text.content()
    }

    // === Text Extraction ===

    /// Extract text within a range.
    ///
    /// Returns the text between `start` and `end` positions.
    /// Positions are clamped to valid bounds.
    #[must_use]
    pub fn text_in_range(&self, start: Position, end: Position) -> String {
        if self.text.is_empty() {
            return String::new();
        }

        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        let line_count = self.text.line_count();

        // Clamp positions
        let start_line = start.line.min(line_count - 1);
        let end_line = end.line.min(line_count - 1);

        if start_line == end_line {
            // Single line extraction
            let line = self.text.line(start_line).unwrap_or("");
            let chars: Vec<char> = line.chars().collect();
            let start_col = start.column.min(chars.len());
            let end_col = end.column.min(chars.len());
            return chars[start_col..end_col].iter().collect();
        }

        // Multi-line extraction
        let mut result = String::new();

        for line_idx in start_line..=end_line {
            let line = self.text.line(line_idx).unwrap_or("");
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
        if pos.line >= self.text.line_count() {
            return false;
        }
        self.line(pos.line)
            .is_some_and(|line| pos.column <= line.chars().count())
    }

    // NOTE: Selection methods removed in Phase 8 (#465).
    // Selection now lives in Window (per-window state), not Buffer/BufferSnapshot.
    // Use SessionRuntime::selection(buffer_id) via BufferApi trait.
}
