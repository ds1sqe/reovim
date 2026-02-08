//! Search engine implementation.
//!
//! Provides regex-based search functionality for pattern matching in buffers.

use {
    regex::Regex,
    reovim_driver_search::{Direction, SearchError, SearchMatch, SearchProvider},
    reovim_kernel::api::v1::{Buffer, Position},
};

/// Regex-based search engine implementing `SearchProvider`.
pub struct SearchEngine;

impl SearchProvider for SearchEngine {
    fn find_next(
        &self,
        buffer: &Buffer,
        cursor: Position,
        pattern: &str,
        direction: Direction,
        wrap: bool,
    ) -> Result<Option<SearchMatch>, SearchError> {
        let regex = Regex::new(pattern).map_err(|e| SearchError::InvalidPattern(e.to_string()))?;

        let content = buffer.content();
        let cursor_byte = position_to_byte(buffer, cursor);

        let result = match direction {
            Direction::Forward => find_forward(&content, cursor_byte, &regex, wrap, buffer),
            Direction::Backward => find_backward(&content, cursor_byte, &regex, wrap, buffer),
        };

        Ok(result)
    }

    fn find_all(&self, buffer: &Buffer, pattern: &str) -> Result<Vec<SearchMatch>, SearchError> {
        let regex = Regex::new(pattern).map_err(|e| SearchError::InvalidPattern(e.to_string()))?;
        let content = buffer.content();

        let matches: Vec<_> = regex
            .find_iter(&content)
            .map(|m| SearchMatch {
                start: byte_to_position(buffer, m.start()),
                end: byte_to_position(buffer, m.end()),
            })
            .collect();

        Ok(matches)
    }

