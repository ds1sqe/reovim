//! Read-only buffer snapshot for lock-free access.
//!
//! Moved from `reovim-kernel` as part of #740. `BufferSnapshot` captures
//! provider-owned text state and does not belong in the kernel.

use {
    reovim_domain_text::{Cursor, Position, Rope},
    reovim_kernel::api::v1::BufferId,
};

/// Read-only snapshot of buffer state.
///
/// A `BufferSnapshot` captures the complete state of a buffer at a point
/// in time. Clone is O(1) via `Arc` structural sharing.
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
    /// Create a snapshot from a rope and metadata.
    ///
    /// Cursor must be passed explicitly - get it from Window.
    /// This is O(1) — the rope is shared via `Arc`.
    #[must_use]
    pub const fn from_parts(
        id: BufferId,
        rope: Rope,
        cursor: Cursor,
        file_path: Option<String>,
        modified: bool,
    ) -> Self {
        Self {
            id,
            text: rope,
            cursor,
            file_path,
            modified,
        }
    }

    /// Create a snapshot from individual components.
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
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        (0..self.text.line_count())
            .filter_map(|i| self.text.line(i).map(String::from))
            .collect()
    }

    /// Get the full content as a string.
    #[must_use]
    pub fn content(&self) -> String {
        self.text.content()
    }

    /// Extract text within a range.
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
        let start_line = start.line.min(line_count - 1);
        let end_line = end.line.min(line_count - 1);

        if start_line == end_line {
            let line = self.text.line(start_line).unwrap_or("");
            let chars: Vec<char> = line.chars().collect();
            let start_col = start.column.min(chars.len());
            let end_col = end.column.min(chars.len());
            return chars[start_col..end_col].iter().collect();
        }

        let mut result = String::new();

        for line_idx in start_line..=end_line {
            let line = self.text.line(line_idx).unwrap_or("");
            let chars: Vec<char> = line.chars().collect();

            if line_idx == start_line {
                let start_col = start.column.min(chars.len());
                result.extend(&chars[start_col..]);
                result.push('\n');
            } else if line_idx == end_line {
                let end_col = end.column.min(chars.len());
                result.extend(&chars[..end_col]);
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }

        result
    }

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
}

#[cfg(test)]
#[path = "tests/snapshot.rs"]
mod tests;
