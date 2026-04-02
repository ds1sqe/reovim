//! Tests for `LineIndex`.

use super::*;

// ── Construction ──────────────────────────────────────────────────────

#[test]
fn empty_content() {
    let idx = LineIndex::from_bytes(b"").unwrap();
    assert_eq!(idx.line_count(), 0);
    assert_eq!(idx.total_bytes(), 0);
    assert!(!idx.has_crlf());
}

#[test]
fn single_line_no_newline() {
    let idx = LineIndex::from_bytes(b"hello").unwrap();
    assert_eq!(idx.line_count(), 1);
    assert_eq!(idx.total_bytes(), 5);
    assert_eq!(idx.line_byte_range(0), Some(0..5));
    assert_eq!(idx.line_byte_range(1), None);
}

#[test]
fn single_line_with_trailing_newline() {
    // "hello\n" → 2 lines (line 0: "hello", line 1: "")
    let idx = LineIndex::from_bytes(b"hello\n").unwrap();
    assert_eq!(idx.line_count(), 2);
    assert_eq!(idx.line_byte_range(0), Some(0..5));
    assert_eq!(idx.line_byte_range(1), Some(6..6)); // empty last line
}

#[test]
fn multi_line() {
    // "a\nb\nc" → 3 lines
    let idx = LineIndex::from_bytes(b"a\nb\nc").unwrap();
    assert_eq!(idx.line_count(), 3);
    assert_eq!(idx.line_byte_range(0), Some(0..1)); // "a"
    assert_eq!(idx.line_byte_range(1), Some(2..3)); // "b"
    assert_eq!(idx.line_byte_range(2), Some(4..5)); // "c"
}

#[test]
fn trailing_newline_three_lines() {
    // "a\nb\n" → 3 lines (last is empty)
    let idx = LineIndex::from_bytes(b"a\nb\n").unwrap();
    assert_eq!(idx.line_count(), 3);
    assert_eq!(idx.line_byte_range(0), Some(0..1)); // "a"
    assert_eq!(idx.line_byte_range(1), Some(2..3)); // "b"
    assert_eq!(idx.line_byte_range(2), Some(4..4)); // "" (empty)
}

#[test]
fn consecutive_newlines() {
    // "a\n\n\nb" → 4 lines
    let idx = LineIndex::from_bytes(b"a\n\n\nb").unwrap();
    assert_eq!(idx.line_count(), 4);
    assert_eq!(idx.line_byte_range(0), Some(0..1)); // "a"
    assert_eq!(idx.line_byte_range(1), Some(2..2)); // ""
    assert_eq!(idx.line_byte_range(2), Some(3..3)); // ""
    assert_eq!(idx.line_byte_range(3), Some(4..5)); // "b"
}

#[test]
fn single_newline() {
    // "\n" → 2 lines (both empty)
    let idx = LineIndex::from_bytes(b"\n").unwrap();
    assert_eq!(idx.line_count(), 2);
    assert_eq!(idx.line_byte_range(0), Some(0..0)); // ""
    assert_eq!(idx.line_byte_range(1), Some(1..1)); // ""
}

// ── CRLF ──────────────────────────────────────────────────────────────

#[test]
fn crlf_detection() {
    let idx = LineIndex::from_bytes(b"a\r\nb").unwrap();
    assert!(idx.has_crlf());
    assert_eq!(idx.line_count(), 2);
    // \r is part of line 0 content, \n is at byte 2
    assert_eq!(idx.line_byte_range(0), Some(0..2)); // "a\r"
    assert_eq!(idx.line_byte_range(1), Some(3..4)); // "b"
}

#[test]
fn no_crlf() {
    let idx = LineIndex::from_bytes(b"a\nb").unwrap();
    assert!(!idx.has_crlf());
}

// ── byte_to_line ──────────────────────────────────────────────────────

#[test]
fn byte_to_line_simple() {
    // "a\nb\nc" → offsets [1, 3]
    let idx = LineIndex::from_bytes(b"a\nb\nc").unwrap();
    assert_eq!(idx.byte_to_line(0), 0); // 'a'
    assert_eq!(idx.byte_to_line(1), 0); // '\n' terminates line 0
    assert_eq!(idx.byte_to_line(2), 1); // 'b'
    assert_eq!(idx.byte_to_line(3), 1); // '\n' terminates line 1
    assert_eq!(idx.byte_to_line(4), 2); // 'c'
}

#[test]
fn byte_to_line_empty() {
    let idx = LineIndex::from_bytes(b"").unwrap();
    assert_eq!(idx.byte_to_line(0), 0);
    assert_eq!(idx.byte_to_line(100), 0);
}

#[test]
fn byte_to_line_clamped() {
    let idx = LineIndex::from_bytes(b"hello").unwrap();
    assert_eq!(idx.byte_to_line(0), 0);
    assert_eq!(idx.byte_to_line(4), 0);
    assert_eq!(idx.byte_to_line(100), 0); // clamped
}

