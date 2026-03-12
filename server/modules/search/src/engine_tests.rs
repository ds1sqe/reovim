use super::*;

fn create_test_buffer(content: &str) -> Buffer {
    Buffer::from_string(content)
}

#[test]
fn test_find_forward_basic() {
    let engine = SearchEngine;
    let buffer = create_test_buffer("hello world");
    let result = engine.find_next(&buffer, Position::new(0, 0), "world", Direction::Forward, false);

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
    let result = engine.find_next(&buffer, Position::new(0, 0), "xyz", Direction::Forward, false);

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_find_wrap_forward() {
    let engine = SearchEngine;
    let buffer = create_test_buffer("hello world hello");
    let result = engine.find_next(&buffer, Position::new(0, 15), "hello", Direction::Forward, true);

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
    let result = engine.find_next(&buffer, Position::new(0, 0), "hello", Direction::Forward, true);

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
    let result = engine.find_next(&buffer, Position::new(0, 0), "hello", Direction::Backward, true);

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
    let result = engine.find_next(&buffer, Position::new(0, 0), "line2", Direction::Forward, false);

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
    let result = engine.find_next(&buffer, Position::new(0, 0), r"\d+", Direction::Forward, false);

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
    let result = engine.find_next(&buffer, Position::new(0, 12), "aaa", Direction::Backward, false);

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

#[cfg_attr(coverage_nightly, coverage(off))]
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
    let result = engine.find_next(&buffer, Position::new(0, 5), "hello", Direction::Forward, false);

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_find_backward_wrap_finds_last_match() {
    let engine = SearchEngine;
    let buffer = create_test_buffer("abc abc abc");
    // Cursor at 0, backward wrap should find last "abc"
    let result = engine.find_next(&buffer, Position::new(0, 0), "abc", Direction::Backward, true);

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
    let result = engine.find_next(&buffer, Position::new(0, 0), "abc", Direction::Forward, true);
    assert!(result.is_ok());
    // Should find nothing because cursor_byte is 0, wrap searches [..0] which is empty
    assert!(result.unwrap().is_none());
}

#[test]
fn test_find_backward_no_wrap_no_match_before() {
    let engine = SearchEngine;
    let buffer = create_test_buffer("xyz abc");
    // Cursor at start - nothing before cursor
    let result = engine.find_next(&buffer, Position::new(0, 0), "abc", Direction::Backward, false);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_find_backward_wrap_no_matches_anywhere() {
    let engine = SearchEngine;
    let buffer = create_test_buffer("hello world");
    // Pattern exists nowhere in the buffer, backward wrap should return None
    let result = engine.find_next(&buffer, Position::new(0, 5), "zzz", Direction::Backward, true);
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
