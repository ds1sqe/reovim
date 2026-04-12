use super::*;

use crate::{Position, SimpleText};

#[test]
fn test_find_symmetric_quotes() {
    let buffer = SimpleText::new("\"hello world\"");
    let result = find_delimiter_pair(&buffer, Position::new(0, 5), '"', '"');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 0));
    assert_eq!(close, Position::new(0, 12));
}

#[test]
fn test_find_asymmetric_parens() {
    let buffer = SimpleText::new("fn foo(bar)");
    let result = find_delimiter_pair(&buffer, Position::new(0, 8), '(', ')');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 6));
    assert_eq!(close, Position::new(0, 10));
}

#[test]
fn test_find_asymmetric_brackets() {
    let buffer = SimpleText::new("arr[idx]");
    let result = find_delimiter_pair(&buffer, Position::new(0, 5), '[', ']');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 3));
    assert_eq!(close, Position::new(0, 7));
}

#[test]
fn test_find_nested_delimiters() {
    let buffer = SimpleText::new("((inner))");
    let result = find_delimiter_pair(&buffer, Position::new(0, 4), '(', ')');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 1));
    assert_eq!(close, Position::new(0, 7));
}

#[test]
fn test_find_multiline_delimiters() {
    let buffer = SimpleText::new("{\n  content\n}");
    let result = find_delimiter_pair(&buffer, Position::new(1, 2), '{', '}');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 0));
    assert_eq!(close, Position::new(2, 0));
}

#[test]
fn test_find_no_match() {
    let buffer = SimpleText::new("no brackets here");
    let result = find_delimiter_pair(&buffer, Position::new(0, 5), '(', ')');
    assert!(result.is_none());
}

#[test]
fn test_find_at_delimiter() {
    let buffer = SimpleText::new("(hello)");
    let result = find_delimiter_pair(&buffer, Position::new(0, 0), '(', ')');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 0));
    assert_eq!(close, Position::new(0, 6));
}

#[test]
fn test_find_delimiter_empty_buffer() {
    let buffer = SimpleText::new("");
    let result = find_delimiter_pair(&buffer, Position::new(0, 0), '(', ')');
    assert!(result.is_none());
}

#[test]
fn test_find_delimiter_unbalanced() {
    let buffer = SimpleText::new("(unclosed");
    let result = find_delimiter_pair(&buffer, Position::new(0, 5), '(', ')');
    assert!(result.is_none());
}

#[test]
fn test_find_matching_delimiter_paren() {
    let buffer = SimpleText::new("(hello)");
    let result = find_matching_delimiter(&buffer, Position::new(0, 0));
    assert_eq!(result, Some(Position::new(0, 6)));
}

#[test]
fn test_find_matching_delimiter_bracket() {
    let buffer = SimpleText::new("[item]");
    let result = find_matching_delimiter(&buffer, Position::new(0, 0));
    assert_eq!(result, Some(Position::new(0, 5)));
}

#[test]
fn test_find_matching_delimiter_brace() {
    let buffer = SimpleText::new("{block}");
    let result = find_matching_delimiter(&buffer, Position::new(0, 0));
    assert_eq!(result, Some(Position::new(0, 6)));
}

#[test]
fn test_multiple_quote_pairs() {
    let buffer = SimpleText::new("\"a\" \"b\"");
    let result1 = find_delimiter_pair(&buffer, Position::new(0, 1), '"', '"');
    assert!(result1.is_some());
    let (open1, close1) = result1.unwrap();
    assert_eq!(open1, Position::new(0, 0));
    assert_eq!(close1, Position::new(0, 2));

    let result2 = find_delimiter_pair(&buffer, Position::new(0, 5), '"', '"');
    assert!(result2.is_some());
    let (open2, close2) = result2.unwrap();
    assert_eq!(open2, Position::new(0, 4));
    assert_eq!(close2, Position::new(0, 6));
}

#[test]
fn test_find_matching_delimiter_close_paren() {
    let buffer = SimpleText::new("(hello)");
    let result = find_matching_delimiter(&buffer, Position::new(0, 6));
    assert_eq!(result, Some(Position::new(0, 0)));
}

#[test]
fn test_find_matching_delimiter_close_bracket() {
    let buffer = SimpleText::new("[item]");
    let result = find_matching_delimiter(&buffer, Position::new(0, 5));
    assert_eq!(result, Some(Position::new(0, 0)));
}

#[test]
fn test_find_matching_delimiter_close_brace() {
    let buffer = SimpleText::new("{block}");
    let result = find_matching_delimiter(&buffer, Position::new(0, 6));
    assert_eq!(result, Some(Position::new(0, 0)));
}

#[test]
fn test_find_matching_delimiter_angle_brackets() {
    let buffer = SimpleText::new("<item>");
    let result = find_matching_delimiter(&buffer, Position::new(0, 0));
    assert_eq!(result, Some(Position::new(0, 5)));
    let result = find_matching_delimiter(&buffer, Position::new(0, 5));
    assert_eq!(result, Some(Position::new(0, 0)));
}

