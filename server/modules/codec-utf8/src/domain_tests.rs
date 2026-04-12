//! Tests for UTF-8 domain codec implementations.

use {
    reovim_domain_text::{Text, TextEdit, TextPosition},
    reovim_driver_codec::{ByteNotifiable, CodecMetadata, ContentType, Decode, Encode, Index},
    reovim_kernel::api::v1::ByteEdit,
};

use super::{Utf8Codec, Utf8LineIndex};

// ─── Decode<Text> ───────────────────────────────────────────────────────────

#[test]
fn decode_simple_utf8() {
    let codec = Utf8Codec::new();
    let result = <Utf8Codec as Decode<Text>>::decode(&codec, b"hello\nworld").unwrap();
    assert_eq!(result.content, "hello\nworld");
    assert!(!result.lossy);
    assert!(!result.readonly);
}

#[test]
fn decode_with_bom() {
    let codec = Utf8Codec::new();
    let mut input = vec![0xEF, 0xBB, 0xBF];
    input.extend_from_slice(b"bom text");
    let result = <Utf8Codec as Decode<Text>>::decode(&codec, &input).unwrap();
    assert_eq!(result.content, "bom text");
    assert_eq!(result.metadata.get("bom"), Some("true"));
}

#[test]
fn decode_crlf_normalization() {
    let codec = Utf8Codec::new();
    let result = <Utf8Codec as Decode<Text>>::decode(&codec, b"line1\r\nline2").unwrap();
    assert_eq!(result.content, "line1\nline2");
    assert_eq!(result.metadata.get("line_ending"), Some("crlf"));
}

#[test]
fn decode_invalid_utf8() {
    let codec = Utf8Codec::new();
    let result = <Utf8Codec as Decode<Text>>::decode(&codec, &[0xFF, 0xFE]);
    assert!(result.is_err());
}

// ─── Encode<Text> ───────────────────────────────────────────────────────────

#[test]
fn encode_simple() {
    let codec = Utf8Codec::new();
    let metadata = CodecMetadata::new(ContentType::new("text/utf-8"));
    let result =
        <Utf8Codec as Encode<Text>>::encode(&codec, &String::from("hello"), &metadata).unwrap();
    assert_eq!(result, b"hello");
}

#[test]
fn encode_roundtrip() {
    let codec = Utf8Codec::new();
    let original = b"hello\r\nworld";
    let decoded = <Utf8Codec as Decode<Text>>::decode(&codec, original).unwrap();
    let re_encoded =
        <Utf8Codec as Encode<Text>>::encode(&codec, &decoded.content, &decoded.metadata).unwrap();
    assert_eq!(re_encoded, original);
}

#[test]
fn encode_roundtrip_with_bom() {
    let codec = Utf8Codec::new();
    let mut original = vec![0xEF, 0xBB, 0xBF];
    original.extend_from_slice(b"bom text\r\n");
    let decoded = <Utf8Codec as Decode<Text>>::decode(&codec, &original).unwrap();
    let re_encoded =
        <Utf8Codec as Encode<Text>>::encode(&codec, &decoded.content, &decoded.metadata).unwrap();
    assert_eq!(re_encoded, original);
}

// ─── ByteNotifiable — build ─────────────────────────────────────────────────

#[test]
fn index_build_empty() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"");
    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 0)), Some(0));
}

#[test]
fn index_build_single_line() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"hello");
    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.offset_to_position(3), Some(TextPosition::new(0, 3)));
    assert_eq!(idx.offset_to_position(5), Some(TextPosition::new(0, 5)));
    assert_eq!(idx.offset_to_position(6), None);
}

#[test]
fn index_build_multi_line() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"ab\ncd\ne");

    // Line 0: "ab\n" (bytes 0-2)
    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.offset_to_position(1), Some(TextPosition::new(0, 1)));
    assert_eq!(idx.offset_to_position(2), Some(TextPosition::new(0, 2)));

    // Line 1: "cd\n" (bytes 3-5)
    assert_eq!(idx.offset_to_position(3), Some(TextPosition::new(1, 0)));
    assert_eq!(idx.offset_to_position(4), Some(TextPosition::new(1, 1)));
    assert_eq!(idx.offset_to_position(5), Some(TextPosition::new(1, 2)));

    // Line 2: "e" (byte 6)
    assert_eq!(idx.offset_to_position(6), Some(TextPosition::new(2, 0)));
}

