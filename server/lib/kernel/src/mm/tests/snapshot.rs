use super::*;

fn make_test_buffer() -> Buffer {
    // Buffer no longer has cursor state - cursor is per-window (#471)
    Buffer::from_string("Hello\nWorld\nTest")
}

#[test]
fn test_snapshot_from_buffer() {
    let buffer = make_test_buffer();
    // Pass cursor explicitly - cursor at (1, 2) for test
    let cursor = Cursor::new(Position::new(1, 2));
    let snapshot = BufferSnapshot::from_buffer(&buffer, cursor);

    assert_eq!(snapshot.id, buffer.id());
    assert_eq!(snapshot.line_count(), 3);
    assert_eq!(snapshot.position(), Position::new(1, 2));
}

#[test]
fn test_snapshot_line_count() {
    let buffer = make_test_buffer();
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());
    assert_eq!(snapshot.line_count(), 3);
}

#[test]
fn test_snapshot_line_access() {
    let buffer = make_test_buffer();
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    assert_eq!(snapshot.line(0), Some("Hello"));
    assert_eq!(snapshot.line(1), Some("World"));
    assert_eq!(snapshot.line(2), Some("Test"));
}

#[test]
fn test_snapshot_content() {
    let buffer = make_test_buffer();
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());
    assert_eq!(snapshot.content(), "Hello\nWorld\nTest");
}

#[test]
fn test_snapshot_text_in_range_single_line() {
    let buffer = Buffer::from_string("Hello World");
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(text, "Hello");
}

#[test]
fn test_snapshot_text_in_range_multi_line() {
    let buffer = make_test_buffer();
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 3), Position::new(1, 3));
    assert_eq!(text, "lo\nWor");
}

#[test]
fn test_snapshot_empty_buffer() {
    let buffer = Buffer::new();
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    assert!(snapshot.is_empty());
    assert_eq!(snapshot.line_count(), 0);
    assert_eq!(snapshot.content(), "");
}

#[test]
fn test_snapshot_line_access_out_of_bounds() {
    let buffer = make_test_buffer();
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    assert!(snapshot.line(100).is_none());
}

#[test]
fn test_snapshot_text_in_range_boundary() {
    let buffer = Buffer::from_string("Hello");
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    // End column exceeds line length - should clamp
    let text = snapshot.text_in_range(Position::new(0, 0), Position::new(0, 100));
    assert_eq!(text, "Hello");
}

#[test]
fn test_snapshot_is_valid_position() {
    let buffer = make_test_buffer();
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    assert!(snapshot.is_valid_position(Position::new(0, 0)));
    assert!(snapshot.is_valid_position(Position::new(0, 5))); // At end of "Hello"
    assert!(!snapshot.is_valid_position(Position::new(0, 100))); // Past end
    assert!(!snapshot.is_valid_position(Position::new(100, 0))); // Past last line
}

// NOTE: Selection tests removed in Phase 8 (#465).
// Selection now lives in Window, not Buffer/BufferSnapshot.

#[test]
fn test_snapshot_immutability() {
    let mut buffer = Buffer::from_string("Original");
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    // Modify the buffer
    buffer.set_content("Modified");

    // Snapshot should still have original content
    assert_eq!(snapshot.content(), "Original");
}

#[test]
fn test_snapshot_new() {
    let snapshot = BufferSnapshot::new(
        BufferId::new(),
        &["Line 1".to_string(), "Line 2".to_string()],
        Cursor::origin(),
        Some("/path/to/file".to_string()),
        true,
    );

    assert_eq!(snapshot.line_count(), 2);
    assert_eq!(snapshot.file_path, Some("/path/to/file".to_string()));
    assert!(snapshot.modified);
}

#[test]
fn test_snapshot_line_len() {
    let buffer = Buffer::from_string("Hello\nWorld!");
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    assert_eq!(snapshot.line_len(0), Some(5));
    assert_eq!(snapshot.line_len(1), Some(6));
    assert_eq!(snapshot.line_len(99), None);
}

// === Coverage: lines() accessor ===

#[test]
fn test_snapshot_lines() {
    let buffer = Buffer::from_string("Hello\nWorld");
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    let lines = snapshot.lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "Hello");
    assert_eq!(lines[1], "World");
}

// === Coverage: content() ===

#[test]
fn test_snapshot_content_two_lines() {
    let buffer = Buffer::from_string("Hello\nWorld");
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    assert_eq!(snapshot.content(), "Hello\nWorld");
}

// === Coverage: text_in_range reversed positions ===

#[test]
fn test_snapshot_text_in_range_reversed() {
    let buffer = Buffer::from_string("Hello World");
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    // Reversed: end before start should still work
    let text = snapshot.text_in_range(Position::new(0, 5), Position::new(0, 0));
    assert_eq!(text, "Hello");
}

// === Coverage: text_in_range empty buffer ===

#[test]
fn test_snapshot_text_in_range_empty() {
    let buffer = Buffer::new();
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(text, "");
}

// === Coverage: text_in_range multiline with middle lines ===

#[test]
fn test_snapshot_text_in_range_multiline_three() {
    let buffer = Buffer::from_string("aaa\nbbb\nccc");
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 1), Position::new(2, 2));
    // First line: "aa", middle line: "bbb", last line: "cc"
    assert_eq!(text, "aa\nbbb\ncc");
}

// === Coverage: single line extraction with empty result ===

#[test]
fn test_snapshot_text_in_range_same_pos() {
    let buffer = Buffer::from_string("Hello");
    let snapshot = BufferSnapshot::from_buffer(&buffer, Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 2), Position::new(0, 2));
    assert_eq!(text, "");
}

// === Coverage: line 117 — Rope::new() branch when lines is empty ===

#[test]
fn test_snapshot_new_empty_lines() {
    let snapshot = BufferSnapshot::new(BufferId::new(), &[], Cursor::origin(), None, false);

    assert!(snapshot.is_empty());
    assert_eq!(snapshot.line_count(), 0);
    assert_eq!(snapshot.content(), "");
    assert!(!snapshot.modified);
    assert!(snapshot.file_path.is_none());
}
