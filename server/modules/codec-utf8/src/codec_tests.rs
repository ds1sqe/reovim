//! Tests for UTF-8 codec.

use {
    reovim_driver_codec::{CodecMetadata, ContentCodec, ContentType, DecodedEdit},
    reovim_driver_vfs::HeapByteSource,
    reovim_kernel::api::v1::ByteEdit,
};

use super::*;

#[test]
fn decode_plain_utf8() {
    let codec = Utf8Codec::new();
    let result = codec.decode(b"hello world").unwrap();
    assert_eq!(result.content, "hello world");
    assert!(!result.lossy);
    assert!(!result.readonly);
    assert!(result.annotations.is_empty());
    assert_eq!(result.metadata.get(META_LINE_ENDING), Some(LINE_ENDING_LF));
    assert_eq!(result.metadata.get(META_BOM), None);
}

#[test]
fn decode_utf8_with_bom() {
    let codec = Utf8Codec::new();
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"hello");
    let result = codec.decode(&bytes).unwrap();
    assert_eq!(result.content, "hello");
    assert_eq!(result.metadata.get(META_BOM), Some("true"));
}

#[test]
fn decode_crlf_normalization() {
    let codec = Utf8Codec::new();
    let result = codec.decode(b"line1\r\nline2\r\nline3").unwrap();
    assert_eq!(result.content, "line1\nline2\nline3");
    assert_eq!(result.metadata.get(META_LINE_ENDING), Some(LINE_ENDING_CRLF));
}

#[test]
fn decode_lf_preserved() {
    let codec = Utf8Codec::new();
    let result = codec.decode(b"line1\nline2\n").unwrap();
    assert_eq!(result.content, "line1\nline2\n");
    assert_eq!(result.metadata.get(META_LINE_ENDING), Some(LINE_ENDING_LF));
}

#[test]
fn decode_empty() {
    let codec = Utf8Codec::new();
    let result = codec.decode(b"").unwrap();
    assert_eq!(result.content, "");
    assert!(!result.lossy);
}

#[test]
fn decode_multibyte() {
    let codec = Utf8Codec::new();
    let text = "한글 Hello 日本語";
    let result = codec.decode(text.as_bytes()).unwrap();
    assert_eq!(result.content, text);
}

#[test]
fn decode_invalid_utf8() {
    let codec = Utf8Codec::new();
    let result = codec.decode(&[0xFF, 0xFE]);
    assert!(result.is_err());
}

#[test]
fn decode_invalid_utf8_after_bom() {
    let codec = Utf8Codec::new();
    let mut bytes = vec![0xEF, 0xBB, 0xBF]; // BOM
    bytes.extend_from_slice(&[0xFF, 0xFE]); // invalid
    let result = codec.decode(&bytes);
    assert!(result.is_err());
}

#[test]
fn encode_plain() {
    let metadata = CodecMetadata::new(ContentType::new("text/utf-8"));
    let encoded = super::encode_fragment("hello", &metadata);
    assert_eq!(encoded, b"hello");
}

#[test]
fn encode_with_bom() {
    let mut metadata = CodecMetadata::new(ContentType::new("text/utf-8"));
    metadata.set(META_BOM, "true");
    let encoded = super::encode_fragment("hello", &metadata);
    assert_eq!(&encoded[..3], &[0xEF, 0xBB, 0xBF]);
    assert_eq!(&encoded[3..], b"hello");
}

#[test]
fn encode_with_crlf() {
    let mut metadata = CodecMetadata::new(ContentType::new("text/utf-8"));
    metadata.set(META_LINE_ENDING, LINE_ENDING_CRLF);
    let encoded = super::encode_fragment("line1\nline2\n", &metadata);
    assert_eq!(encoded, b"line1\r\nline2\r\n");
}

#[test]
fn round_trip_plain() {
    let codec = Utf8Codec::new();
    let original = b"hello world\n";
    let decoded = codec.decode(original).unwrap();
    let encoded = super::encode_fragment(&decoded.content, &decoded.metadata);
    assert_eq!(encoded, original);
}

#[test]
fn round_trip_bom() {
    let codec = Utf8Codec::new();
    let mut original = vec![0xEF, 0xBB, 0xBF];
    original.extend_from_slice(b"hello\n");
    let decoded = codec.decode(&original).unwrap();
    let encoded = super::encode_fragment(&decoded.content, &decoded.metadata);
    assert_eq!(encoded, original);
}

