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

    #[cfg_attr(coverage_nightly, coverage(off))]
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
        // Escape regex special characters and add word boundaries
        Some(format!(r"\b{}\b", regex::escape(&word)))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
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
#[path = "engine_tests.rs"]
mod tests;
