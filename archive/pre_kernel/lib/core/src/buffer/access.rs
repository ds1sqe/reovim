//! Buffer access traits for plugins
//!
//! This module provides read-only and write access traits for buffers,
//! allowing plugins to interact with buffer content safely.

#![allow(clippy::missing_const_for_fn)]

use crate::screen::Position;

use super::{Buffer, Selection};

/// A snapshot of buffer content for read-only access
///
/// This is a lightweight copy of essential buffer information
/// that plugins can use without holding locks on the actual buffer.
#[derive(Debug, Clone)]
pub struct BufferSnapshot {
    /// Buffer ID
    pub id: usize,
    /// Lines content
    pub lines: Vec<String>,
    /// Current cursor position
    pub cursor: Position,
    /// Desired column for vertical movement
    pub desired_col: Option<u16>,
    /// Current selection
    pub selection: Selection,
    /// File path if any
    pub file_path: Option<String>,
    /// Whether buffer has unsaved modifications
    pub modified: bool,
    /// Total byte count
    pub byte_count: usize,
}

impl BufferSnapshot {
    /// Create a snapshot from a buffer
    #[must_use]
    pub fn from_buffer(buffer: &Buffer) -> Self {
        let lines: Vec<String> = buffer.contents.iter().map(|l| l.inner.clone()).collect();
        let byte_count = lines.iter().map(|l| l.len() + 1).sum::<usize>();

        Self {
            id: buffer.id,
            lines,
            cursor: buffer.cur,
            desired_col: buffer.desired_col,
            selection: buffer.selection,
            file_path: buffer.file_path.clone(),
            modified: buffer.modified,
            byte_count,
        }
    }

    /// Get the number of lines
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a specific line
    #[must_use]
    pub fn line(&self, idx: usize) -> Option<&str> {
        self.lines.get(idx).map(String::as_str)
    }

    /// Get the full content as a single string
    #[must_use]
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// Get text in a range (line, col) to (line, col)
    #[must_use]
    pub fn text_in_range(&self, start: Position, end: Position) -> String {
        if start.y == end.y {
            // Single line
            if let Some(line) = self.lines.get(start.y as usize) {
                let start_col = (start.x as usize).min(line.len());
                let end_col = (end.x as usize).min(line.len());
                return line[start_col..end_col].to_string();
            }
        } else {
            // Multiple lines
            let mut result = String::new();
            for y in start.y..=end.y {
                if let Some(line) = self.lines.get(y as usize) {
                    if y == start.y {
                        let col = (start.x as usize).min(line.len());
                        result.push_str(&line[col..]);
                        result.push('\n');
                    } else if y == end.y {
                        let col = (end.x as usize).min(line.len());
                        result.push_str(&line[..col]);
                    } else {
                        result.push_str(line);
                        result.push('\n');
                    }
                }
            }
            return result;
        }
        String::new()
    }

    /// Get the word at a position
    #[must_use]
    pub fn word_at(&self, pos: Position) -> Option<(String, Position, Position)> {
        let line = self.lines.get(pos.y as usize)?;
        let col = (pos.x as usize).min(line.len());

        // Find word boundaries
        let chars: Vec<char> = line.chars().collect();
        if col >= chars.len() {
            return None;
        }

        // Check if cursor is on a word character
        let c = chars[col];
        if !c.is_alphanumeric() && c != '_' {
            return None;
        }

        // Find start of word
        let mut start = col;
        while start > 0 {
            let prev = chars[start - 1];
            if !prev.is_alphanumeric() && prev != '_' {
                break;
            }
            start -= 1;
        }

        // Find end of word
        let mut end = col;
        while end < chars.len() {
            let curr = chars[end];
            if !curr.is_alphanumeric() && curr != '_' {
                break;
            }
            end += 1;
        }

        let word: String = chars[start..end].iter().collect();
        #[allow(clippy::cast_possible_truncation)]
        let start_pos = Position {
            x: start as u16,
            y: pos.y,
        };
        #[allow(clippy::cast_possible_truncation)]
        let end_pos = Position {
            x: end as u16,
            y: pos.y,
        };

        Some((word, start_pos, end_pos))
    }
}

