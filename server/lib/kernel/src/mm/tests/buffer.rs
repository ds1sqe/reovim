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

// NOTE: lines_accessor test removed in #711 — Buffer::lines() removed.

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

// NOTE: char_to_byte_offset / byte_to_char_offset tests removed in #711.
// These helpers are no longer needed — the Rope handles all byte/char conversion.

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

/// Trace substitute-style delete+insert on multiline buffer.
#[test]
fn substitute_delete_insert_preserves_lines() {
    let mut buf = Buffer::from_string("aaa\nbbb\naaa");
    assert_eq!(buf.line_count(), 3);

    // Substitute line 2: delete "aaa", insert "zzz"
    let deleted = buf.delete_range(Position::new(2, 0), Position::new(2, 3));
    eprintln!("After delete line 2: content={:?} lines={}", buf.content(), buf.line_count());
    for i in 0..buf.line_count() {
        eprintln!("  line {}: {:?}", i, buf.line(i));
    }
    assert_eq!(deleted, "aaa");

    buf.insert_at(Position::new(2, 0), "zzz");
    eprintln!("After insert line 2: content={:?} lines={}", buf.content(), buf.line_count());
    for i in 0..buf.line_count() {
        eprintln!("  line {}: {:?}", i, buf.line(i));
    }
    assert_eq!(buf.line(0), Some("aaa"), "line 0 after step 1");
    assert_eq!(buf.line(1), Some("bbb"), "line 1 after step 1");
    assert_eq!(buf.line(2), Some("zzz"), "line 2 after step 1");

    // Substitute line 0: delete "aaa", insert "zzz"
    let deleted = buf.delete_range(Position::new(0, 0), Position::new(0, 3));
    eprintln!("After delete line 0: content={:?} lines={}", buf.content(), buf.line_count());
    for i in 0..buf.line_count() {
        eprintln!("  line {}: {:?}", i, buf.line(i));
    }
    assert_eq!(deleted, "aaa");

    buf.insert_at(Position::new(0, 0), "zzz");
    eprintln!("After insert line 0: content={:?} lines={}", buf.content(), buf.line_count());
    for i in 0..buf.line_count() {
        eprintln!("  line {}: {:?}", i, buf.line(i));
    }
    assert_eq!(buf.line(0), Some("zzz"), "line 0 final");
    assert_eq!(buf.line(1), Some("bbb"), "line 1 final");
    assert_eq!(buf.line(2), Some("zzz"), "line 2 final");
    assert_eq!(buf.content(), "zzz\nbbb\nzzz");
}

// === Coverage: extract_byte_range skip/break on multi-chunk rope (lines 394-395, 398) ===

#[test]
fn delete_range_on_large_buffer_exercises_chunk_iteration() {
    // Build a buffer larger than MAX_CHUNK_BYTES (1024) so the rope has multiple chunks.
    // Then delete a range that starts partway through, forcing extract_byte_range to:
    //   - skip early chunks (continue on line 394-395)
    //   - break after the end range (line 398)
    let line = "abcdefghijklmnopqrstuvwxyz"; // 26 chars
    // 50 lines * 26 chars + 49 newlines = 1349 bytes → >1024, multiple chunks
    let content: String = (0..50).map(|_| line).collect::<Vec<_>>().join("\n");
    let mut buf = Buffer::from_string(&content);

    // Delete a small range in the middle (line 25, cols 5..10)
    let deleted = buf.delete_range(Position::new(25, 5), Position::new(25, 10));
    assert_eq!(deleted, "fghij", "should extract the correct byte range across chunks");
}

// === Coverage: extract_byte_range chunk skip and break (lines 393-395, 397-398) ===

#[test]
fn extract_byte_range_skips_early_chunks_and_breaks_after() {
    // Build a multi-chunk buffer and extract a range that starts AFTER the first chunk.
    // This forces the `chunk_end <= start → continue` path (lines 393-395)
    // and the `pos >= end → break` path (lines 397-398).
    use std::fmt::Write;
    let mut text = String::new();
    // 200 lines of 30+ chars each → well over 1024 bytes → multiple chunks
    for i in 0..200 {
        writeln!(text, "line number {i:05} with padding").unwrap();
    }
    let mut buf = Buffer::from_string(&text);
    assert!(buf.content().len() > 2048, "buffer should have multiple chunks");

    // Delete from the last line — this forces extract_byte_range to skip
    // all early chunks (continue on line 394) and break after finding the
    // range (break on line 398).
    let deleted = buf.delete_range(Position::new(199, 0), Position::new(199, 4));
    assert_eq!(deleted, "line", "should extract text from the last chunk");
}

// === Coverage: delete_at resulting in empty deleted text (branch 284:1) ===

#[test]
fn delete_at_at_very_end_yields_empty_not_modified() {
    // delete_at where byte_start >= byte_end after clamping.
    // Position at end of buffer content → byte_start == byte_end.
    let mut buf = Buffer::from_string("abc");
    let deleted = buf.delete_at(Position::new(0, 3), 0);
    assert_eq!(deleted, "");
    assert!(!buf.is_modified(), "empty deletion should not set modified");
}

// === Coverage: delete_range resulting in empty deleted text (branch 314:1) ===

#[test]
fn delete_range_zero_width_not_modified() {
    // delete_range with same start and end position → byte_start == byte_end → empty string.
    let mut buf = Buffer::from_string("hello\nworld");
    let deleted = buf.delete_range(Position::new(0, 3), Position::new(0, 3));
    assert_eq!(deleted, "");
    assert!(!buf.is_modified(), "zero-width delete_range should not set modified");
}

// === Coverage: delete_range with reversed positions (branch 284:1 indirectly) ===

#[test]
fn delete_range_reversed_positions() {
    // When start > end, the function swaps them (line 297-298).
    let mut buf = Buffer::from_string("hello\nworld");
    let deleted = buf.delete_range(Position::new(0, 5), Position::new(0, 2));
    assert_eq!(deleted, "llo", "reversed range should still delete correctly");
    assert!(buf.is_modified());
}

// === Coverage: extract_byte_range on truly multi-chunk rope ===

#[test]
fn extract_byte_range_spanning_multiple_chunks() {
    // Build a buffer large enough for 3+ chunks, then extract a range
    // spanning from one chunk into another.
    use std::fmt::Write;
    let mut text = String::new();
    for i in 0..300 {
        writeln!(text, "data line {i:05} padding chars here").unwrap();
    }
    let mut buf = Buffer::from_string(&text);
    // Delete a range that spans across chunk boundaries
    // Lines 100-200 should be well past the first chunk
    let deleted = buf.delete_range(Position::new(100, 0), Position::new(200, 0));
    assert!(!deleted.is_empty(), "should extract cross-chunk content");
    // The extracted text should start with line 100
    assert!(deleted.starts_with("data line 00100"));
}

// === Coverage: clamp_position on empty buffer (L346:br0, L347) ===

#[test]
fn delete_range_on_empty_buffer() {
    // delete_range calls clamp_position without an empty-buffer guard,
    // so this hits the `self.text.is_empty()` true branch (L346:br0).
    let mut buf = Buffer::new();
    let deleted = buf.delete_range(Position::new(0, 0), Position::new(1, 5));
    assert!(deleted.is_empty());
}

// === Coverage: normalize_to_rope joined empty (L376:br0, L377) ===

#[test]
fn from_string_single_newline() {
    // "\n".lines() → [""], joined → "". joined.is_empty() → true (L376:br0).
    let buf = Buffer::from_string("\n");
    assert_eq!(buf.line_count(), 0, "single newline normalizes to empty");
}
