//! Tests for legacy encoding codec.

use {
    reovim_domain_text::Position,
    reovim_driver_codec::{ContentCodec, ContentType, DecodedEdit},
    reovim_driver_vfs::HeapByteSource,
};

use super::*;

fn latin1_codec() -> LegacyCodec {
    LegacyCodec::new(super::super::classifier::LATIN_1, false)
}

fn cp1252_codec() -> LegacyCodec {
    LegacyCodec::new(super::super::classifier::WINDOWS_1252, true)
}

#[test]
fn latin1_decode_accented() {
    let codec = latin1_codec();
    // "café" in Latin-1: c(0x63) a(0x61) f(0x66) é(0xE9)
    let bytes: &[u8] = &[0x63, 0x61, 0x66, 0xE9];
    let result = codec.decode(bytes).unwrap();
    assert_eq!(result.content, "caf\u{e9}");
    assert!(!result.lossy);
    assert!(!result.readonly);
}

#[test]
fn latin1_round_trip() {
    let codec = latin1_codec();
    let bytes: &[u8] = &[0x63, 0x61, 0x66, 0xE9];
    let result = codec.decode(bytes).unwrap();
    let encoded = codec
        .encode_fragment(&result.content, &result.metadata)
        .unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn cp1252_decode_smart_quotes() {
    let codec = cp1252_codec();
    // Left/right double quotes in Windows-1252: 0x93/0x94
    let bytes: &[u8] = &[0x93, b'h', b'i', 0x94];
    let result = codec.decode(bytes).unwrap();
    assert_eq!(result.content, "\u{201c}hi\u{201d}");
}

#[test]
fn cp1252_round_trip() {
    let codec = cp1252_codec();
    let bytes: &[u8] = &[0x93, b'h', b'i', 0x94];
    let result = codec.decode(bytes).unwrap();
    let encoded = codec
        .encode_fragment(&result.content, &result.metadata)
        .unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn cp1252_em_dash() {
    let codec = cp1252_codec();
    // Em dash is 0x97 in Windows-1252
    let bytes: &[u8] = &[b'a', 0x97, b'b'];
    let result = codec.decode(bytes).unwrap();
    assert_eq!(result.content, "a\u{2014}b");
}

#[test]
fn ascii_passthrough_latin1() {
    let codec = latin1_codec();
    let result = codec.decode(b"Hello").unwrap();
    assert_eq!(result.content, "Hello");
    let encoded = codec.encode_fragment("Hello", &result.metadata).unwrap();
    assert_eq!(encoded, b"Hello");
}

#[test]
fn ascii_passthrough_cp1252() {
    let codec = cp1252_codec();
    let result = codec.decode(b"Hello").unwrap();
    assert_eq!(result.content, "Hello");
    let encoded = codec.encode_fragment("Hello", &result.metadata).unwrap();
    assert_eq!(encoded, b"Hello");
}

#[test]
fn metadata_has_encoding_latin1() {
    let codec = latin1_codec();
    let result = codec.decode(b"Hi").unwrap();
    assert_eq!(result.metadata.get("encoding"), Some("latin-1"));
}

#[test]
fn metadata_has_encoding_cp1252() {
    let codec = cp1252_codec();
    let result = codec.decode(b"Hi").unwrap();
    assert_eq!(result.metadata.get("encoding"), Some("windows-1252"));
}

#[test]
fn latin1_encode_unencodable() {
    let codec = latin1_codec();
    let metadata = CodecMetadata::new(ContentType::new("encoding/latin-1"));
    // Korean character can't be encoded in Latin-1
    let result = codec.encode_fragment("한", &metadata);
    assert!(result.is_err());
}

#[test]
fn cp1252_encode_unencodable() {
    let codec = cp1252_codec();
    let metadata = CodecMetadata::new(ContentType::new("encoding/windows-1252"));
    // Emoji can't be encoded in Windows-1252
    let result = codec.encode_fragment("🎉", &metadata);
    assert!(result.is_err());
}

#[test]
fn empty_input_latin1() {
    let codec = latin1_codec();
    let result = codec.decode(b"").unwrap();
    assert_eq!(result.content, "");
}

#[test]
fn empty_input_cp1252() {
    let codec = cp1252_codec();
    let result = codec.decode(b"").unwrap();
    assert_eq!(result.content, "");
}

#[test]
fn is_windows_1252_accessor() {
    assert!(!latin1_codec().is_windows_1252());
    assert!(cp1252_codec().is_windows_1252());
}

#[test]
fn latin1_full_range_decode() {
    let codec = latin1_codec();
    // All bytes 0xA0..0xFF should decode to their Unicode equivalents
    let bytes: Vec<u8> = (0xA0..=0xFF).collect();
    let result = codec.decode(&bytes).unwrap();
    assert_eq!(result.content.chars().count(), bytes.len());
    for (i, ch) in result.content.chars().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let expected = (0xA0 + i) as u32;
        assert_eq!(ch as u32, expected);
    }
}

#[test]
fn translate_edit_ascii_replacement() {
    let codec = latin1_codec();
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Text {
        start: Position::new(0, 1),
        end: Position::new(0, 2),
        replacement: "Z".to_string(),
    };

    let translated = codec
        .translate_edit(&bytes, &edit)
        .expect("latin1 replacement accepted")
        .expect("produced a byte edit");

    assert_eq!(translated.offset, 1);
    assert_eq!(translated.old_bytes, b"b");
    assert_eq!(translated.new_bytes, b"Z");
}

#[test]
fn translate_edit_cp1252_replacement() {
    let codec = cp1252_codec();

    // left quote in Windows-1252: 0x93
    let raw = [0x93, b'h', b'i', 0x94];
    let bytes = HeapByteSource::new(raw);
    let edit = DecodedEdit::Text {
        start: Position::new(0, 0),
        end: Position::new(0, 1),
        replacement: "A".to_string(),
    };

    let translated = codec
        .translate_edit(&bytes, &edit)
        .expect("cp1252 replacement accepted")
        .expect("produced a byte edit");

    assert_eq!(translated.offset, 0);
    assert_eq!(translated.old_bytes, vec![0x93]);
    assert_eq!(translated.new_bytes, b"A");
}

#[test]
fn translate_edit_bytes_variant_is_not_supported() {
    use reovim_driver_codec::TranslateEditError;
    let codec = latin1_codec();
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Bytes {
        offset: 1,
        old_len: 1,
        new_bytes: b"x".to_vec(),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}