#[test]
fn index_to_bytes_multi_line() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"ab\ncd\ne");

    assert_eq!(idx.to_bytes(&TextPosition::new(0, 0)), Some(0));
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 2)), Some(2));
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 0)), Some(3));
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 1)), Some(4));
    assert_eq!(idx.to_bytes(&TextPosition::new(2, 0)), Some(6));
    assert_eq!(idx.to_bytes(&TextPosition::new(2, 1)), Some(7));
    assert_eq!(idx.to_bytes(&TextPosition::new(3, 0)), None);
}

// ─── ByteNotifiable — notify ────────────────────────────────────────────────

#[test]
fn index_notify_insert_no_newline() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"ab\ncd");

    // Insert "xy" at offset 1 → "axyb\ncd"
    idx.notify(&ByteEdit::insert(1, b"xy"));

    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.offset_to_position(3), Some(TextPosition::new(0, 3)));
    assert_eq!(idx.offset_to_position(4), Some(TextPosition::new(0, 4)));
    // Line 1 starts at 5 (was 3, shifted by 2)
    assert_eq!(idx.offset_to_position(5), Some(TextPosition::new(1, 0)));
}

#[test]
fn index_notify_insert_with_newline() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"ab\ncd");

    // Insert "x\ny" at offset 1 → "ax\nyb\ncd"
    idx.notify(&ByteEdit::insert(1, b"x\ny"));

    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.offset_to_position(1), Some(TextPosition::new(0, 1)));
    // New line starts at 3 (after "x\n")
    assert_eq!(idx.offset_to_position(3), Some(TextPosition::new(1, 0)));
    // "yb\n" → offset 5 is the newline
    assert_eq!(idx.offset_to_position(6), Some(TextPosition::new(2, 0)));
}

#[test]
fn index_notify_delete_no_newline() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"abcd\nef");

    // Delete "bc" at offset 1 → "ad\nef"
    idx.notify(&ByteEdit::delete(1, b"bc"));

    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.offset_to_position(2), Some(TextPosition::new(0, 2)));
    // Line 1 was at 5, now at 3
    assert_eq!(idx.offset_to_position(3), Some(TextPosition::new(1, 0)));
}

#[test]
fn index_notify_delete_with_newline() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"ab\ncd\nef");

    // Delete "\ncd" at offset 2 → "ab\nef"
    idx.notify(&ByteEdit::delete(2, b"\ncd"));

    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.offset_to_position(2), Some(TextPosition::new(0, 2)));
    // Line 1 was at 6 ("ef"), now at 3
    assert_eq!(idx.offset_to_position(3), Some(TextPosition::new(1, 0)));
}

// ─── Index<Text> — translate_edit ───────────────────────────────────────────

#[test]
fn translate_insert_edit() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"hello\nworld");

    let edit = TextEdit::insert(TextPosition::new(1, 2), "XY");
    let byte_edit = idx.translate_edit(&edit).unwrap();
    assert_eq!(byte_edit.offset, 8); // line 1 starts at 6, col 2 → offset 8
    assert!(byte_edit.old_bytes.is_empty());
    assert_eq!(byte_edit.new_bytes, b"XY");
}

#[test]
fn translate_delete_edit() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"hello\nworld");

    let edit = TextEdit::delete(TextPosition::new(0, 2), "llo");
    let byte_edit = idx.translate_edit(&edit).unwrap();
    assert_eq!(byte_edit.offset, 2);
    assert_eq!(byte_edit.old_bytes, b"llo");
    assert!(byte_edit.new_bytes.is_empty());
}

#[test]
fn translate_edit_out_of_bounds() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"hi");

    let edit = TextEdit::insert(TextPosition::new(5, 0), "oob");
    assert!(idx.translate_edit(&edit).is_none());
}

// ─── Default impl ───────────────────────────────────────────────────────────

#[test]
fn utf8_line_index_default() {
    let idx = Utf8LineIndex::default();
    assert!(idx.offset_to_position(0).is_none());
}

// ─── ByteNotifiable ─────────────────────────────────────────────────────────

#[test]
fn byte_notifiable_notify() {
    let mut idx = Utf8LineIndex::new();
    ByteNotifiable::build(&mut idx, b"hello\nworld");

    // Use ByteNotifiable::notify
    let edit = ByteEdit::insert(5, b"!");
    ByteNotifiable::notify(&mut idx, &edit);

    // Verify index was updated (line 1 start shifted by 1)
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 0)), Some(7));
}

#[test]
fn byte_notifiable_build() {
    let mut idx = Utf8LineIndex::new();

    // Build via ByteNotifiable
    ByteNotifiable::build(&mut idx, b"a\nb\nc");

    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.offset_to_position(2), Some(TextPosition::new(1, 0)));
    assert_eq!(idx.offset_to_position(4), Some(TextPosition::new(2, 0)));
}

