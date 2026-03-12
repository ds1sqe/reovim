use super::*;

// === Construction ===

#[test]
fn new_buffer_is_empty() {
    let buf = Buffer::new();
    assert_eq!(buf.line_count(), 0);
    assert!(buf.is_empty());
    assert!(!buf.is_modified());
    assert!(buf.file_path().is_none());
}

#[test]
fn with_id() {
    let id = BufferId::from_raw(42);
    let buf = Buffer::with_id(id);
    assert_eq!(buf.id(), id);
    assert_eq!(buf.line_count(), 0);
    assert!(buf.is_empty());
    assert!(!buf.is_modified());
}

#[test]
fn from_string_multi_line() {
    let buf = Buffer::from_string("Hello\nWorld\nTest");
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.line(0), Some("Hello"));
    assert_eq!(buf.line(1), Some("World"));
    assert_eq!(buf.line(2), Some("Test"));
}

#[test]
fn from_string_not_modified() {
    let buf = Buffer::from_string("content");
    assert!(!buf.is_modified());
}

#[test]
fn default_is_new() {
    let buf = Buffer::default();
    assert_eq!(buf.line_count(), 0);
    assert!(buf.is_empty());
}

// === Accessors ===

#[test]
fn modified_flag() {
    let mut buf = Buffer::from_string("test");
    assert!(!buf.is_modified());

    buf.set_modified(true);
    assert!(buf.is_modified());

    buf.set_modified(false);
    assert!(!buf.is_modified());
}

#[test]
fn file_path_none_by_default() {
    let buf = Buffer::new();
    assert!(buf.file_path().is_none());
}

#[test]
fn set_and_get_file_path() {
    let mut buf = Buffer::new();
    buf.set_file_path(Some("/tmp/test.txt".to_string()));
    assert_eq!(buf.file_path(), Some("/tmp/test.txt"));

    buf.set_file_path(None);
    assert!(buf.file_path().is_none());
}

// === Line Access ===

#[test]
fn line_out_of_bounds() {
    let buf = Buffer::from_string("Hello");
    assert!(buf.line(1).is_none());
    assert!(buf.line(100).is_none());
}

#[test]
fn line_len_valid() {
    let buf = Buffer::from_string("Hello\nWorld!");
    assert_eq!(buf.line_len(0), Some(5));
    assert_eq!(buf.line_len(1), Some(6));
}

#[test]
fn line_len_out_of_bounds() {
    let buf = Buffer::from_string("Hello");
    assert!(buf.line_len(1).is_none());
}

#[test]
fn line_len_unicode() {
    // "H\u{00e9}llo" = H + e-acute + l + l + o = 5 chars
    let buf = Buffer::from_string("H\u{00e9}llo");
    assert_eq!(buf.line_len(0), Some(5));
}

#[test]
fn lines_accessor() {
    let buf = Buffer::from_string("A\nB\nC");
    let lines = buf.lines();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "A");
    assert_eq!(lines[1], "B");
    assert_eq!(lines[2], "C");
}

#[test]
fn content_empty() {
    let buf = Buffer::new();
    assert_eq!(buf.content(), "");
}

#[test]
fn content_single_line() {
    let buf = Buffer::from_string("Hello");
    assert_eq!(buf.content(), "Hello");
}

// === set_content ===

#[test]
fn set_content_replaces_all() {
    let mut buf = Buffer::from_string("old");
    buf.set_content("new\ncontent");
    assert_eq!(buf.line_count(), 2);
    assert_eq!(buf.line(0), Some("new"));
    assert_eq!(buf.line(1), Some("content"));
    assert!(buf.is_modified());
}

#[test]
fn set_content_empty_clears_lines() {
    let mut buf = Buffer::from_string("Hello\nWorld");
    buf.set_content("");
    assert_eq!(buf.line_count(), 0);
    assert!(buf.is_empty());
    assert!(buf.is_modified());
}

// === Line Hashing ===

#[test]
fn line_hash_valid_line() {
    let buf = Buffer::from_string("Hello\nWorld");
    let hash0 = buf.line_hash(0);
    let hash1 = buf.line_hash(1);
    assert!(hash0.is_some());
    assert!(hash1.is_some());
    // Different content should produce different hashes (probabilistically)
    assert_ne!(hash0, hash1);
}

