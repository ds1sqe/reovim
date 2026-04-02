//! Search engine implementation.
//!
//! Provides regex-based search functionality for pattern matching in buffers.

use {
    regex::Regex,
    reovim_driver_search::{Direction, LineSource, SearchError, SearchMatch, SearchProvider},
    reovim_kernel::api::v1::Buffer,
    reovim_types_text::Position,
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
        let cursor_byte = buffer.position_to_byte(cursor);

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
                start: buffer.byte_to_position(m.start()),
                end: buffer.byte_to_position(m.end()),
            })
            .collect();

        Ok(matches)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn word_at_cursor(&self, buffer: &Buffer, cursor: Position) -> Option<String> {
        let line = buffer.line(cursor.line)?;
        extract_word_pattern(line, cursor.column)
    }

    fn find_next_source(
        &self,
        source: &dyn LineSource,
        cursor: Position,
        pattern: &str,
        direction: Direction,
        wrap: bool,
    ) -> Result<Option<SearchMatch>, SearchError> {
        let regex = Regex::new(pattern).map_err(|e| SearchError::InvalidPattern(e.to_string()))?;
        let content = source.content();
        let cursor_byte = source.position_to_byte(cursor);

        let result = match direction {
            Direction::Forward => find_forward_source(&content, cursor_byte, &regex, wrap, source),
            Direction::Backward => {
                find_backward_source(&content, cursor_byte, &regex, wrap, source)
            }
        };

        Ok(result)
    }

    fn find_all_source(
        &self,
        source: &dyn LineSource,
        pattern: &str,
    ) -> Result<Vec<SearchMatch>, SearchError> {
        let regex = Regex::new(pattern).map_err(|e| SearchError::InvalidPattern(e.to_string()))?;
        let content = source.content();

        let matches: Vec<_> = regex
            .find_iter(&content)
            .map(|m| SearchMatch {
                start: source.byte_to_position(m.start()),
                end: source.byte_to_position(m.end()),
            })
            .collect();

        Ok(matches)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn word_at_cursor_source(&self, source: &dyn LineSource, cursor: Position) -> Option<String> {
        let line = source.line(cursor.line)?;
        extract_word_pattern(&line, cursor.column)
    }
}

/// Extract a word pattern from a line at a given column.
fn extract_word_pattern(line: &str, column: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();

    if column >= chars.len() {
        return None;
    }

    let c = chars[column];
    if !c.is_alphanumeric() && c != '_' {
        return None;
    }

    let mut start = column;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }

    let mut end = column;
    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
        end += 1;
    }

    let word: String = chars[start..end].iter().collect();
    Some(format!(r"\b{}\b", regex::escape(&word)))
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
            start: buffer.byte_to_position(start_byte),
            end: buffer.byte_to_position(end_byte),
        });
    }

    // Wrap to beginning if enabled
    if wrap
        && cursor_byte > 0
        && let Some(m) = regex.find(&content[..cursor_byte])
    {
        return Some(SearchMatch {
            start: buffer.byte_to_position(m.start()),
            end: buffer.byte_to_position(m.end()),
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
            start: buffer.byte_to_position(m.start()),
            end: buffer.byte_to_position(m.end()),
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
                start: buffer.byte_to_position(m.start()),
                end: buffer.byte_to_position(m.end()),
            });
        }
    }

    None
}

// ── LineSource-based search helpers ─────────────────────────────────────

#[cfg_attr(coverage_nightly, coverage(off))]
fn find_forward_source(
    content: &str,
    cursor_byte: usize,
    regex: &Regex,
    wrap: bool,
    source: &dyn LineSource,
) -> Option<SearchMatch> {
    let search_start = (cursor_byte + 1).min(content.len());

    if search_start < content.len()
        && let Some(m) = regex.find(&content[search_start..])
    {
        let start_byte = search_start + m.start();
        let end_byte = search_start + m.end();
        return Some(SearchMatch {
            start: source.byte_to_position(start_byte),
            end: source.byte_to_position(end_byte),
        });
    }

    if wrap
        && cursor_byte > 0
        && let Some(m) = regex.find(&content[..cursor_byte])
    {
        return Some(SearchMatch {
            start: source.byte_to_position(m.start()),
            end: source.byte_to_position(m.end()),
        });
    }

    None
}

fn find_backward_source(
    content: &str,
    cursor_byte: usize,
    regex: &Regex,
    wrap: bool,
    source: &dyn LineSource,
) -> Option<SearchMatch> {
    let all_matches: Vec<_> = regex.find_iter(content).collect();

    let before_cursor: Vec<_> = all_matches
        .iter()
        .filter(|m| m.start() < cursor_byte)
        .collect();

    if let Some(m) = before_cursor.last() {
        return Some(SearchMatch {
            start: source.byte_to_position(m.start()),
            end: source.byte_to_position(m.end()),
        });
    }

    if wrap {
        let at_or_after: Vec<_> = all_matches
            .iter()
            .filter(|m| m.start() >= cursor_byte)
            .collect();

        if let Some(m) = at_or_after.last() {
            return Some(SearchMatch {
                start: source.byte_to_position(m.start()),
                end: source.byte_to_position(m.end()),
            });
        }
    }

    None
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