#[test]
fn byte_notifiable_is_object_safe() {
    let mut idx = Utf8LineIndex::new();
    ByteNotifiable::build(&mut idx, b"hello");

    let notifiable: &mut dyn ByteNotifiable = &mut idx;
    notifiable.notify(&ByteEdit::insert(5, b"!"));
}

#[test]
fn byte_notifiable_boxed_storage() {
    let mut idx: Box<dyn ByteNotifiable> = Box::new(Utf8LineIndex::new());
    idx.build(b"line1\nline2");
    idx.notify(&ByteEdit::insert(5, b"!"));
    // No panic — proves boxed trait object works for CodecSessionState storage
}

// ─── Multi-byte UTF-8 — to_bytes ──────────────────────────────────────────

#[test]
fn to_bytes_two_byte_char() {
    let mut idx = Utf8LineIndex::new();
    // "héllo\nworld" — 'é' is 2 bytes (U+00E9: 0xC3 0xA9)
    idx.build("héllo\nworld".as_bytes());

    // Line 0: h(0) é(1..3) l(3) l(4) o(5) \n(6) — 7 bytes
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 0)), Some(0)); // 'h'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 1)), Some(1)); // 'é'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 2)), Some(3)); // 'l' after 2-byte é
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 3)), Some(4)); // 'l'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 4)), Some(5)); // 'o'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 5)), Some(6)); // '\n'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 6)), Some(7)); // past '\n'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 7)), None); // out of bounds

    // Line 1: w(7) o(8) r(9) l(10) d(11) — 5 bytes
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 0)), Some(7));
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 3)), Some(10));
}

#[test]
fn to_bytes_three_byte_cjk() {
    let mut idx = Utf8LineIndex::new();
    // "日本語" — each CJK char is 3 bytes (9 bytes total)
    idx.build("日本語".as_bytes());

    assert_eq!(idx.to_bytes(&TextPosition::new(0, 0)), Some(0)); // '日'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 1)), Some(3)); // '本'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 2)), Some(6)); // '語'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 3)), Some(9)); // end
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 4)), None);
}

#[test]
fn to_bytes_four_byte_emoji() {
    let mut idx = Utf8LineIndex::new();
    // "a😀b" — '😀' is 4 bytes (U+1F600: 0xF0 0x9F 0x98 0x80)
    idx.build("a😀b".as_bytes());

    assert_eq!(idx.to_bytes(&TextPosition::new(0, 0)), Some(0)); // 'a'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 1)), Some(1)); // '😀'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 2)), Some(5)); // 'b'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 3)), Some(6)); // end
}

#[test]
fn to_bytes_mixed_widths_multi_line() {
    let mut idx = Utf8LineIndex::new();
    // Line 0: "café\n" (5 chars, 6 bytes — é is 2 bytes)
    // Line 1: "日本\n" (2 chars, 7 bytes — each CJK 3 bytes + \n)
    // Line 2: "ok"    (2 chars, 2 bytes)
    idx.build("café\n日本\nok".as_bytes());

    // Line 0: c(0) a(1) f(2) é(3..5) \n(5)
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 3)), Some(3)); // 'é'
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 4)), Some(5)); // '\n'

    // Line 1 starts at byte 6: 日(6..9) 本(9..12) \n(12)
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 0)), Some(6));
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 1)), Some(9));
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 2)), Some(12));

    // Line 2 starts at byte 13: o(13) k(14)
    assert_eq!(idx.to_bytes(&TextPosition::new(2, 0)), Some(13));
    assert_eq!(idx.to_bytes(&TextPosition::new(2, 1)), Some(14));
}

// ─── Multi-byte UTF-8 — offset_to_position ─────────────────────────────────

#[test]
fn offset_to_position_two_byte_char() {
    let mut idx = Utf8LineIndex::new();
    idx.build("héllo\nworld".as_bytes());

    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0))); // 'h'
    assert_eq!(idx.offset_to_position(1), Some(TextPosition::new(0, 1))); // start of 'é'
    assert_eq!(idx.offset_to_position(3), Some(TextPosition::new(0, 2))); // 'l' after é
    assert_eq!(idx.offset_to_position(7), Some(TextPosition::new(1, 0))); // 'w'
}