#[test]
fn line_hash_out_of_bounds() {
    let buf = Buffer::from_string("Hello");
    assert!(buf.line_hash(1).is_none());
}

#[test]
fn line_hash_consistent() {
    let buf = Buffer::from_string("Hello");
    let hash1 = buf.line_hash(0);
    let hash2 = buf.line_hash(0);
    assert_eq!(hash1, hash2);
}

#[test]
fn line_hashes_returns_all() {
    let buf = Buffer::from_string("A\nB\nC");
    let hashes = buf.line_hashes();
    assert_eq!(hashes.len(), 3);
}

#[test]
fn line_hashes_empty_buffer() {
    let buf = Buffer::new();
    let hashes = buf.line_hashes();
    assert!(hashes.is_empty());
}

// === insert_at ===

#[test]
fn insert_at_empty_text_noop() {
    let mut buf = Buffer::from_string("Hello");
    buf.insert_at(Position::new(0, 0), "");
    assert_eq!(buf.line(0), Some("Hello"));
    assert!(!buf.is_modified());
}

#[test]
fn insert_at_empty_buffer() {
    let mut buf = Buffer::new();
    buf.insert_at(Position::origin(), "Hello");
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.line(0), Some("Hello"));
    assert!(buf.is_modified());
}

#[test]
fn insert_at_end() {
    let mut buf = Buffer::from_string("Hello");
    buf.insert_at(Position::new(0, 5), " World");
    assert_eq!(buf.line(0), Some("Hello World"));
}

#[test]
fn insert_at_clamped_position() {
    let mut buf = Buffer::from_string("Hello");
    // Column past end of line should clamp
    buf.insert_at(Position::new(0, 100), "!");
    assert_eq!(buf.line(0), Some("Hello!"));
}

#[test]
fn insert_at_clamped_line() {
    let mut buf = Buffer::from_string("Hello");
    // Line past end of buffer clamps to last line; col 0 inserts at start
    buf.insert_at(Position::new(100, 0), "!");
    assert_eq!(buf.line(0), Some("!Hello"));
}

#[test]
fn insert_marks_modified() {
    let mut buf = Buffer::from_string("test");
    buf.insert_at(Position::new(0, 0), "x");
    assert!(buf.is_modified());
}

#[test]
fn insert_unicode() {
    let mut buf = Buffer::from_string("H\u{00e9}llo");
    buf.insert_at(Position::new(0, 5), " World");
    assert_eq!(buf.line(0), Some("H\u{00e9}llo World"));
}

// === delete_at ===

#[test]
fn delete_at_zero_count() {
    let mut buf = Buffer::from_string("Hello");
    let deleted = buf.delete_at(Position::new(0, 0), 0);
    assert_eq!(deleted, "");
    assert_eq!(buf.line(0), Some("Hello"));
    assert!(!buf.is_modified());
}

#[test]
fn delete_at_empty_buffer() {
    let mut buf = Buffer::new();
    let deleted = buf.delete_at(Position::origin(), 5);
    assert_eq!(deleted, "");
}

#[test]
fn delete_at_single_char() {
    let mut buf = Buffer::from_string("Hello");
    let deleted = buf.delete_at(Position::new(0, 0), 1);
    assert_eq!(deleted, "H");
    assert_eq!(buf.line(0), Some("ello"));
}

#[test]
fn delete_at_multiple_chars() {
    let mut buf = Buffer::from_string("Hello World");
    let deleted = buf.delete_at(Position::new(0, 0), 6);
    assert_eq!(deleted, "Hello ");
    assert_eq!(buf.line(0), Some("World"));
}

#[test]
fn delete_at_newline_merges_lines() {
    let mut buf = Buffer::from_string("Hello\nWorld");
    let deleted = buf.delete_at(Position::new(0, 5), 1);
    assert_eq!(deleted, "\n");
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.line(0), Some("HelloWorld"));
}

#[test]
fn delete_at_across_lines() {
    let mut buf = Buffer::from_string("Hello\nWorld");
    let deleted = buf.delete_at(Position::new(0, 3), 5);
    assert_eq!(deleted, "lo\nWo");
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.line(0), Some("Helrld"));
}