#[test]
fn round_trip_crlf() {
    let codec = Utf8Codec::new();
    let original = b"line1\r\nline2\r\nline3\r\n";
    let decoded = codec.decode(original).unwrap();
    assert_eq!(decoded.content, "line1\nline2\nline3\n");
    let encoded = super::encode_fragment(&decoded.content, &decoded.metadata);
    assert_eq!(encoded, original);
}

#[test]
fn round_trip_bom_and_crlf() {
    let codec = Utf8Codec::new();
    let mut original = vec![0xEF, 0xBB, 0xBF];
    original.extend_from_slice(b"line1\r\nline2\r\n");
    let decoded = codec.decode(&original).unwrap();
    assert_eq!(decoded.content, "line1\nline2\n");
    assert_eq!(decoded.metadata.get(META_BOM), Some("true"));
    assert_eq!(decoded.metadata.get(META_LINE_ENDING), Some(LINE_ENDING_CRLF));
    let encoded = super::encode_fragment(&decoded.content, &decoded.metadata);
    assert_eq!(encoded, original);
}

#[test]
fn round_trip_multibyte() {
    let codec = Utf8Codec::new();
    let original = "한글 Hello 日本語\n".as_bytes();
    let decoded = codec.decode(original).unwrap();
    let encoded = super::encode_fragment(&decoded.content, &decoded.metadata);
    assert_eq!(encoded, original);
}

#[test]
fn decode_result_is_valid() {
    let codec = Utf8Codec::new();
    let result = codec.decode(b"hello").unwrap();
    assert!(result.is_valid());
}

#[test]
fn content_type_is_utf8() {
    let codec = Utf8Codec::new();
    let result = codec.decode(b"hello").unwrap();
    assert_eq!(result.metadata.content_type().as_str(), ContentType::UTF8);
}

#[test]
fn translate_edit_text_insertion_on_plain_utf8() {
    let codec = Utf8Codec::new();
    let bytes = HeapByteSource::new("hello world");
    let edit = DecodedEdit::Text {
        start: reovim_types_text::Position::new(0, 5),
        end: reovim_types_text::Position::new(0, 5),
        replacement: ",".to_string(),
    };

    let byte_edit = codec
        .translate_edit(&bytes, &edit)
        .expect("plain text edits should translate");

    assert_eq!(byte_edit, ByteEdit::insert(5, b","));
}

#[test]
fn translate_edit_text_insertion_respects_crlf_and_bom() {
    let codec = Utf8Codec::new();
    let mut raw = vec![0xEF, 0xBB, 0xBF];
    raw.extend_from_slice(b"foo\r\nbar");

    let bytes = HeapByteSource::new(raw);
    let edit = DecodedEdit::Text {
        start: reovim_types_text::Position::new(1, 0),
        end: reovim_types_text::Position::new(1, 0),
        replacement: "Z".to_string(),
    };

    let byte_edit = codec
        .translate_edit(&bytes, &edit)
        .expect("CRLF + BOM edits should translate");

    assert_eq!(byte_edit, ByteEdit::insert(8, b"Z"));
}

#[test]
fn translate_edit_text_requires_valid_range() {
    let codec = Utf8Codec::new();
    let bytes = HeapByteSource::new("héllo\r\n世界");
    let edit = DecodedEdit::Text {
        start: reovim_types_text::Position::new(0, 1),
        end: reovim_types_text::Position::new(1, 0),
        replacement: String::new(),
    };

    let byte_edit = codec
        .translate_edit(&bytes, &edit)
        .expect("edit should map");

    let raw = "héllo\r\n世界".as_bytes();
    assert_eq!(byte_edit.offset, 1);
    assert_eq!(byte_edit.old_bytes, raw[1..8].to_vec());
}

#[test]
fn translate_edit_bytes_edit_is_not_supported() {
    let codec = Utf8Codec::new();
    let bytes = HeapByteSource::new("hello");
    let edit = DecodedEdit::Bytes {
        offset: 1,
        old_len: 1,
        new_bytes: b"x".to_vec(),
    };

    assert!(codec.translate_edit(&bytes, &edit).is_none());
}
