//! Tests for CJK encoding codec.

use reovim_driver_codec::ContentCodec;

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
        .encode(&result.content, &result.metadata)
        .unwrap()
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
        .encode(&result.content, &result.metadata)
        .unwrap()
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
        .encode(&result.content, &result.metadata)
        .unwrap()
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
        .encode(&result.content, &result.metadata)
        .unwrap()
        .unwrap();
    assert_eq!(encoded, big5_bytes);
}

#[test]
fn ascii_passthrough() {
    let codec = euc_kr_codec();
    let result = codec.decode(b"Hello").unwrap();
    assert_eq!(result.content, "Hello");

    let encoded = codec.encode("Hello", &result.metadata).unwrap().unwrap();
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
    let result = codec.encode("Hello 🎉", &metadata).unwrap();
    assert!(result.is_err());
}