#[test]
fn offset_to_position_mid_char_returns_none() {
    let mut idx = Utf8LineIndex::new();
    // "héllo" — 'é' occupies bytes 1..3 (0xC3 0xA9)
    idx.build("héllo".as_bytes());

    // Byte 2 is the continuation byte of 'é' — not a valid char boundary.
    // from_utf8(&raw[0..2]) = from_utf8(&[0x68, 0xC3]) → Err (incomplete sequence)
    assert_eq!(idx.offset_to_position(2), None);
}

#[test]
fn offset_to_position_cjk() {
    let mut idx = Utf8LineIndex::new();
    idx.build("日本語".as_bytes());

    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.offset_to_position(3), Some(TextPosition::new(0, 1)));
    assert_eq!(idx.offset_to_position(6), Some(TextPosition::new(0, 2)));
    assert_eq!(idx.offset_to_position(9), Some(TextPosition::new(0, 3)));
    // Mid-char offsets
    assert_eq!(idx.offset_to_position(1), None);
    assert_eq!(idx.offset_to_position(4), None);
}

#[test]
fn offset_to_position_emoji() {
    let mut idx = Utf8LineIndex::new();
    idx.build("a😀b".as_bytes());

    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0))); // 'a'
    assert_eq!(idx.offset_to_position(1), Some(TextPosition::new(0, 1))); // '😀'
    assert_eq!(idx.offset_to_position(5), Some(TextPosition::new(0, 2))); // 'b'
    // Mid-emoji offsets
    assert_eq!(idx.offset_to_position(2), None);
    assert_eq!(idx.offset_to_position(3), None);
    assert_eq!(idx.offset_to_position(4), None);
}

// ─── Multi-byte UTF-8 — translate_edit ──────────────────────────────────────

#[test]
fn translate_insert_edit_multi_byte() {
    let mut idx = Utf8LineIndex::new();
    idx.build("héllo\nworld".as_bytes());

    // Insert at char 2 of line 0 (= byte 3, after 2-byte 'é')
    let edit = TextEdit::insert(TextPosition::new(0, 2), "X");
    let byte_edit = idx.translate_edit(&edit).unwrap();
    assert_eq!(byte_edit.offset, 3);
    assert!(byte_edit.old_bytes.is_empty());
    assert_eq!(byte_edit.new_bytes, b"X");
}

#[test]
fn translate_delete_edit_multi_byte() {
    let mut idx = Utf8LineIndex::new();
    idx.build("café\nworld".as_bytes());

    // Delete 'é' at char 3 of line 0 (= byte 3, 'é' is 2 bytes)
    let edit = TextEdit::delete(TextPosition::new(0, 3), "é");
    let byte_edit = idx.translate_edit(&edit).unwrap();
    assert_eq!(byte_edit.offset, 3);
    assert_eq!(byte_edit.old_bytes, "é".as_bytes());
    assert!(byte_edit.new_bytes.is_empty());
}

// ─── Multi-byte UTF-8 — notify + verify ─────────────────────────────────────

#[test]
fn notify_insert_multi_byte_then_lookup() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"hello\nworld");

    // Insert 'é' (2 bytes) at byte 5 → "hellé\nworld" wait, let's insert at byte 0
    // Insert "日" (3 bytes) at byte 0 → "日hello\nworld"
    idx.notify(&ByteEdit::insert(0, "日".as_bytes()));

    // Line 0: 日(0..3) h(3) e(4) l(5) l(6) o(7) \n(8) — 9 bytes
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 0)), Some(0));
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 1)), Some(3)); // 'h' after 3-byte CJK
    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    assert_eq!(idx.offset_to_position(3), Some(TextPosition::new(0, 1)));

    // Line 1 starts at byte 9
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 0)), Some(9));
}

#[test]
fn notify_delete_multi_byte_then_lookup() {
    let mut idx = Utf8LineIndex::new();
    idx.build("héllo\nworld".as_bytes());

    // Delete 'é' (bytes 1..3) → "hllo\nworld"
    idx.notify(&ByteEdit::delete(1, "é".as_bytes()));

    // Line 0: h(0) l(1) l(2) o(3) \n(4) — 5 bytes
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 1)), Some(1)); // 'l'
    assert_eq!(idx.offset_to_position(1), Some(TextPosition::new(0, 1)));

    // Line 1 starts at byte 5 (was 7, shifted by -2)
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 0)), Some(5));
}

// ─── Large-edit verification (E.3) ──────────────────────────────────────────

/// Build a large content with `n` lines, each 40 chars + newline = 41 bytes.
fn make_large_content(n: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(n * 41);
    for i in 0..n {
        use std::fmt::Write;
        let mut line = String::new();
        // "line " (5) + zero-padded 35-digit number = 40 chars + \n = 41 bytes.
        write!(line, "line {i:>035}").unwrap();
        buf.extend_from_slice(line.as_bytes());
        buf.push(b'\n');
    }
    buf
}

