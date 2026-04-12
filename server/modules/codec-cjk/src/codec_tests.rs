//! Tests for CJK encoding codec.

use {
    reovim_domain_text::Position,
    reovim_driver_codec::{ContentCodec, ContentType, DecodedEdit},
    reovim_driver_vfs::HeapByteSource,
};

use super::*;

fn euc_kr_codec() -> CjkCodec {
    CjkCodec::new(encoding_rs::EUC_KR, "encoding/euc-kr")
}

fn shift_jis_codec() -> CjkCodec {
    CjkCodec::new(encoding_rs::SHIFT_JIS, "encoding/shift-jis")
}

fn gbk_codec() -> CjkCodec {
    CjkCodec::new(encoding_rs::GBK, "encoding/gbk")
}

fn big5_codec() -> CjkCodec {
    CjkCodec::new(encoding_rs::BIG5, "encoding/big5")
}

#[test]
fn euc_kr_round_trip() {
    let codec = euc_kr_codec();
    // "한글" in EUC-KR
    let euc_kr_bytes: &[u8] = &[0xC7, 0xD1, 0xB1, 0xDB];
    let result = codec.decode(euc_kr_bytes).unwrap();
    assert_eq!(result.content, "한글");
    assert!(!result.lossy);
    assert!(!result.readonly);

    // Round-trip encode
    let encoded = codec
        .encode_fragment(&result.content, &result.metadata)
        .unwrap();
    assert_eq!(encoded, euc_kr_bytes);
}

#[test]
fn shift_jis_round_trip() {
    let codec = shift_jis_codec();
    // "日本語" in Shift-JIS
    let shift_jis_bytes: &[u8] = &[0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA];
    let result = codec.decode(shift_jis_bytes).unwrap();
    assert_eq!(result.content, "日本語");

    let encoded = codec
        .encode_fragment(&result.content, &result.metadata)
        .unwrap();
    assert_eq!(encoded, shift_jis_bytes);
}

#[test]
fn gbk_round_trip() {
    let codec = gbk_codec();
    // "中文" in GBK
    let gbk_bytes: &[u8] = &[0xD6, 0xD0, 0xCE, 0xC4];
    let result = codec.decode(gbk_bytes).unwrap();
    assert_eq!(result.content, "中文");

    let encoded = codec
        .encode_fragment(&result.content, &result.metadata)
        .unwrap();
    assert_eq!(encoded, gbk_bytes);
}

#[test]
fn big5_round_trip() {
    let codec = big5_codec();
    // "中文" in Big5
    let big5_bytes: &[u8] = &[0xA4, 0xA4, 0xA4, 0xE5];
    let result = codec.decode(big5_bytes).unwrap();
    assert_eq!(result.content, "中文");

    let encoded = codec
        .encode_fragment(&result.content, &result.metadata)
        .unwrap();
    assert_eq!(encoded, big5_bytes);
}

#[test]
fn ascii_passthrough() {
    let codec = euc_kr_codec();
    let result = codec.decode(b"Hello").unwrap();
    assert_eq!(result.content, "Hello");

    let encoded = codec.encode_fragment("Hello", &result.metadata).unwrap();
    assert_eq!(encoded, b"Hello");
}

#[test]
fn metadata_has_encoding() {
    let codec = euc_kr_codec();
    let result = codec.decode(b"Hello").unwrap();
    assert_eq!(result.metadata.get("encoding"), Some("EUC-KR"));
    assert_eq!(result.metadata.content_type().as_str(), "encoding/euc-kr");
}

#[test]
fn encoding_accessor() {
    let codec = euc_kr_codec();
    assert_eq!(codec.encoding().name(), "EUC-KR");
}

#[test]
fn empty_input() {
    let codec = euc_kr_codec();
    let result = codec.decode(b"").unwrap();
    assert_eq!(result.content, "");
}

#[test]
fn encode_unencodable_returns_error() {
    let codec = euc_kr_codec();
    // Emoji can't be encoded in EUC-KR
    let metadata = CodecMetadata::new(ContentType::new("encoding/euc-kr"));
    let result = codec.encode_fragment("Hello 🎉", &metadata);
    assert!(result.is_err());
}

#[test]
fn translate_edit_ascii_replacement() {
    let codec = euc_kr_codec();
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Text {
        start: Position::new(0, 1),
        end: Position::new(0, 2),
        replacement: "Z".to_string(),
    };

    let translated = codec
        .translate_edit(&bytes, &edit)
        .expect("ascii replacement accepted")
        .expect("produced a byte edit");

    assert_eq!(translated.offset, 1);
    assert_eq!(translated.old_bytes, b"b");
    assert_eq!(translated.new_bytes, b"Z");
}

#[test]
fn translate_edit_euc_kr_replacement() {
    let codec = euc_kr_codec();

    // "한글" in EUC-KR
    let raw = [0xC7, 0xD1, 0xB1, 0xDB];
    let bytes = HeapByteSource::new(raw);
    let edit = DecodedEdit::Text {
        start: Position::new(0, 1),
        end: Position::new(0, 2),
        replacement: "A".to_string(),
    };

    let translated = codec
        .translate_edit(&bytes, &edit)
        .expect("euc-kr replacement accepted")
        .expect("produced a byte edit");

    assert_eq!(translated.offset, 2);
    assert_eq!(translated.old_bytes, vec![0xB1, 0xDB]);
    assert_eq!(translated.new_bytes, b"A");
}

#[test]
fn translate_edit_bytes_variant_is_not_supported() {
    use reovim_driver_codec::TranslateEditError;
    let codec = euc_kr_codec();
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
