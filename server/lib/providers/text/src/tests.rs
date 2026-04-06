//! Tests for `VirtualBuffer`, `HeapMapping`, and trait integration.
//!
//! Moved from `reovim-kernel` as part of #740 (kernel buffer extraction).

use std::{borrow::Cow, sync::Arc};

use {
    reovim_driver_vfs::FileMapping,
    reovim_kernel::api::v1::BufferId,
    reovim_types_text::{LineIndex, Position, TextGeometry},
};

use crate::Buffer;

use super::{BufferOps, HeapMapping, StorageOps, VirtualBuffer, VirtualSnapshot};

/// Helper: create a `VirtualBuffer` from a string.
fn vbuf_from_str(s: &str) -> VirtualBuffer {
    let mapping = Arc::new(HeapMapping(s.as_bytes().to_vec()));
    let line_index = LineIndex::from_bytes(s.as_bytes()).unwrap();
    VirtualBuffer::new(mapping, line_index)
}

// ── HeapMapping ──────────────────────────────────────────────────────

#[test]
fn heap_mapping_as_bytes() {
    let data = b"hello world";
    let mapping = HeapMapping(data.to_vec());
    assert_eq!(mapping.as_bytes(), data);
}

#[test]
fn heap_mapping_len() {
    let mapping = HeapMapping(b"hello".to_vec());
    assert_eq!(mapping.len(), 5);
}

#[test]
fn heap_mapping_is_empty() {
    let empty = HeapMapping(Vec::new());
    assert!(empty.is_empty());

    let non_empty = HeapMapping(b"x".to_vec());
    assert!(!non_empty.is_empty());
}

#[test]
fn heap_mapping_arc_clone() {
    let mapping: Arc<dyn FileMapping> = Arc::new(HeapMapping(b"test".to_vec()));
    assert_eq!(Arc::strong_count(&mapping), 1);

    let clone = Arc::clone(&mapping);
    assert_eq!(Arc::strong_count(&mapping), 2);
    assert_eq!(clone.as_bytes(), b"test");
}

#[test]
fn heap_mapping_is_stale() {
    let mapping = HeapMapping(Vec::new());
    assert!(!mapping.is_stale());
}

// ── Construction ──────────────────────────────────────────────────────

#[test]
fn construction_empty() {
    let vbuf = vbuf_from_str("");
    assert_eq!(vbuf.line_count(), 0);
    assert!(vbuf.is_empty());
    assert!(!vbuf.is_modified());
    assert_eq!(vbuf.file_size(), 0);
    assert_eq!(vbuf.content(), "");
}

#[test]
fn construction_single_line() {
    let vbuf = vbuf_from_str("hello");
    assert_eq!(vbuf.line_count(), 1);
    assert!(!vbuf.is_empty());
    assert_eq!(vbuf.line(0), Some("hello".to_string()));
    assert_eq!(vbuf.content(), "hello");
}

#[test]
fn construction_multi_line() {
    let vbuf = vbuf_from_str("hello\nworld");
    assert_eq!(vbuf.line_count(), 2);
    assert_eq!(vbuf.line(0), Some("hello".to_string()));
    assert_eq!(vbuf.line(1), Some("world".to_string()));
    assert_eq!(vbuf.content(), "hello\nworld");
}

#[test]
fn construction_trailing_newline() {
    let vbuf = vbuf_from_str("hello\n");
    assert_eq!(vbuf.line_count(), 2);
    assert_eq!(vbuf.line(0), Some("hello".to_string()));
    assert_eq!(vbuf.line(1), Some(String::new()));
}

#[test]
fn construction_with_id() {
    let id = BufferId::from_raw(42);
    let mapping = Arc::new(HeapMapping(b"test".to_vec()));
    let line_index = LineIndex::from_bytes(b"test").unwrap();
    let vbuf = VirtualBuffer::with_id(id, mapping, line_index);
    assert_eq!(vbuf.id(), id);
}

// ── Line Access ──────────────────────────────────────────────────────

#[test]
fn line_out_of_bounds() {
    let vbuf = vbuf_from_str("hello");
    assert_eq!(vbuf.line(1), None);
    assert_eq!(vbuf.line_len(1), None);
}

#[test]
fn line_len_unicode() {
    let vbuf = vbuf_from_str("héllo");
    assert_eq!(vbuf.line_len(0), Some(5)); // 5 chars, 6 bytes
}

