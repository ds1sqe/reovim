use crate::{Buffer, BufferSnapshot};

use {
    reovim_domain_text::{Cursor, Position, Rope},
    reovim_kernel::api::v1::BufferId,
};

fn make_snapshot(content: &str, cursor: Cursor) -> BufferSnapshot {
    Buffer::from_string(content).snapshot(cursor)
}

fn make_test_snapshot() -> BufferSnapshot {
    make_snapshot("Hello\nWorld\nTest", Cursor::origin())
}

#[test]
fn test_snapshot_from_buffer() {
    let buffer = Buffer::from_string("Hello\nWorld\nTest");
    let cursor = Cursor::new(Position::new(1, 2));
    let snapshot = buffer.snapshot(cursor);

    assert_eq!(snapshot.id, buffer.id());
    assert_eq!(snapshot.line_count(), 3);
    assert_eq!(snapshot.position(), Position::new(1, 2));
}

#[test]
fn test_snapshot_line_count() {
    let snapshot = make_test_snapshot();
    assert_eq!(snapshot.line_count(), 3);
}

#[test]
fn test_snapshot_line_access() {
    let snapshot = make_test_snapshot();

    assert_eq!(snapshot.line(0), Some("Hello"));
    assert_eq!(snapshot.line(1), Some("World"));
    assert_eq!(snapshot.line(2), Some("Test"));
}

#[test]
fn test_snapshot_content() {
    let snapshot = make_test_snapshot();
    assert_eq!(snapshot.content(), "Hello\nWorld\nTest");
}

#[test]
fn test_snapshot_text_in_range_single_line() {
    let snapshot = make_snapshot("Hello World", Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(text, "Hello");
}

#[test]
fn test_snapshot_text_in_range_multi_line() {
    let snapshot = make_test_snapshot();

    let text = snapshot.text_in_range(Position::new(0, 3), Position::new(1, 3));
    assert_eq!(text, "lo\nWor");
}

#[test]
fn test_snapshot_empty_buffer() {
    let snapshot = Buffer::new().snapshot(Cursor::origin());

    assert!(snapshot.is_empty());
    assert_eq!(snapshot.line_count(), 0);
    assert_eq!(snapshot.content(), "");
}

#[test]
fn test_snapshot_line_access_out_of_bounds() {
    let snapshot = make_test_snapshot();

    assert!(snapshot.line(100).is_none());
}

#[test]
fn test_snapshot_text_in_range_boundary() {
    let snapshot = make_snapshot("Hello", Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 0), Position::new(0, 100));
    assert_eq!(text, "Hello");
}

#[test]
fn test_snapshot_is_valid_position() {
    let snapshot = make_test_snapshot();

    assert!(snapshot.is_valid_position(Position::new(0, 0)));
    assert!(snapshot.is_valid_position(Position::new(0, 5)));
    assert!(!snapshot.is_valid_position(Position::new(0, 100)));
    assert!(!snapshot.is_valid_position(Position::new(100, 0)));
}

#[test]
fn test_snapshot_immutability() {
    let mut buffer = Buffer::from_string("Original");
    let snapshot = buffer.snapshot(Cursor::origin());

    buffer.set_content("Modified");

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
    let snapshot = make_snapshot("Hello\nWorld!", Cursor::origin());

    assert_eq!(snapshot.line_len(0), Some(5));
    assert_eq!(snapshot.line_len(1), Some(6));
    assert_eq!(snapshot.line_len(99), None);
}

#[test]
fn test_snapshot_lines() {
    let snapshot = make_snapshot("Hello\nWorld", Cursor::origin());

    let lines = snapshot.lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "Hello");
    assert_eq!(lines[1], "World");
}

#[test]
fn test_snapshot_content_two_lines() {
    let snapshot = make_snapshot("Hello\nWorld", Cursor::origin());

    assert_eq!(snapshot.content(), "Hello\nWorld");
}

#[test]
fn test_snapshot_text_in_range_reversed() {
    let snapshot = make_snapshot("Hello World", Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 5), Position::new(0, 0));
    assert_eq!(text, "Hello");
}

#[test]
fn test_snapshot_text_in_range_empty() {
    let snapshot = Buffer::new().snapshot(Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(text, "");
}

#[test]
fn test_snapshot_text_in_range_multiline_three() {
    let snapshot = make_snapshot("aaa\nbbb\nccc", Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 1), Position::new(2, 2));
    assert_eq!(text, "aa\nbbb\ncc");
}

#[test]
fn test_snapshot_text_in_range_same_pos() {
    let snapshot = make_snapshot("Hello", Cursor::origin());

    let text = snapshot.text_in_range(Position::new(0, 2), Position::new(0, 2));
    assert_eq!(text, "");
}

#[test]
fn test_snapshot_new_empty_lines() {
    let snapshot = BufferSnapshot::new(BufferId::new(), &[], Cursor::origin(), None, false);

    assert!(snapshot.is_empty());
    assert_eq!(snapshot.line_count(), 0);
    assert_eq!(snapshot.content(), "");
    assert!(!snapshot.modified);
    assert!(snapshot.file_path.is_none());
}

#[test]
fn test_snapshot_from_parts() {
    let id = BufferId::new();
    let rope = Rope::from_str("hello\nworld");
    let cursor = Cursor::new(Position::new(0, 3));
    let snap = BufferSnapshot::from_parts(id, rope, cursor, Some("/tmp/f".into()), true);

    assert_eq!(snap.id, id);
    assert_eq!(snap.line_count(), 2);
    assert_eq!(snap.position(), Position::new(0, 3));
    assert_eq!(snap.file_path, Some("/tmp/f".to_string()));
    assert!(snap.modified);
}