// ── line_to_byte ──────────────────────────────────────────────────────

#[test]
fn line_to_byte_simple() {
    // "a\nb\nc"
    let idx = LineIndex::from_bytes(b"a\nb\nc").unwrap();
    assert_eq!(idx.line_to_byte(0), 0);
    assert_eq!(idx.line_to_byte(1), 2);
    assert_eq!(idx.line_to_byte(2), 4);
    assert_eq!(idx.line_to_byte(3), 5); // past end
}

#[test]
fn line_to_byte_empty() {
    let idx = LineIndex::from_bytes(b"").unwrap();
    assert_eq!(idx.line_to_byte(0), 0);
    assert_eq!(idx.line_to_byte(1), 0);
}

// ── Round-trip ────────────────────────────────────────────────────────

#[test]
fn byte_to_line_round_trip() {
    let content = b"hello\nworld\nfoo\nbar\nbaz";
    let idx = LineIndex::from_bytes(content).unwrap();
    for line in 0..idx.line_count() {
        let range = idx.line_byte_range(line).unwrap();
        assert_eq!(
            idx.byte_to_line(range.start),
            line,
            "byte_to_line(line_byte_range({line}).start) failed"
        );
    }
}

#[test]
fn line_to_byte_round_trip() {
    let content = b"a\nb\nc\nd\ne";
    let idx = LineIndex::from_bytes(content).unwrap();
    for line in 0..idx.line_count() {
        let byte = idx.line_to_byte(line);
        let range = idx.line_byte_range(line).unwrap();
        assert_eq!(byte, range.start, "line_to_byte({line}) != line_byte_range({line}).start");
    }
}

// ── from_str ──────────────────────────────────────────────────────────

#[test]
fn from_str_matches_from_bytes() {
    let s = "hello\nworld\n";
    let from_bytes = LineIndex::from_bytes(s.as_bytes()).unwrap();
    let from_str = LineIndex::build_from_str(s);
    assert_eq!(from_bytes, from_str);
}

// ── UTF-8 validation ─────────────────────────────────────────────────

#[test]
fn invalid_utf8_rejected() {
    let bad = [0xFF, 0xFE, b'\n', b'a'];
    assert!(LineIndex::from_bytes(&bad).is_err());
}

#[test]
fn valid_utf8_multibyte() {
    let s = "héllo\nwörld\n日本語";
    let idx = LineIndex::from_bytes(s.as_bytes()).unwrap();
    assert_eq!(idx.line_count(), 3);
}

// ── rebuild ──────────────────────────────────────────────────────────

#[test]
fn rebuild_updates_index() {
    let mut idx = LineIndex::from_bytes(b"a\nb").unwrap();
    assert_eq!(idx.line_count(), 2);

    idx.rebuild(b"x\ny\nz");
    assert_eq!(idx.line_count(), 3);
    assert_eq!(idx.line_byte_range(0), Some(0..1));
    assert_eq!(idx.line_byte_range(1), Some(2..3));
    assert_eq!(idx.line_byte_range(2), Some(4..5));
}

#[test]
fn rebuild_empty() {
    let mut idx = LineIndex::from_bytes(b"hello").unwrap();
    idx.rebuild(b"");
    assert_eq!(idx.line_count(), 0);
}

// ── Large index ──────────────────────────────────────────────────────

#[test]
fn large_line_count() {
    // 10,000 lines
    let content: Vec<u8> = (0..10_000)
        .flat_map(|_| b"line\n".iter().copied())
        .collect();
    let idx = LineIndex::from_bytes(&content).unwrap();
    // 10,000 newlines → 10,001 lines (last is empty after trailing \n)
    assert_eq!(idx.line_count(), 10_001);
    assert_eq!(idx.line_byte_range(0), Some(0..4));
    assert_eq!(idx.line_byte_range(9_999), Some(49_995..49_999));
    assert_eq!(idx.line_byte_range(10_000), Some(50_000..50_000));
}

// ── Edge: byte_to_line on newline byte ───────────────────────────────

#[test]
fn byte_to_line_on_newline_byte() {
    // "ab\ncd\n" → offsets [2, 5]
    let idx = LineIndex::from_bytes(b"ab\ncd\n").unwrap();
    // Byte 2 is '\n' → terminates line 0, so it belongs to line 0
    assert_eq!(idx.byte_to_line(2), 0);
    // Byte 5 is '\n' → terminates line 1, so it belongs to line 1
    assert_eq!(idx.byte_to_line(5), 1);
}

// ── InvalidUtf8 display ─────────────────────────────────────────────

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn invalid_utf8_display() {
    let err = InvalidUtf8;
    assert_eq!(format!("{err}"), "file is not valid UTF-8");
    // Also test std::error::Error impl exists
    let _: &dyn std::error::Error = &err;
}