#[test]
fn delete_at_end_of_line_no_next() {
    let mut buf = Buffer::from_string("Hello");
    // At end of last line, nothing more to delete
    let deleted = buf.delete_at(Position::new(0, 5), 1);
    assert_eq!(deleted, "");
}

#[test]
fn delete_at_marks_modified() {
    let mut buf = Buffer::from_string("test");
    buf.delete_at(Position::new(0, 0), 1);
    assert!(buf.is_modified());
}

#[test]
fn delete_at_nothing_deleted_not_modified() {
    let mut buf = Buffer::from_string("test");
    let deleted = buf.delete_at(Position::new(0, 4), 0);
    assert_eq!(deleted, "");
    assert!(!buf.is_modified());
}

// === delete_range ===

// === position_to_byte ===

#[test]
fn position_to_byte_start() {
    let buf = Buffer::from_string("Hello\nWorld");
    assert_eq!(buf.position_to_byte(Position::new(0, 0)), 0);
}

#[test]
fn position_to_byte_mid_line() {
    let buf = Buffer::from_string("Hello\nWorld");
    assert_eq!(buf.position_to_byte(Position::new(0, 5)), 5);
}

#[test]
fn position_to_byte_second_line() {
    let buf = Buffer::from_string("Hello\nWorld");
    assert_eq!(buf.position_to_byte(Position::new(1, 0)), 6);
    assert_eq!(buf.position_to_byte(Position::new(1, 5)), 11);
}

#[test]
fn position_to_byte_empty_buffer() {
    let buf = Buffer::new();
    assert_eq!(buf.position_to_byte(Position::new(0, 0)), 0);
}

#[test]
fn position_to_byte_unicode() {
    // "H\u{00e9}llo" -> H(1 byte) + e-acute(2 bytes) + l(1) + l(1) + o(1) = 6 bytes
    let buf = Buffer::from_string("H\u{00e9}llo");
    assert_eq!(buf.position_to_byte(Position::new(0, 0)), 0); // H
    assert_eq!(buf.position_to_byte(Position::new(0, 1)), 1); // start of e-acute
    assert_eq!(buf.position_to_byte(Position::new(0, 2)), 3); // after 2-byte char
}

// === byte_to_position ===

#[test]
fn byte_to_position_start() {
    let buf = Buffer::from_string("Hello\nWorld");
    assert_eq!(buf.byte_to_position(0), Position::new(0, 0));
}

#[test]
fn byte_to_position_mid_line() {
    let buf = Buffer::from_string("Hello\nWorld");
    assert_eq!(buf.byte_to_position(3), Position::new(0, 3));
}

#[test]
fn byte_to_position_at_newline() {
    let buf = Buffer::from_string("Hello\nWorld");
    // byte 5 is at end of "Hello" (at the newline)
    assert_eq!(buf.byte_to_position(5), Position::new(0, 5));
}

#[test]
fn byte_to_position_second_line() {
    let buf = Buffer::from_string("Hello\nWorld");
    assert_eq!(buf.byte_to_position(6), Position::new(1, 0));
    assert_eq!(buf.byte_to_position(11), Position::new(1, 5));
}

#[test]
fn byte_to_position_past_end() {
    let buf = Buffer::from_string("Hello\nWorld");
    // Past end should clamp to last position
    let result = buf.byte_to_position(100);
    assert_eq!(result, Position::new(1, 5));
}

#[test]
fn byte_to_position_empty_buffer() {
    let buf = Buffer::new();
    assert_eq!(buf.byte_to_position(0), Position::new(0, 0));
}

#[test]
fn byte_position_roundtrip() {
    let buf = Buffer::from_string("Hello\nWorld\nTest");
    for line in 0..buf.line_count() {
        for col in 0..=buf.line_len(line).unwrap() {
            let pos = Position::new(line, col);
            let byte = buf.position_to_byte(pos);
            let back = buf.byte_to_position(byte);
            assert_eq!(pos, back, "Roundtrip failed for {pos:?} (byte={byte})");
        }
    }
}