// ── Content Materialization ──────────────────────────────────────────

#[test]
fn content_matches_original() {
    let text = "line1\nline2\nline3";
    let vbuf = vbuf_from_str(text);
    assert_eq!(vbuf.content(), text);
}

// ── CRLF ──────────────────────────────────────────────────────────────

#[test]
fn crlf_detection_and_stripping() {
    let vbuf = vbuf_from_str("hello\r\nworld");
    assert!(vbuf.has_crlf());
    assert_eq!(vbuf.line(0), Some("hello".to_string())); // \r stripped
    assert_eq!(vbuf.line(1), Some("world".to_string()));
    // content() normalizes to LF
    assert_eq!(vbuf.content(), "hello\nworld");
}

#[test]
fn no_crlf() {
    let vbuf = vbuf_from_str("hello\nworld");
    assert!(!vbuf.has_crlf());
}

// ── File Path ────────────────────────────────────────────────────────

#[test]
fn file_path() {
    let mut vbuf = vbuf_from_str("test");
    assert_eq!(vbuf.file_path(), None);

    vbuf.set_file_path(Some("/tmp/test.txt".to_string()));
    assert_eq!(vbuf.file_path(), Some("/tmp/test.txt"));

    vbuf.set_file_path(None);
    assert_eq!(vbuf.file_path(), None);
}

// ── Modified Flag ────────────────────────────────────────────────────

#[test]
fn modified_flag() {
    let mut vbuf = vbuf_from_str("hello");
    assert!(!vbuf.is_modified());

    vbuf.insert_at(Position::new(0, 5), "!");
    assert!(vbuf.is_modified());

    vbuf.set_modified(false);
    assert!(!vbuf.is_modified());
}

// ── Insert ────────────────────────────────────────────────────────────

#[test]
fn insert_at_end() {
    let mut vbuf = vbuf_from_str("hello");
    vbuf.insert_at(Position::new(0, 5), " world");
    assert_eq!(vbuf.content(), "hello world");
    assert_eq!(vbuf.line_count(), 1);
}

#[test]
fn insert_at_start() {
    let mut vbuf = vbuf_from_str("world");
    vbuf.insert_at(Position::new(0, 0), "hello ");
    assert_eq!(vbuf.content(), "hello world");
}

#[test]
fn insert_newline() {
    let mut vbuf = vbuf_from_str("helloworld");
    vbuf.insert_at(Position::new(0, 5), "\n");
    assert_eq!(vbuf.line_count(), 2);
    assert_eq!(vbuf.line(0), Some("hello".to_string()));
    assert_eq!(vbuf.line(1), Some("world".to_string()));
}

#[test]
fn insert_into_empty() {
    let mut vbuf = vbuf_from_str("");
    vbuf.insert_at(Position::origin(), "hello");
    assert_eq!(vbuf.content(), "hello");
    assert_eq!(vbuf.line_count(), 1);
}

#[test]
fn insert_empty_string() {
    let mut vbuf = vbuf_from_str("hello");
    vbuf.insert_at(Position::new(0, 2), "");
    assert_eq!(vbuf.content(), "hello");
}

#[test]
fn insert_multiline() {
    let mut vbuf = vbuf_from_str("AC");
    vbuf.insert_at(Position::new(0, 1), "X\nY\nZ");
    assert_eq!(vbuf.line_count(), 3);
    assert_eq!(vbuf.line(0), Some("AX".to_string()));
    assert_eq!(vbuf.line(1), Some("Y".to_string()));
    assert_eq!(vbuf.line(2), Some("ZC".to_string()));
}

// ── Delete ────────────────────────────────────────────────────────────

#[test]
fn delete_single_char() {
    let mut vbuf = vbuf_from_str("Hello");
    let deleted = vbuf.delete_at(Position::new(0, 0), 1);
    assert_eq!(deleted, "H");
    assert_eq!(vbuf.content(), "ello");
}

#[test]
fn delete_multiple_chars() {
    let mut vbuf = vbuf_from_str("Hello World");
    vbuf.delete_at(Position::new(0, 0), 6);
    assert_eq!(vbuf.content(), "World");
}