/// Trait for read-only buffer access
///
/// This trait allows plugins to read buffer content without
/// modifying it.
pub trait BufferRead {
    /// Get a snapshot of the buffer
    fn snapshot(&self, buffer_id: usize) -> Option<BufferSnapshot>;

    /// Get the active buffer ID
    fn active_id(&self) -> usize;

    /// Get a list of all buffer IDs
    fn buffer_ids(&self) -> Vec<usize>;

    /// Get the content of a buffer as a string
    fn content(&self, buffer_id: usize) -> Option<String> {
        self.snapshot(buffer_id).map(|s| s.content())
    }

    /// Get the line count of a buffer
    fn line_count(&self, buffer_id: usize) -> Option<usize> {
        self.snapshot(buffer_id).map(|s| s.line_count())
    }

    /// Get a specific line from a buffer
    fn line(&self, buffer_id: usize, line_idx: usize) -> Option<String> {
        self.snapshot(buffer_id)
            .and_then(|s| s.line(line_idx).map(String::from))
    }

    /// Get the cursor position of a buffer
    fn cursor(&self, buffer_id: usize) -> Option<Position> {
        self.snapshot(buffer_id).map(|s| s.cursor)
    }

    /// Get the file path of a buffer
    fn file_path(&self, buffer_id: usize) -> Option<String> {
        self.snapshot(buffer_id).and_then(|s| s.file_path)
    }

    /// Check if a buffer is modified
    fn is_modified(&self, buffer_id: usize) -> Option<bool> {
        self.snapshot(buffer_id).map(|s| s.modified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_snapshot() -> BufferSnapshot {
        BufferSnapshot {
            id: 0,
            lines: vec![
                "Hello World".to_string(),
                "This is a test".to_string(),
                "Third line".to_string(),
            ],
            cursor: Position { x: 0, y: 0 },
            desired_col: None,
            selection: Selection::default(),
            file_path: Some("/test/file.rs".to_string()),
            modified: false,
            byte_count: 38,
        }
    }

    #[test]
    fn test_snapshot_line_count() {
        let snapshot = create_test_snapshot();
        assert_eq!(snapshot.line_count(), 3);
    }

    #[test]
    fn test_snapshot_line_access() {
        let snapshot = create_test_snapshot();
        assert_eq!(snapshot.line(0), Some("Hello World"));
        assert_eq!(snapshot.line(1), Some("This is a test"));
        assert_eq!(snapshot.line(3), None);
    }

    #[test]
    fn test_snapshot_content() {
        let snapshot = create_test_snapshot();
        let content = snapshot.content();
        assert_eq!(content, "Hello World\nThis is a test\nThird line");
    }

    #[test]
    fn test_snapshot_text_in_range_single_line() {
        let snapshot = create_test_snapshot();
        let text = snapshot.text_in_range(Position { x: 0, y: 0 }, Position { x: 5, y: 0 });
        assert_eq!(text, "Hello");
    }

    #[test]
    fn test_snapshot_text_in_range_multiple_lines() {
        let snapshot = create_test_snapshot();
        let text = snapshot.text_in_range(Position { x: 6, y: 0 }, Position { x: 4, y: 1 });
        assert_eq!(text, "World\nThis");
    }

    #[test]
    fn test_word_at() {
        let snapshot = create_test_snapshot();

        // Test word at beginning
        let result = snapshot.word_at(Position { x: 0, y: 0 });
        assert!(result.is_some());
        let (word, start, end) = result.unwrap();
        assert_eq!(word, "Hello");
        assert_eq!(start.x, 0);
        assert_eq!(end.x, 5);

        // Test word in middle
        let result = snapshot.word_at(Position { x: 7, y: 0 });
        assert!(result.is_some());
        let (word, _, _) = result.unwrap();
        assert_eq!(word, "World");

        // Test on space (no word)
        let result = snapshot.word_at(Position { x: 5, y: 0 });
        assert!(result.is_none());
    }
}
