//! Search engine for pattern matching in buffers.
//!
//! Provides regex-based search functionality for Vim-style / and ? commands.

use {
    regex::Regex,
    reovim_kernel::api::v1::{Buffer, Position},
};

/// Search result with match position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Start position of the match.
    pub start: Position,
    /// End position of the match.
    pub end: Position,
}

/// Search direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Search forward from cursor.
    #[default]
    Forward,
    /// Search backward from cursor.
    Backward,
}

/// Search engine for finding patterns in buffers.
pub struct SearchEngine;

impl SearchEngine {
    /// Find next match from cursor position.
    ///
    /// # Arguments
    /// * `buffer` - The buffer to search in
    /// * `cursor` - Current cursor position
    /// * `pattern` - Regex pattern to search for
    /// * `direction` - Search direction (forward or backward)
    /// * `wrap` - Whether to wrap around buffer boundaries
    ///
    /// # Returns
    /// * `Ok(Some(match))` - Match found
    /// * `Ok(None)` - No match found
    /// * `Err(e)` - Invalid pattern
    ///
    /// # Errors
    /// Returns `SearchError::InvalidPattern` if the pattern is not valid regex.
    #[must_use = "search result should be used"]
    pub fn find_next(
        buffer: &Buffer,
        cursor: Position,
        pattern: &str,
        direction: Direction,
        wrap: bool,
    ) -> Result<Option<SearchMatch>, SearchError> {
        let regex = Regex::new(pattern).map_err(|e| SearchError::InvalidPattern(e.to_string()))?;

        let content = buffer.content();
        let cursor_byte = Self::position_to_byte(buffer, cursor);

        let result = match direction {
            Direction::Forward => Self::find_forward(&content, cursor_byte, &regex, wrap, buffer),
            Direction::Backward => Self::find_backward(&content, cursor_byte, &regex, wrap, buffer),
        };

        Ok(result)
    }

    /// Find all matches in buffer (for highlighting).
    ///
    /// # Errors
    /// Returns `SearchError::InvalidPattern` if the pattern is not valid regex.
    #[must_use = "search results should be used"]
    pub fn find_all(buffer: &Buffer, pattern: &str) -> Result<Vec<SearchMatch>, SearchError> {
        let regex = Regex::new(pattern).map_err(|e| SearchError::InvalidPattern(e.to_string()))?;
        let content = buffer.content();

        let matches: Vec<_> = regex
            .find_iter(&content)
            .map(|m| SearchMatch {
                start: Self::byte_to_position(buffer, m.start()),
                end: Self::byte_to_position(buffer, m.end()),
            })
            .collect();

        Ok(matches)
    }

    /// Get word under cursor for * and # commands.
    ///
    /// Returns the word as a regex pattern with word boundaries.
    #[must_use]
    pub fn word_at_cursor(buffer: &Buffer, cursor: Position) -> Option<String> {
        let line = buffer.line(cursor.line)?;
        let chars: Vec<char> = line.chars().collect();

        if cursor.column >= chars.len() {
            return None;
        }

        // Check if cursor is on a word character
        let c = chars[cursor.column];
        if !c.is_alphanumeric() && c != '_' {
            return None;
        }

        // Find word boundaries
        let mut start = cursor.column;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }

        let mut end = cursor.column;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }

        let word: String = chars[start..end].iter().collect();
        if word.is_empty() {
            None
        } else {
            // Escape regex special characters and add word boundaries
            Some(format!(r"\b{}\b", regex::escape(&word)))
        }
    }

    fn find_forward(
        content: &str,
        cursor_byte: usize,
        regex: &Regex,
        wrap: bool,
        buffer: &Buffer,
    ) -> Option<SearchMatch> {
        let search_start = (cursor_byte + 1).min(content.len());

        // Search after cursor
        if search_start < content.len()
            && let Some(m) = regex.find(&content[search_start..])
        {
            let start_byte = search_start + m.start();
            let end_byte = search_start + m.end();
            return Some(SearchMatch {
                start: Self::byte_to_position(buffer, start_byte),
                end: Self::byte_to_position(buffer, end_byte),
            });
        }

        // Wrap to beginning if enabled
        if wrap
            && cursor_byte > 0
            && let Some(m) = regex.find(&content[..cursor_byte])
        {
            return Some(SearchMatch {
                start: Self::byte_to_position(buffer, m.start()),
                end: Self::byte_to_position(buffer, m.end()),
            });
        }

        None
    }

    fn find_backward(
        content: &str,
        cursor_byte: usize,
        regex: &Regex,
        wrap: bool,
        buffer: &Buffer,
    ) -> Option<SearchMatch> {
        let search_end = cursor_byte.min(content.len());

        // Search before cursor
        if search_end > 0 {
            let matches: Vec<_> = regex.find_iter(&content[..search_end]).collect();
            if let Some(m) = matches.last() {
                return Some(SearchMatch {
                    start: Self::byte_to_position(buffer, m.start()),
                    end: Self::byte_to_position(buffer, m.end()),
                });
            }
        }

        // Wrap to end if enabled
        if wrap && cursor_byte < content.len() {
            let matches: Vec<_> = regex.find_iter(&content[cursor_byte..]).collect();
            if let Some(m) = matches.last() {
                let start_byte = cursor_byte + m.start();
                let end_byte = cursor_byte + m.end();
                return Some(SearchMatch {
                    start: Self::byte_to_position(buffer, start_byte),
                    end: Self::byte_to_position(buffer, end_byte),
                });
            }
        }

        None
    }

    /// Convert buffer position to byte offset.
    fn position_to_byte(buffer: &Buffer, pos: Position) -> usize {
        let content = buffer.content();
        let mut byte_offset = 0;

        for (line_idx, line) in content.lines().enumerate() {
            if line_idx == pos.line {
                // Count bytes in this line up to the column
                let line_chars: Vec<char> = line.chars().collect();
                for (col_idx, c) in line_chars.iter().enumerate() {
                    if col_idx >= pos.column {
                        break;
                    }
                    byte_offset += c.len_utf8();
                }
                break;
            }
            // Add line bytes plus newline
            byte_offset += line.len() + 1;
        }

        byte_offset.min(content.len())
    }

    /// Convert byte offset to buffer position.
    fn byte_to_position(buffer: &Buffer, byte_offset: usize) -> Position {
        let content = buffer.content();
        let mut current_byte = 0;
        let mut line_num = 0;

        for line in content.lines() {
            let line_end = current_byte + line.len();

            if byte_offset <= line_end {
                // Found the line, now find column
                let offset_in_line = byte_offset.saturating_sub(current_byte);
                let mut column = 0;
                let mut byte_count = 0;

                for c in line.chars() {
                    if byte_count >= offset_in_line {
                        break;
                    }
                    byte_count += c.len_utf8();
                    column += 1;
                }

                return Position::new(line_num, column);
            }

            current_byte = line_end + 1; // +1 for newline
            line_num += 1;
        }

        // Past end of content
        Position::new(line_num, 0)
    }
}

