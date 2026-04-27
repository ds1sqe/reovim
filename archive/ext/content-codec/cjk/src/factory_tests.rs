//! Tests for CJK codec factory.

use reovim_content_codec::{ContentCodecFactory, ContentType};

use super::CjkCodecFactory;

#[test]
fn creates_euc_kr_codec() {
    let factory = CjkCodecFactory::new();
    let ct = ContentType::new("encoding/euc-kr");
    assert!(factory.create(&ct).is_some());
}

#[test]
fn creates_shift_jis_codec() {
    let factory = CjkCodecFactory::new();
    let ct = ContentType::new("encoding/shift-jis");
    assert!(factory.create(&ct).is_some());
}

#[test]
fn creates_gbk_codec() {
    let factory = CjkCodecFactory::new();
    let ct = ContentType::new("encoding/gbk");
    assert!(factory.create(&ct).is_some());
}

#[test]
fn creates_big5_codec() {
    let factory = CjkCodecFactory::new();
    let ct = ContentType::new("encoding/big5");
    assert!(factory.create(&ct).is_some());
}

#[test]
fn rejects_utf8() {
    let factory = CjkCodecFactory::new();
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_binary() {
    let factory = CjkCodecFactory::new();
    let ct = ContentType::new(ContentType::BINARY_RAW);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn supported_content_types() {
    let factory = CjkCodecFactory::new();
    let types = factory.supported_content_types();
    assert_eq!(types.len(), 4);
    assert!(types.contains(&"encoding/euc-kr"));
    assert!(types.contains(&"encoding/shift-jis"));
    assert!(types.contains(&"encoding/gbk"));
    assert!(types.contains(&"encoding/big5"));
}

#[test]
fn name() {
    let factory = CjkCodecFactory::new();
    assert_eq!(factory.name(), "cjk");
}

#[test]
fn created_codec_decodes() {
    let factory = CjkCodecFactory::new();
    let ct = ContentType::new("encoding/euc-kr");
    let codec = factory.create(&ct).unwrap();
    // "한글" in EUC-KR
    let euc_kr_bytes: &[u8] = &[0xC7, 0xD1, 0xB1, 0xDB];
    let result = codec.decode(euc_kr_bytes).unwrap();
    assert_eq!(result.content, "한글");
    assert!(!result.lossy);
    assert!(!result.readonly);
}

#[test]
fn default_impl() {
    let factory = CjkCodecFactory;
    assert_eq!(factory.name(), "cjk");
}
