//! Tests for UTF-8 domain codec implementations.

use reovim_driver_codec::{CodecMetadata, ContentType, Decode, Encode, Index};
use reovim_driver_vfs::ByteEdit;
use reovim_types_text::{Text, TextEdit, TextPosition};

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
    let result = <Utf8Codec as Encode<Text>>::encode(&codec, &String::from("hello"), &metadata)
        .unwrap();
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

// ─── Index<Text> — build ────────────────────────────────────────────────────

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

// ─── Index<Text> — notify ───────────────────────────────────────────────────

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