#[test]
fn large_build_and_lookup() {
    let content = make_large_content(10_000);
    let mut idx = Utf8LineIndex::new();
    idx.build(&content);

    // First line.
    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    // Last line (line 9999). Each line is 41 bytes.
    let last_start = 9999 * 41;
    assert_eq!(idx.offset_to_position(last_start), Some(TextPosition::new(9999, 0)));
    // Reverse: position → byte.
    assert_eq!(idx.to_bytes(&TextPosition::new(9999, 0)), Some(last_start));
    assert_eq!(idx.to_bytes(&TextPosition::new(5000, 5)), Some(5000 * 41 + 5));
}

#[test]
fn large_insert_many_newlines() {
    let content = make_large_content(1_000);
    let mut idx = Utf8LineIndex::new();
    idx.build(&content);

    // Line size = 41 bytes. Line 2 starts at byte 82, line 3 at 123.
    // Insert 500 newlines at byte 100 (within line 2).
    let insert_bytes: Vec<u8> = vec![b'\n'; 500];
    idx.notify(&ByteEdit::insert(100, &insert_bytes));

    // Line 0 unchanged.
    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    // After insertion, 500 new (empty) lines were created.
    // Line 3 starts at byte 101 (after first inserted \n at 100).
    assert_eq!(idx.offset_to_position(101), Some(TextPosition::new(3, 0)));
    // Original line 3 (byte 123) shifted by 500 → byte 623, now line 503.
    assert_eq!(idx.offset_to_position(623), Some(TextPosition::new(503, 0)));
}

#[test]
fn large_delete_many_newlines() {
    let content = make_large_content(1_000);
    let mut idx = Utf8LineIndex::new();
    idx.build(&content);

    // Line size = 41 bytes. Delete lines 1..100 = bytes 41..4141 (100 lines).
    let deleted = content[41..4141].to_vec();
    idx.notify(&ByteEdit::delete(41, &deleted));

    // Line 0 unchanged.
    assert_eq!(idx.offset_to_position(0), Some(TextPosition::new(0, 0)));
    // After deletion, original line 101 is now line 1 at byte 41.
    assert_eq!(idx.offset_to_position(41), Some(TextPosition::new(1, 0)));
    // Position → byte roundtrip.
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 0)), Some(41));
}

#[test]
fn large_replace_preserves_consistency() {
    let content = make_large_content(1_000);
    let mut idx = Utf8LineIndex::new();
    idx.build(&content);

    // Replace first line content (bytes 0..40) with "short" — no newline change.
    // Line 0 = 40 chars content + \n = 41 bytes. We replace the 40 content bytes.
    let old = content[0..40].to_vec();
    let new_text = b"short";
    idx.notify(&ByteEdit::replace(0, &old, new_text));

    // Line 0 is now "short\n" = 6 bytes (was 41).
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 0)), Some(0));
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 5)), Some(5)); // '\n'
    // Line 1 shifted left by 35 bytes (41 - 6).
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 0)), Some(6));
    // Last line (999): line k starts at 6 + (k-1)*41 for k >= 1.
    let last_start = 6 + 998 * 41;
    assert_eq!(idx.to_bytes(&TextPosition::new(999, 0)), Some(last_start));
}

#[test]
fn large_sequential_edits_stay_consistent() {
    let mut idx = Utf8LineIndex::new();
    idx.build(b"aaa\nbbb\nccc\nddd\neee");

    // Simulate typing: insert chars one at a time at line 2, col 0
    for ch in b"hello" {
        let offset = idx.to_bytes(&TextPosition::new(2, 0)).unwrap();
        idx.notify(&ByteEdit::insert(offset, &[*ch]));
    }

    // Line 2 now starts with "hello" before "ccc"
    // Content: "aaa\nbbb\nhelloccc\nddd\neee"
    assert_eq!(idx.to_bytes(&TextPosition::new(0, 0)), Some(0));
    assert_eq!(idx.to_bytes(&TextPosition::new(1, 0)), Some(4));
    assert_eq!(idx.to_bytes(&TextPosition::new(2, 0)), Some(8));
    assert_eq!(idx.to_bytes(&TextPosition::new(2, 5)), Some(13)); // 'c' in "ccc"
    assert_eq!(idx.to_bytes(&TextPosition::new(3, 0)), Some(17)); // "ddd"
}