/// Search error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// Invalid regex pattern.
    InvalidPattern(String),
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPattern(msg) => write!(f, "E486: Invalid pattern: {msg}"),
        }
    }
}

impl std::error::Error for SearchError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_buffer(content: &str) -> Buffer {
        Buffer::from_string(content)
    }

    #[test]
    fn test_find_forward_basic() {
        let buffer = create_test_buffer("hello world");
        let result = SearchEngine::find_next(
            &buffer,
            Position::new(0, 0),
            "world",
            Direction::Forward,
            false,
        );

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        assert_eq!(m.start, Position::new(0, 6));
    }

    #[test]
    fn test_find_backward_basic() {
        let buffer = create_test_buffer("hello world hello");
        let result = SearchEngine::find_next(
            &buffer,
            Position::new(0, 17),
            "hello",
            Direction::Backward,
            false,
        );

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        // Should find the first "hello" (backward from end)
        assert_eq!(m.start.column, 12); // Second "hello"
    }

    #[test]
    fn test_find_not_found() {
        let buffer = create_test_buffer("hello world");
        let result =
            SearchEngine::find_next(&buffer, Position::new(0, 0), "xyz", Direction::Forward, false);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_find_wrap_forward() {
        let buffer = create_test_buffer("hello world hello");
        let result = SearchEngine::find_next(
            &buffer,
            Position::new(0, 15),
            "hello",
            Direction::Forward,
            true,
        );

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        // Should wrap to beginning
        assert_eq!(m.start, Position::new(0, 0));
    }

    #[test]
    fn test_invalid_pattern() {
        let buffer = create_test_buffer("hello");
        let result = SearchEngine::find_next(
            &buffer,
            Position::new(0, 0),
            "[invalid",
            Direction::Forward,
            false,
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SearchError::InvalidPattern(_)));
    }

    #[test]
    fn test_word_at_cursor() {
        let buffer = create_test_buffer("hello world test");
        let word = SearchEngine::word_at_cursor(&buffer, Position::new(0, 7));

        assert!(word.is_some());
        assert_eq!(word.unwrap(), r"\bworld\b");
    }

    #[test]
    fn test_word_at_cursor_not_on_word() {
        let buffer = create_test_buffer("hello world");
        let word = SearchEngine::word_at_cursor(&buffer, Position::new(0, 5)); // On space

        assert!(word.is_none());
    }

    #[test]
    fn test_find_all() {
        let buffer = create_test_buffer("foo bar foo baz foo");
        let result = SearchEngine::find_all(&buffer, "foo");

        assert!(result.is_ok());
        let matches = result.unwrap();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_search_error_display() {
        let err = SearchError::InvalidPattern("unclosed bracket".into());
        assert!(err.to_string().contains("E486"));
        assert!(err.to_string().contains("unclosed bracket"));
    }

    #[test]
    fn test_find_in_empty_buffer() {
        let buffer = create_test_buffer("");
        let result = SearchEngine::find_next(
            &buffer,
            Position::new(0, 0),
            "hello",
            Direction::Forward,
            true,
        );

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_find_all_in_empty_buffer() {
        let buffer = create_test_buffer("");
        let result = SearchEngine::find_all(&buffer, "foo");

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_word_at_cursor_in_empty_buffer() {
        let buffer = create_test_buffer("");
        let word = SearchEngine::word_at_cursor(&buffer, Position::new(0, 0));

        assert!(word.is_none());
    }

    #[test]
    fn test_find_wrap_backward() {
        let buffer = create_test_buffer("hello world hello");
        let result = SearchEngine::find_next(
            &buffer,
            Position::new(0, 2), // After first "hello"
            "hello",
            Direction::Backward,
            true,
        );

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        // Should wrap to end and find the last "hello" at column 12
        assert_eq!(m.start.column, 12);
    }
}
