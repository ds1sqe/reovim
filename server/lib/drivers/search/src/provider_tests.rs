use super::*;

/// Mock search provider for testing the trait contract.
struct MockSearch;

impl SearchProvider for MockSearch {
    fn find_next(
        &self,
        buffer: &Buffer,
        _cursor: Position,
        pattern: &str,
        _direction: Direction,
        _wrap: bool,
    ) -> Result<Option<SearchMatch>, SearchError> {
        if pattern == "[invalid" {
            return Err(SearchError::InvalidPattern("unclosed bracket".into()));
        }
        let content = buffer.content();
        Ok(content.find(pattern).map(|pos| SearchMatch {
            start: Position::new(0, pos),
            end: Position::new(0, pos + pattern.len()),
        }))
    }

    fn find_all(&self, buffer: &Buffer, pattern: &str) -> Result<Vec<SearchMatch>, SearchError> {
        if pattern == "[invalid" {
            return Err(SearchError::InvalidPattern("unclosed bracket".into()));
        }
        let content = buffer.content();
        let mut matches = Vec::new();
        let mut start = 0;
        while let Some(pos) = content[start..].find(pattern) {
            let abs_pos = start + pos;
            matches.push(SearchMatch {
                start: Position::new(0, abs_pos),
                end: Position::new(0, abs_pos + pattern.len()),
            });
            start = abs_pos + pattern.len();
        }
        Ok(matches)
    }

    fn word_at_cursor(&self, buffer: &Buffer, cursor: Position) -> Option<String> {
        let content = buffer.content();
        let col = cursor.column;
        if col >= content.len() {
            return None;
        }
        // Simple word extraction: find word boundaries around cursor
        let bytes = content.as_bytes();
        if !bytes[col].is_ascii_alphanumeric() {
            return None;
        }
        let start = (0..=col)
            .rev()
            .take_while(|&i| bytes[i].is_ascii_alphanumeric())
            .last()
            .unwrap_or(col);
        let end = (col..content.len())
            .take_while(|&i| bytes[i].is_ascii_alphanumeric())
            .last()
            .map_or(col, |i| i + 1);
        Some(content[start..end].to_string())
    }
}

#[test]
fn test_search_provider_find_next_found() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("hello world");
    let cursor = Position::new(0, 0);
    let result = provider
        .find_next(&buffer, cursor, "world", Direction::Forward, true)
        .unwrap();
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.start, Position::new(0, 6));
    assert_eq!(m.end, Position::new(0, 11));
}

#[test]
fn test_search_provider_find_next_not_found() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("hello world");
    let cursor = Position::new(0, 0);
    let result = provider
        .find_next(&buffer, cursor, "missing", Direction::Forward, true)
        .unwrap();
    assert!(result.is_none());
}

#[test]
fn test_search_provider_find_next_invalid_pattern() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("hello");
    let cursor = Position::new(0, 0);
    let result = provider.find_next(&buffer, cursor, "[invalid", Direction::Forward, true);
    assert!(result.is_err());
}

#[test]
fn test_search_provider_find_all() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("abcabc");
    let result = provider.find_all(&buffer, "abc").unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn test_search_provider_find_all_no_matches() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("hello");
    let result = provider.find_all(&buffer, "xyz").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_search_provider_word_at_cursor() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("hello world");
    let word = provider.word_at_cursor(&buffer, Position::new(0, 0));
    assert_eq!(word, Some("hello".to_string()));
}

#[test]
fn test_search_provider_word_at_cursor_non_word() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("hello world");
    let word = provider.word_at_cursor(&buffer, Position::new(0, 5));
    assert!(word.is_none()); // space is not alphanumeric
}

#[test]
fn test_search_provider_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MockSearch>();
}

#[test]
fn test_search_provider_word_at_cursor_past_end() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("hello");
    // col >= content.len() should return None
    let word = provider.word_at_cursor(&buffer, Position::new(0, 100));
    assert!(word.is_none());
}

#[test]
fn test_search_provider_word_at_cursor_exactly_at_end() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("hello");
    // col == content.len() (5) should return None
    let word = provider.word_at_cursor(&buffer, Position::new(0, 5));
    assert!(word.is_none());
}

#[test]
fn test_search_provider_find_all_invalid_pattern() {
    let provider = MockSearch;
    let buffer = Buffer::from_string("hello");
    let result = provider.find_all(&buffer, "[invalid");
    assert!(result.is_err());
}