#[test]
fn delete_newline() {
    let mut vbuf = vbuf_from_str("Hello\nWorld");
    vbuf.delete_at(Position::new(0, 5), 1);
    assert_eq!(vbuf.line_count(), 1);
    assert_eq!(vbuf.content(), "HelloWorld");
}

#[test]
fn delete_across_lines() {
    let mut vbuf = vbuf_from_str("Hello\nWorld");
    vbuf.delete_at(Position::new(0, 3), 5); // "lo\nWo"
    assert_eq!(vbuf.line_count(), 1);
    assert_eq!(vbuf.content(), "Helrld");
}

#[test]
fn delete_nothing() {
    let mut vbuf = vbuf_from_str("Hello");
    let deleted = vbuf.delete_at(Position::new(0, 2), 0);
    assert!(deleted.is_empty());
    assert_eq!(vbuf.content(), "Hello");
}

#[test]
fn delete_past_end() {
    let mut vbuf = vbuf_from_str("Hi");
    let deleted = vbuf.delete_at(Position::new(0, 0), 100);
    assert_eq!(deleted, "Hi");
    assert!(vbuf.is_empty());
}

#[test]
fn delete_range_basic() {
    let mut vbuf = vbuf_from_str("Hello World");
    let deleted = vbuf.delete_range(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(deleted, "Hello");
    assert_eq!(vbuf.content(), " World");
}

#[test]
fn delete_range_reversed() {
    let mut vbuf = vbuf_from_str("Hello World");
    let deleted = vbuf.delete_range(Position::new(0, 5), Position::new(0, 0));
    assert_eq!(deleted, "Hello");
}

#[test]
fn delete_range_multiline() {
    let mut vbuf = vbuf_from_str("Hello\nBeautiful\nWorld");
    let deleted = vbuf.delete_range(Position::new(0, 3), Position::new(2, 2));
    assert_eq!(deleted, "lo\nBeautiful\nWo");
    assert_eq!(vbuf.line_count(), 1);
    assert_eq!(vbuf.content(), "Helrld");
}

#[test]
fn delete_range_same_position() {
    let mut vbuf = vbuf_from_str("Hello");
    let deleted = vbuf.delete_range(Position::new(0, 2), Position::new(0, 2));
    assert!(deleted.is_empty());
    assert_eq!(vbuf.content(), "Hello");
}

#[test]
fn delete_from_empty() {
    let mut vbuf = vbuf_from_str("");
    let deleted = vbuf.delete_at(Position::new(0, 0), 5);
    assert!(deleted.is_empty());
}

// ── Position Conversion ──────────────────────────────────────────────

#[test]
fn position_to_byte_simple() {
    let vbuf = vbuf_from_str("Hello\nWorld");
    assert_eq!(vbuf.position_to_byte(Position::new(0, 0)), 0);
    assert_eq!(vbuf.position_to_byte(Position::new(0, 5)), 5);
    assert_eq!(vbuf.position_to_byte(Position::new(1, 0)), 6);
    assert_eq!(vbuf.position_to_byte(Position::new(1, 5)), 11);
}

#[test]
fn byte_to_position_simple() {
    let vbuf = vbuf_from_str("Hello\nWorld");
    assert_eq!(vbuf.byte_to_position(0), Position::new(0, 0));
    assert_eq!(vbuf.byte_to_position(5), Position::new(0, 5));
    assert_eq!(vbuf.byte_to_position(6), Position::new(1, 0));
    assert_eq!(vbuf.byte_to_position(11), Position::new(1, 5));
}

#[test]
fn position_round_trip() {
    let vbuf = vbuf_from_str("Hello\nWorld\nTest");
    for line in 0..vbuf.line_count() {
        for col in 0..=vbuf.line_len(line).unwrap() {
            let pos = Position::new(line, col);
            let byte = vbuf.position_to_byte(pos);
            let back = vbuf.byte_to_position(byte);
            assert_eq!(pos, back, "Round-trip failed for {pos:?}");
        }
    }
}

#[test]
fn position_empty() {
    let vbuf = vbuf_from_str("");
    assert_eq!(vbuf.position_to_byte(Position::new(0, 0)), 0);
    assert_eq!(vbuf.byte_to_position(0), Position::origin());
}

// ── Edit Sequence ────────────────────────────────────────────────────

#[test]
fn edit_sequence_insert_delete_insert() {
    let mut vbuf = vbuf_from_str("hello");
    vbuf.insert_at(Position::new(0, 5), " world");
    assert_eq!(vbuf.content(), "hello world");

    vbuf.delete_at(Position::new(0, 5), 1); // delete space
    assert_eq!(vbuf.content(), "helloworld");

    vbuf.insert_at(Position::new(0, 5), "-");
    assert_eq!(vbuf.content(), "hello-world");
}

// ── set_content ──────────────────────────────────────────────────────

#[test]
fn set_content() {
    let mut vbuf = vbuf_from_str("original");
    vbuf.set_content("new\ncontent");
    assert_eq!(vbuf.content(), "new\ncontent");
    assert_eq!(vbuf.line_count(), 2);
    assert!(vbuf.is_modified());
}

#[test]
fn set_content_empty() {
    let mut vbuf = vbuf_from_str("hello");
    vbuf.set_content("");
    assert!(vbuf.is_empty());
    assert_eq!(vbuf.line_count(), 0);
}

// ── Line Hash ────────────────────────────────────────────────────────

#[test]
fn line_hash_consistent() {
    let vbuf = vbuf_from_str("hello\nworld");
    let hash1 = vbuf.line_hash(0).unwrap();
    let hash2 = vbuf.line_hash(0).unwrap();
    assert_eq!(hash1, hash2);
}

#[test]
fn line_hash_out_of_bounds() {
    let vbuf = vbuf_from_str("hello");
    assert_eq!(vbuf.line_hash(1), None);
}

#[test]
fn line_hashes() {
    let vbuf = vbuf_from_str("a\nb\nc");
    let hashes = vbuf.line_hashes();
    assert_eq!(hashes.len(), 3);
}

// ── Equivalence with Buffer ──────────────────────────────────────────

#[test]
fn equivalence_line_count() {
    for content in &["", "hello", "hello\nworld", "a\nb\nc"] {
        let buf = Buffer::from_string(content);
        let vbuf = vbuf_from_str(content);
        assert_eq!(buf.line_count(), vbuf.line_count(), "line_count mismatch for {content:?}");
    }
}

#[test]
fn equivalence_line_access() {
    let content = "hello\nworld\nfoo";
    let buf = Buffer::from_string(content);
    let vbuf = vbuf_from_str(content);

    for i in 0..buf.line_count() {
        assert_eq!(buf.line(i).map(String::from), vbuf.line(i), "line({i}) mismatch");
    }
}

#[test]
fn equivalence_content() {
    let content = "hello\nworld";
    let buf = Buffer::from_string(content);
    let vbuf = vbuf_from_str(content);
    assert_eq!(buf.content(), vbuf.content());
}

#[test]
fn equivalence_is_empty() {
    let buf = Buffer::from_string("");
    let vbuf = vbuf_from_str("");
    assert_eq!(buf.is_empty(), vbuf.is_empty());

    let buf = Buffer::from_string("x");
    let vbuf = vbuf_from_str("x");
    assert_eq!(buf.is_empty(), vbuf.is_empty());
}

// ── Clone ────────────────────────────────────────────────────────────

#[test]
fn clone_shares_original() {
    let mapping: Arc<dyn FileMapping> = Arc::new(HeapMapping(b"hello".to_vec()));
    let strong_before = Arc::strong_count(&mapping);

    let line_index = LineIndex::from_bytes(b"hello").unwrap();
    let vbuf1 = VirtualBuffer::new(Arc::clone(&mapping), line_index);
    let vbuf2 = vbuf1.clone();

    // Original mapping should have 3 strong refs: mapping + vbuf1 + vbuf2
    assert_eq!(Arc::strong_count(&mapping), 3);
    assert!(strong_before < Arc::strong_count(&mapping));
    assert_eq!(vbuf1.content(), vbuf2.content());
}

// ── Snapshot ──────────────────────────────────────────────────────────

#[test]
fn snapshot_round_trip() {
    let mut vbuf = vbuf_from_str("hello\nworld");
    let snap = vbuf.capture_snapshot();

    vbuf.insert_at(Position::new(0, 5), " there");
    assert_eq!(vbuf.content(), "hello there\nworld");

    vbuf.restore_snapshot(snap);
    assert_eq!(vbuf.content(), "hello\nworld");
}

#[test]
fn snapshot_of_unedited() {
    let vbuf = vbuf_from_str("hello");
    let snap = vbuf.capture_snapshot();
    // Should be cheap — just Arc bumps
    assert_eq!(snap.add_buffer_len(), 0);
}

#[test]
fn snapshot_captures_after_edit() {
    let mut vbuf = vbuf_from_str("hello");
    vbuf.insert_at(Position::new(0, 5), " world");

    let snap = vbuf.capture_snapshot();
    assert!(snap.add_buffer_len() > 0);

    // More edits
    vbuf.insert_at(Position::new(0, 11), "!");
    assert_eq!(vbuf.content(), "hello world!");

    // Restore
    vbuf.restore_snapshot(snap);
    assert_eq!(vbuf.content(), "hello world");
}

#[test]
fn snapshot_add_buffer_monotonic() {
    let mut vbuf = vbuf_from_str("hello");
    let snap1 = vbuf.capture_snapshot();

    vbuf.insert_at(Position::new(0, 5), " world");
    let snap2 = vbuf.capture_snapshot();

    // add_buffer only grows
    assert!(snap2.add_buffer_len() >= snap1.add_buffer_len());

    // After restore, add_buffer is truncated but still >= snap1
    vbuf.restore_snapshot(snap1);
    assert!(vbuf.capture_snapshot().add_buffer_len() <= snap2.add_buffer_len());
}

// ── Debug formatting ─────────────────────────────────────────────────

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn debug_formatting() {
    let vbuf = vbuf_from_str("hello");
    let debug = format!("{vbuf:?}");
    assert!(debug.contains("VirtualBuffer"));
    assert!(debug.contains("piece_count"));
    assert!(debug.contains("file_size"));

    let snap = vbuf.capture_snapshot();
    let snap_debug = format!("{snap:?}");
    assert!(snap_debug.contains("VirtualSnapshot"));
}

// ── piece_count accessor ─────────────────────────────────────────────

#[test]
fn piece_count_grows_with_edits() {
    let vbuf = vbuf_from_str("hello");
    let initial = vbuf.piece_count();

    let mut vbuf = vbuf;
    vbuf.insert_at(Position::new(0, 2), "X");
    assert!(vbuf.piece_count() > initial);
}

// ── Unicode handling ─────────────────────────────────────────────────

#[test]
fn unicode_content() {
    let text = "héllo\nwörld\n日本語";
    let vbuf = vbuf_from_str(text);
    assert_eq!(vbuf.content(), text);
    assert_eq!(vbuf.line_count(), 3);
    assert_eq!(vbuf.line(2), Some("日本語".to_string()));
}

#[test]
fn unicode_insert() {
    let mut vbuf = vbuf_from_str("héllo");
    vbuf.insert_at(Position::new(0, 5), " wörld");
    assert_eq!(vbuf.content(), "héllo wörld");
}

#[test]
fn unicode_position_round_trip() {
    let vbuf = vbuf_from_str("héllo\nwörld");
    for line in 0..vbuf.line_count() {
        for col in 0..=vbuf.line_len(line).unwrap() {
            let pos = Position::new(line, col);
            let byte = vbuf.position_to_byte(pos);
            let back = vbuf.byte_to_position(byte);
            assert_eq!(pos, back, "Unicode round-trip failed for {pos:?}");
        }
    }
}

// ── VirtualSnapshot inherent methods ─────────────────────────────────

#[test]
fn virtual_capture_and_restore() {
    let mut vbuf = vbuf_from_str("Hello\nWorld");

    let snap = vbuf.capture_snapshot();

    // Modify buffer
    vbuf.set_content("Something else");
    assert_eq!(vbuf.content(), "Something else");

    // Restore via inherent method
    vbuf.restore_snapshot(snap);
    assert_eq!(vbuf.content(), "Hello\nWorld");
}

#[test]
fn virtual_snapshot_after_edits() {
    let mut vbuf = vbuf_from_str("abc\ndef");

    // Capture before edits
    let snap = vbuf.capture_snapshot();

    // Make edits
    vbuf.insert_at(Position::new(0, 3), "XYZ");
    assert!(vbuf.content().contains("XYZ"));

    // Restore removes edits
    vbuf.restore_snapshot(snap);
    assert_eq!(vbuf.content(), "abc\ndef");
}

#[test]
fn virtual_snapshot_clone() {
    let vbuf = vbuf_from_str("clone test");
    let snap: VirtualSnapshot = vbuf.capture_snapshot();
    let cloned = snap.clone();

    assert_eq!(cloned.add_buffer_len(), snap.add_buffer_len());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn virtual_snapshot_debug() {
    let vbuf = vbuf_from_str("debug");
    let snap = vbuf.capture_snapshot();
    let debug = format!("{snap:?}");
    assert!(debug.contains("VirtualSnapshot"));
}

// ── TextGeometry trait ───────────────────────────────────────────────

#[test]
fn vbuf_text_geometry_line_count() {
    let vbuf = vbuf_from_str("hello\nworld\nfoo");
    let tg: &dyn TextGeometry = &vbuf;
    assert_eq!(tg.line_count(), 3);
}

#[test]
fn vbuf_text_geometry_line_owned() {
    let vbuf = vbuf_from_str("hello\nworld");
    let tg: &dyn TextGeometry = &vbuf;
    let line = tg.line(0).unwrap();
    assert!(matches!(line, Cow::Owned(_)));
    assert_eq!(&*line, "hello");
}

#[test]
fn vbuf_text_geometry_line_out_of_bounds() {
    let vbuf = vbuf_from_str("hello");
    let tg: &dyn TextGeometry = &vbuf;
    assert!(tg.line(1).is_none());
}

#[test]
fn vbuf_text_geometry_line_len() {
    let vbuf = vbuf_from_str("hello\nab");
    let tg: &dyn TextGeometry = &vbuf;
    assert_eq!(tg.line_len(0), Some(5));
    assert_eq!(tg.line_len(1), Some(2));
    assert_eq!(tg.line_len(2), None);
}

#[test]
fn vbuf_text_geometry_is_empty() {
    let empty = vbuf_from_str("");
    let non_empty = vbuf_from_str("x");
    let tg_empty: &dyn TextGeometry = &empty;
    let tg_full: &dyn TextGeometry = &non_empty;
    assert!(tg_empty.is_empty());
    assert!(!tg_full.is_empty());
}

#[test]
fn vbuf_text_geometry_auto_coercion() {
    fn accepts_geometry(tg: &dyn TextGeometry) -> usize {
        tg.line_count()
    }
    let vbuf = vbuf_from_str("a\nb\nc");
    assert_eq!(accepts_geometry(&vbuf), 3);
}

#[test]
fn polymorphic_dispatch() {
    fn first_line(tg: &dyn TextGeometry) -> Option<String> {
        tg.line(0).map(Cow::into_owned)
    }

    let buf = Buffer::from_string("rope line");
    let vbuf = vbuf_from_str("virtual line");

    assert_eq!(first_line(&buf), Some("rope line".to_string()));
    assert_eq!(first_line(&vbuf), Some("virtual line".to_string()));
}

#[test]
fn polymorphic_line_count_matches() {
    let content = "line1\nline2\nline3";
    let buf = Buffer::from_string(content);
    let vbuf = vbuf_from_str(content);

    let buf_tg: &dyn TextGeometry = &buf;
    let vbuf_tg: &dyn TextGeometry = &vbuf;

    assert_eq!(buf_tg.line_count(), vbuf_tg.line_count());
    for i in 0..3 {
        assert_eq!(buf_tg.line(i).map(Cow::into_owned), vbuf_tg.line(i).map(Cow::into_owned),);
    }
}

// ── StorageOps trait ─────────────────────────────────────────────────

#[test]
fn storage_ops_byte_len() {
    let vbuf = vbuf_from_str("hello");
    let so: &dyn StorageOps = &vbuf;
    assert_eq!(so.byte_len(), 5);
}

#[test]
fn storage_ops_read_bytes() {
    let vbuf = vbuf_from_str("hello world");
    let so: &dyn StorageOps = &vbuf;
    let mut buf = [0u8; 5];
    let n = so.read_bytes(0, &mut buf);
    assert_eq!(n, 5);
    assert_eq!(&buf, b"hello");
}

// ── BufferOps trait ──────────────────────────────────────────────────

#[test]
fn buffer_ops_content() {
    let vbuf = vbuf_from_str("hello\nworld");
    let bo: &dyn BufferOps = &vbuf;
    assert_eq!(bo.content(), "hello\nworld");
    assert_eq!(bo.line_count(), 2);
    assert_eq!(bo.line(0).unwrap(), "hello");
}
