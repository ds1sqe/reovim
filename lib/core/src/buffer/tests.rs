//! Tests for buffer operations

use super::*;
use crate::screen::Position;

// === Buffer Creation Tests ===

#[test]
fn test_empty_buffer() {
    let buffer = Buffer::empty(0);
    assert_eq!(buffer.id, 0);
    assert_eq!(buffer.cur, Position { x: 0, y: 0 });
    assert!(buffer.contents.is_empty());
    assert!(!buffer.selection.active);
}

// === Text Operations Tests ===

#[test]
fn test_set_content() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello\nworld");
    assert_eq!(buffer.contents.len(), 2);
    assert_eq!(buffer.contents[0].inner, "hello");
    assert_eq!(buffer.contents[1].inner, "world");
}

#[test]
fn test_set_content_empty() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("");
    assert!(buffer.contents.is_empty());
}

#[test]
fn test_content_to_string() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello\nworld");
    assert_eq!(buffer.content_to_string(), "hello\nworld");
}

#[test]
fn test_insert_char() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello");
    buffer.cur = Position { x: 5, y: 0 };
    buffer.insert_char('!');
    assert_eq!(buffer.contents[0].inner, "hello!");
    assert_eq!(buffer.cur.x, 6);
}

#[test]
fn test_insert_char_middle() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hllo");
    buffer.cur = Position { x: 1, y: 0 };
    buffer.insert_char('e');
    assert_eq!(buffer.contents[0].inner, "hello");
    assert_eq!(buffer.cur.x, 2);
}

#[test]
fn test_insert_char_empty_buffer() {
    let mut buffer = Buffer::empty(0);
    buffer.insert_char('a');
    assert_eq!(buffer.contents.len(), 1);
    assert_eq!(buffer.contents[0].inner, "a");
}

#[test]
fn test_delete_char_backward() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello");
    buffer.cur = Position { x: 5, y: 0 };
    buffer.delete_char_backward();
    assert_eq!(buffer.contents[0].inner, "hell");
    assert_eq!(buffer.cur.x, 4);
}

#[test]
fn test_delete_char_backward_at_start() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello");
    buffer.cur = Position { x: 0, y: 0 };
    buffer.delete_char_backward();
    // Should not change anything
    assert_eq!(buffer.contents[0].inner, "hello");
    assert_eq!(buffer.cur.x, 0);
}

#[test]
fn test_delete_char_forward() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello");
    buffer.cur = Position { x: 0, y: 0 };
    buffer.delete_char_forward();
    assert_eq!(buffer.contents[0].inner, "ello");
}

#[test]
fn test_delete_line() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello\nworld\ntest");
    buffer.cur = Position { x: 0, y: 1 };
    buffer.delete_line();
    assert_eq!(buffer.contents.len(), 2);
    assert_eq!(buffer.contents[0].inner, "hello");
    assert_eq!(buffer.contents[1].inner, "test");
}

#[test]
fn test_delete_last_line() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello\nworld");
    buffer.cur = Position { x: 0, y: 1 };
    buffer.delete_line();
    assert_eq!(buffer.contents.len(), 1);
    assert_eq!(buffer.cur.y, 0); // Cursor should move up
}

// === Cursor Operations Tests ===

#[test]
fn test_word_forward() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello world test");
    buffer.cur = Position { x: 0, y: 0 };
    buffer.word_forward();
    assert_eq!(buffer.cur.x, 6); // After "hello "
}

#[test]
fn test_word_forward_at_end() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello");
    buffer.cur = Position { x: 3, y: 0 };
    buffer.word_forward();
    assert_eq!(buffer.cur.x, 5); // At end of line
}

#[test]
fn test_word_backward() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello world");
    buffer.cur = Position { x: 8, y: 0 };
    buffer.word_backward();
    assert_eq!(buffer.cur.x, 6); // Start of "world"
}

#[test]
fn test_word_backward_at_start() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello world");
    buffer.cur = Position { x: 0, y: 0 };
    buffer.word_backward();
    assert_eq!(buffer.cur.x, 0); // Should stay at start
}

// === Selection Operations Tests ===

#[test]
fn test_start_selection() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello");
    buffer.cur = Position { x: 2, y: 0 };
    buffer.start_selection();
    assert!(buffer.selection.active);
    assert_eq!(buffer.selection.anchor, Position { x: 2, y: 0 });
}

#[test]
fn test_clear_selection() {
    let mut buffer = Buffer::empty(0);
    buffer.start_selection();
    assert!(buffer.selection.active);
    buffer.clear_selection();
    assert!(!buffer.selection.active);
}

#[test]
fn test_selection_bounds_forward() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello world");
    buffer.cur = Position { x: 0, y: 0 };
    buffer.start_selection();
    buffer.cur = Position { x: 5, y: 0 };
    let (start, end) = buffer.selection_bounds();
    assert_eq!(start, Position { x: 0, y: 0 });
    assert_eq!(end, Position { x: 5, y: 0 });
}

#[test]
fn test_selection_bounds_backward() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello world");
    buffer.cur = Position { x: 5, y: 0 };
    buffer.start_selection();
    buffer.cur = Position { x: 0, y: 0 };
    let (start, end) = buffer.selection_bounds();
    // Should normalize so start < end
    assert_eq!(start, Position { x: 0, y: 0 });
    assert_eq!(end, Position { x: 5, y: 0 });
}

#[test]
fn test_get_selected_text_single_line() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello world");
    buffer.cur = Position { x: 0, y: 0 };
    buffer.start_selection();
    buffer.cur = Position { x: 4, y: 0 };
    let text = buffer.get_selected_text();
    assert_eq!(text, "hello");
}

#[test]
fn test_get_selected_text_inactive() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello world");
    let text = buffer.get_selected_text();
    assert_eq!(text, "");
}

#[test]
fn test_get_selected_text_multi_line() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello\nworld\ntest");
    buffer.cur = Position { x: 2, y: 0 };
    buffer.start_selection();
    buffer.cur = Position { x: 2, y: 1 };
    let text = buffer.get_selected_text();
    assert_eq!(text, "llo\nwor");
}

#[test]
fn test_delete_selection_single_line() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello world");
    buffer.cur = Position { x: 0, y: 0 };
    buffer.start_selection();
    buffer.cur = Position { x: 4, y: 0 };
    let deleted = buffer.delete_selection();
    assert_eq!(deleted, "hello");
    assert_eq!(buffer.contents[0].inner, " world");
    assert!(!buffer.selection.active);
}

#[test]
fn test_delete_selection_multi_line() {
    let mut buffer = Buffer::empty(0);
    buffer.set_content("hello\nworld\ntest");
    buffer.cur = Position { x: 2, y: 0 };
    buffer.start_selection();
    buffer.cur = Position { x: 2, y: 1 };
    let deleted = buffer.delete_selection();
    assert_eq!(deleted, "llo\nwor");
    assert_eq!(buffer.contents.len(), 2);
    assert_eq!(buffer.contents[0].inner, "held");
    assert_eq!(buffer.contents[1].inner, "test");
}