    fn word_at_cursor(&self, buffer: &Buffer, cursor: Position) -> Option<String> {
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
            start: byte_to_position(buffer, start_byte),
            end: byte_to_position(buffer, end_byte),
        });
    }

    // Wrap to beginning if enabled
    if wrap
        && cursor_byte > 0
        && let Some(m) = regex.find(&content[..cursor_byte])
    {
        return Some(SearchMatch {
            start: byte_to_position(buffer, m.start()),
            end: byte_to_position(buffer, m.end()),
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
    // Search the entire content and filter by match start position.
    // Vim's backward search finds matches where match.start() < cursor.
    // This handles matches that span across the cursor position correctly.
    let all_matches: Vec<_> = regex.find_iter(content).collect();

    // Find matches that start before cursor
    let before_cursor: Vec<_> = all_matches
        .iter()
        .filter(|m| m.start() < cursor_byte)
        .collect();

    if let Some(m) = before_cursor.last() {
        return Some(SearchMatch {
            start: byte_to_position(buffer, m.start()),
            end: byte_to_position(buffer, m.end()),
        });
    }

    // Wrap to end if enabled - find matches that start at or after cursor
    if wrap {
        let at_or_after: Vec<_> = all_matches
            .iter()
            .filter(|m| m.start() >= cursor_byte)
            .collect();

        if let Some(m) = at_or_after.last() {
            return Some(SearchMatch {
                start: byte_to_position(buffer, m.start()),
                end: byte_to_position(buffer, m.end()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_buffer(content: &str) -> Buffer {
        Buffer::from_string(content)
    }

    #[test]
    fn test_find_forward_basic() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world");
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "world", Direction::Forward, false);

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        assert_eq!(m.start, Position::new(0, 6));
    }

    #[test]
    fn test_find_backward_basic() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world hello");
        let result =
            engine.find_next(&buffer, Position::new(0, 17), "hello", Direction::Backward, false);

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        // Should find the second "hello" (backward from end)
        assert_eq!(m.start.column, 12);
    }

    #[test]
    fn test_find_not_found() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world");
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "xyz", Direction::Forward, false);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_find_wrap_forward() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world hello");
        let result =
            engine.find_next(&buffer, Position::new(0, 15), "hello", Direction::Forward, true);

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        // Should wrap to beginning
        assert_eq!(m.start, Position::new(0, 0));
    }

    #[test]
    fn test_invalid_pattern() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello");
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "[invalid", Direction::Forward, false);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SearchError::InvalidPattern(_)));
    }

    #[test]
    fn test_word_at_cursor() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world test");
        let word = engine.word_at_cursor(&buffer, Position::new(0, 7));

        assert!(word.is_some());
        assert_eq!(word.unwrap(), r"\bworld\b");
    }

    #[test]
    fn test_word_at_cursor_not_on_word() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world");
        let word = engine.word_at_cursor(&buffer, Position::new(0, 5)); // On space

        assert!(word.is_none());
    }

    #[test]
    fn test_find_all() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("foo bar foo baz foo");
        let result = engine.find_all(&buffer, "foo");

        assert!(result.is_ok());
        let matches = result.unwrap();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_find_in_empty_buffer() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("");
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "hello", Direction::Forward, true);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_find_all_in_empty_buffer() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("");
        let result = engine.find_all(&buffer, "foo");

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_word_at_cursor_in_empty_buffer() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("");
        let word = engine.word_at_cursor(&buffer, Position::new(0, 0));

        assert!(word.is_none());
    }

    #[test]
    fn test_find_wrap_backward() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world hello");
        // Cursor at column 0 - no matches start before this position
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "hello", Direction::Backward, true);

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        // Should wrap to end and find the last "hello" at column 12
        assert_eq!(m.start.column, 12);
    }

    #[test]
    fn test_find_forward_no_wrap() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world");
        // Cursor after the only match, no wrap
        let result =
            engine.find_next(&buffer, Position::new(0, 10), "hello", Direction::Forward, false);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_find_backward_no_wrap() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world");
        // Cursor at beginning, no matches before it
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "world", Direction::Backward, false);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_find_all_invalid_pattern() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello");
        let result = engine.find_all(&buffer, "[invalid");

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SearchError::InvalidPattern(_)));
    }

    #[test]
    fn test_find_multiline() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("line1\nline2\nline3");
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "line2", Direction::Forward, false);

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        assert_eq!(m.start, Position::new(1, 0));
    }

    #[test]
    fn test_find_all_multiline() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("foo\nbar\nfoo\nbaz\nfoo");
        let result = engine.find_all(&buffer, "foo");

        assert!(result.is_ok());
        let matches = result.unwrap();
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].start, Position::new(0, 0));
        assert_eq!(matches[1].start, Position::new(2, 0));
        assert_eq!(matches[2].start, Position::new(4, 0));
    }

    #[test]
    fn test_find_forward_regex_pattern() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello 123 world 456");
        let result =
            engine.find_next(&buffer, Position::new(0, 0), r"\d+", Direction::Forward, false);

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        assert_eq!(m.start, Position::new(0, 6));
        assert_eq!(m.end, Position::new(0, 9));
    }

    #[test]
    fn test_find_backward_finds_closest_before_cursor() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("aaa bbb aaa bbb aaa");
        // Cursor at column 12, should find "aaa" at column 8 (closest before)
        let result =
            engine.find_next(&buffer, Position::new(0, 12), "aaa", Direction::Backward, false);

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        assert_eq!(m.start.column, 8);
    }

    #[test]
    fn test_word_at_cursor_at_start_of_word() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world");
        let word = engine.word_at_cursor(&buffer, Position::new(0, 0));

        assert!(word.is_some());
        assert_eq!(word.unwrap(), r"\bhello\b");
    }

    #[test]
    fn test_word_at_cursor_at_end_of_word() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world");
        let word = engine.word_at_cursor(&buffer, Position::new(0, 4));

        assert!(word.is_some());
        assert_eq!(word.unwrap(), r"\bhello\b");
    }

    #[test]
    fn test_word_at_cursor_with_underscore() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("foo_bar baz");
        let word = engine.word_at_cursor(&buffer, Position::new(0, 3));

        assert!(word.is_some());
        assert_eq!(word.unwrap(), r"\bfoo_bar\b");
    }

    #[test]
    fn test_word_at_cursor_column_out_of_bounds() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hi");
        let word = engine.word_at_cursor(&buffer, Position::new(0, 10));

        assert!(word.is_none());
    }

    #[test]
    fn test_word_at_cursor_on_special_char() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("a.b");
        let word = engine.word_at_cursor(&buffer, Position::new(0, 1)); // On '.'

        assert!(word.is_none());
    }

    #[test]
    fn test_position_to_byte_and_back() {
        let buffer = create_test_buffer("hello\nworld");

        // Position (0, 0) -> byte 0
        let byte = position_to_byte(&buffer, Position::new(0, 0));
        assert_eq!(byte, 0);

        // Position (1, 0) -> byte 6 (after "hello\n")
        let byte = position_to_byte(&buffer, Position::new(1, 0));
        assert_eq!(byte, 6);

        // byte 0 -> Position (0, 0)
        let pos = byte_to_position(&buffer, 0);
        assert_eq!(pos, Position::new(0, 0));

        // byte 6 -> Position (1, 0)
        let pos = byte_to_position(&buffer, 6);
        assert_eq!(pos, Position::new(1, 0));
    }

    #[test]
    fn test_byte_to_position_past_end() {
        let buffer = create_test_buffer("hi");
        let pos = byte_to_position(&buffer, 100);
        // Should return past-end position
        assert!(pos.line >= 1 || pos.column >= 2);
    }

    #[test]
    fn test_position_to_byte_clamps() {
        let buffer = create_test_buffer("hi");
        let byte = position_to_byte(&buffer, Position::new(100, 100));
        // Should be clamped to content length
        assert!(byte <= 2);
    }

    #[test]
    fn test_find_forward_at_end_of_content() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello");
        // Cursor at the very end
        let result =
            engine.find_next(&buffer, Position::new(0, 5), "hello", Direction::Forward, false);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_find_backward_wrap_finds_last_match() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("abc abc abc");
        // Cursor at 0, backward wrap should find last "abc"
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "abc", Direction::Backward, true);

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        assert_eq!(m.start.column, 8);
    }

    #[test]
    fn test_find_all_no_matches() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello world");
        let result = engine.find_all(&buffer, "xyz");

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_find_all_match_positions() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("abab");
        let result = engine.find_all(&buffer, "ab");

        assert!(result.is_ok());
        let matches = result.unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].start, Position::new(0, 0));
        assert_eq!(matches[0].end, Position::new(0, 2));
        assert_eq!(matches[1].start, Position::new(0, 2));
        assert_eq!(matches[1].end, Position::new(0, 4));
    }

    #[test]
    fn test_find_forward_skips_cursor_position() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("aaa");
        // Cursor at 0, should find match at 1 (not 0 itself)
        let result = engine.find_next(&buffer, Position::new(0, 0), "a", Direction::Forward, false);

        assert!(result.is_ok());
        let m = result.unwrap().unwrap();
        assert_eq!(m.start, Position::new(0, 1));
    }

    #[test]
    fn test_word_at_cursor_with_regex_special_chars() {
        let engine = SearchEngine;
        // The word itself won't have special chars (alphanumeric + underscore only)
        // but this tests that regex::escape works if needed
        let buffer = create_test_buffer("hello");
        let word = engine.word_at_cursor(&buffer, Position::new(0, 0));
        assert!(word.is_some());
        assert_eq!(word.unwrap(), r"\bhello\b");
    }

    #[test]
    fn test_word_at_cursor_line_out_of_bounds() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("hello");
        // Line 5 does not exist - buffer.line() returns None
        let word = engine.word_at_cursor(&buffer, Position::new(5, 0));
        assert!(word.is_none());
    }

    #[test]
    fn test_find_forward_wrap_with_cursor_at_zero() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("abc");
        // Cursor at 0, search for "abc" forward with wrap
        // Match starts at 0 but cursor_byte is 0 so search_start is 1
        // No match after cursor, wrap tries content[..0] which is empty
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "abc", Direction::Forward, true);
        assert!(result.is_ok());
        // Should find nothing because cursor_byte is 0, wrap searches [..0] which is empty
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_find_backward_no_wrap_no_match_before() {
        let engine = SearchEngine;
        let buffer = create_test_buffer("xyz abc");
        // Cursor at start - nothing before cursor
        let result =
            engine.find_next(&buffer, Position::new(0, 0), "abc", Direction::Backward, false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_byte_to_position_at_exact_line_boundary() {
        let buffer = create_test_buffer("ab\ncd");
        // byte 2 is the newline, byte 3 is 'c' on line 1
        let pos = byte_to_position(&buffer, 3);
        assert_eq!(pos, Position::new(1, 0));
    }
}