#[test]
fn byte_position_roundtrip_unicode() {
    let buf = Buffer::from_string("H\u{00e9}llo\nWorld");
    for line in 0..buf.line_count() {
        for col in 0..=buf.line_len(line).unwrap() {
            let pos = Position::new(line, col);
            let byte = buf.position_to_byte(pos);
            let back = buf.byte_to_position(byte);
            assert_eq!(pos, back, "Unicode roundtrip failed for {pos:?}");
        }
    }
}

// === Helper functions ===

#[test]
fn char_to_byte_offset_ascii() {
    assert_eq!(char_to_byte_offset("hello", 0), 0);
    assert_eq!(char_to_byte_offset("hello", 3), 3);
    assert_eq!(char_to_byte_offset("hello", 5), 5); // past end
}

#[test]
fn char_to_byte_offset_unicode() {
    // "H\u{00e9}llo" -> H(1) + e-acute(2) + l(1) + l(1) + o(1)
    assert_eq!(char_to_byte_offset("H\u{00e9}llo", 0), 0);
    assert_eq!(char_to_byte_offset("H\u{00e9}llo", 1), 1);
    assert_eq!(char_to_byte_offset("H\u{00e9}llo", 2), 3); // skip 2-byte char
}

#[test]
fn char_to_byte_offset_past_end() {
    assert_eq!(char_to_byte_offset("hi", 10), 2); // returns line.len()
}

#[test]
fn char_to_byte_offset_empty() {
    assert_eq!(char_to_byte_offset("", 0), 0);
}

#[test]
fn byte_to_char_offset_ascii() {
    assert_eq!(byte_to_char_offset("hello", 0), 0);
    assert_eq!(byte_to_char_offset("hello", 3), 3);
    assert_eq!(byte_to_char_offset("hello", 5), 5);
}

#[test]
fn byte_to_char_offset_unicode() {
    // "H\u{00e9}llo" -> byte 3 is char index 2
    assert_eq!(byte_to_char_offset("H\u{00e9}llo", 0), 0);
    assert_eq!(byte_to_char_offset("H\u{00e9}llo", 1), 1);
    assert_eq!(byte_to_char_offset("H\u{00e9}llo", 3), 2); // past the 2-byte char
}

#[test]
fn byte_to_char_offset_empty() {
    assert_eq!(byte_to_char_offset("", 0), 0);
}

// === Clone ===

#[test]
fn buffer_clone() {
    let mut buf = Buffer::from_string("Hello");
    buf.set_file_path(Some("/test".to_string()));
    buf.set_modified(true);

    let cloned = buf.clone();
    assert_eq!(cloned.id(), buf.id());
    assert_eq!(cloned.content(), buf.content());
    assert_eq!(cloned.file_path(), buf.file_path());
    assert_eq!(cloned.is_modified(), buf.is_modified());
}

// === Clamp position edge cases ===

#[test]
fn insert_at_clamped_empty_buffer_with_text() {
    let mut buf = Buffer::new();
    // Inserting into empty buffer at (10, 10) should clamp to (0, 0)
    buf.insert_at(Position::new(10, 10), "text");
    assert_eq!(buf.line(0), Some("text"));
}

#[test]
fn delete_at_clamped() {
    let mut buf = Buffer::from_string("Hello");
    // Delete from past-end position: should clamp
    let deleted = buf.delete_at(Position::new(0, 100), 1);
    // Clamped to (0, 5) which is end of line, nothing to delete on current line
    // but it won't crash
    assert_eq!(deleted, "");
}

// === Multiple operations ===

#[test]
fn insert_then_delete() {
    let mut buf = Buffer::from_string("Hello");
    buf.insert_at(Position::new(0, 5), " World");
    assert_eq!(buf.content(), "Hello World");

    buf.delete_at(Position::new(0, 5), 6);
    assert_eq!(buf.content(), "Hello");
}

#[test]
fn multiple_newline_inserts() {
    let mut buf = Buffer::new();
    buf.insert_at(Position::origin(), "Line1\nLine2\nLine3");
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.line(0), Some("Line1"));
    assert_eq!(buf.line(1), Some("Line2"));
    assert_eq!(buf.line(2), Some("Line3"));
}