#[test]
fn test_find_asymmetric_pair_at_closing() {
    let buffer = SimpleText::new("(hello)");
    let result = find_delimiter_pair(&buffer, Position::new(0, 6), '(', ')');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 0));
    assert_eq!(close, Position::new(0, 6));
}

#[test]
fn test_symmetric_pair_outside() {
    let buffer = SimpleText::new("\"a\" hello \"b\"");
    let result = find_delimiter_pair(&buffer, Position::new(0, 4), '"', '"');
    assert!(result.is_none());
}

#[test]
fn test_find_matching_delimiter_non_delimiter() {
    let buffer = SimpleText::new("hello");
    let result = find_matching_delimiter(&buffer, Position::new(0, 2));
    assert!(result.is_none());
}

#[test]
fn test_symmetric_single_quote() {
    let buffer = SimpleText::new("no quotes");
    let result = find_delimiter_pair(&buffer, Position::new(0, 3), '"', '"');
    assert!(result.is_none());
}

#[test]
fn test_find_backward_previous_lines() {
    let buffer = SimpleText::new("(\n  content\n  )");
    let result = find_delimiter_pair(&buffer, Position::new(2, 2), '(', ')');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 0));
    assert_eq!(close, Position::new(2, 2));
}

#[test]
fn test_find_forward_subsequent_lines() {
    let buffer = SimpleText::new("(\n  content\n  more\n)");
    let result = find_delimiter_pair(&buffer, Position::new(1, 2), '(', ')');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 0));
    assert_eq!(close, Position::new(3, 0));
}

#[test]
fn test_find_asymmetric_deep_nesting() {
    let buffer = SimpleText::new("(a (b (c) d) e)");
    let result = find_delimiter_pair(&buffer, Position::new(0, 7), '(', ')');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 6));
    assert_eq!(close, Position::new(0, 8));
}

#[test]
fn test_find_asymmetric_pair_at_closing_multiline() {
    let buffer = SimpleText::new("{ content }");
    let result = find_delimiter_pair(&buffer, Position::new(0, 10), '{', '}');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 0));
    assert_eq!(close, Position::new(0, 10));
}

#[test]
fn test_find_matching_delimiter_multiline_forward() {
    let buffer = SimpleText::new("(\n  hello\n)");
    let result = find_matching_delimiter(&buffer, Position::new(0, 0));
    assert_eq!(result, Some(Position::new(2, 0)));
}

#[test]
fn test_find_matching_delimiter_multiline_backward() {
    let buffer = SimpleText::new("( hello )");
    let result = find_matching_delimiter(&buffer, Position::new(0, 8));
    assert_eq!(result, Some(Position::new(0, 0)));
}

#[test]
fn test_symmetric_odd_quotes() {
    let buffer = SimpleText::new("\"a\" b \"c");
    let result = find_delimiter_pair(&buffer, Position::new(0, 5), '"', '"');
    assert!(result.is_none());
}

#[test]
fn test_find_backward_with_nested_close() {
    let buffer = SimpleText::new("( () ) x");
    let result = find_delimiter_pair(&buffer, Position::new(0, 5), '(', ')');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 0));
    assert_eq!(close, Position::new(0, 5));
}

#[test]
fn test_find_forward_unmatched() {
    let buffer = SimpleText::new("(no close");
    let result = find_delimiter_pair(&buffer, Position::new(0, 0), '(', ')');
    assert!(result.is_none());
}

#[test]
fn test_find_matching_delimiter_angle_backward() {
    let buffer = SimpleText::new("<a>");
    let result = find_matching_delimiter(&buffer, Position::new(0, 2));
    assert_eq!(result, Some(Position::new(0, 0)));
}

#[test]
fn test_find_forward_nested_depth() {
    let buffer = SimpleText::new("( ( ) )");
    let result = find_delimiter_pair(&buffer, Position::new(0, 0), '(', ')');
    assert!(result.is_some());
    let (open_pos, close_pos) = result.unwrap();
    assert_eq!(open_pos, Position::new(0, 0));
    assert_eq!(close_pos, Position::new(0, 6));
}

#[test]
fn test_cursor_on_close_no_matching_open() {
    let buffer = SimpleText::new(") hello");
    let result = find_delimiter_pair(&buffer, Position::new(0, 0), '(', ')');
    assert!(result.is_none());
}

#[test]
fn test_find_backward_multiline_search() {
    let buffer = SimpleText::new("(\nhello\n)");
    let pos = Position::new(1, 2);
    let result = find_delimiter_pair(&buffer, pos, '(', ')');
    assert!(result.is_some());
    let (open, close) = result.unwrap();
    assert_eq!(open, Position::new(0, 0));
    assert_eq!(close, Position::new(2, 0));
}

#[test]
fn test_find_forward_multiline_no_match() {
    let buffer = SimpleText::new("( hello\nworld");
    let result = find_delimiter_pair(&buffer, Position::new(0, 0), '(', ')');
    assert!(result.is_none());
}
