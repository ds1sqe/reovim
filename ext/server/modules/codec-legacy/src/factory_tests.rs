//! Tests for legacy codec factory.

use reovim_driver_codec::{ContentCodecFactory, ContentType};

use super::LegacyCodecFactory;

#[test]
fn creates_latin1_codec() {
    let factory = LegacyCodecFactory::new();
    let ct = ContentType::new("encoding/latin-1");
    assert!(factory.create(&ct).is_some());
}

#[test]
fn creates_cp1252_codec() {
    let factory = LegacyCodecFactory::new();
    let ct = ContentType::new("encoding/windows-1252");
    assert!(factory.create(&ct).is_some());
}

#[test]
fn rejects_utf8() {
    let factory = LegacyCodecFactory::new();
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_binary() {
    let factory = LegacyCodecFactory::new();
    let ct = ContentType::new(ContentType::BINARY_RAW);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_cjk() {
    let factory = LegacyCodecFactory::new();
    let ct = ContentType::new("encoding/euc-kr");
    assert!(factory.create(&ct).is_none());
}

#[test]
fn supported_content_types() {
    let factory = LegacyCodecFactory::new();
    let types = factory.supported_content_types();
    assert_eq!(types.len(), 2);
    assert!(types.contains(&"encoding/latin-1"));
    assert!(types.contains(&"encoding/windows-1252"));
}

#[test]
fn name() {
    let factory = LegacyCodecFactory::new();
    assert_eq!(factory.name(), "legacy");
}

#[test]
fn created_latin1_codec_decodes() {
    let factory = LegacyCodecFactory::new();
    let ct = ContentType::new("encoding/latin-1");
    let codec = factory.create(&ct).unwrap();
    // "café" in Latin-1
    let bytes: &[u8] = &[0x63, 0x61, 0x66, 0xE9];
    let result = codec.decode(bytes).unwrap();
    assert_eq!(result.content, "caf\u{e9}");
}

#[test]
fn created_cp1252_codec_decodes() {
    let factory = LegacyCodecFactory::new();
    let ct = ContentType::new("encoding/windows-1252");
    let codec = factory.create(&ct).unwrap();
    // Smart quotes in CP1252
    let bytes: &[u8] = &[0x93, b'y', b'o', 0x94];
    let result = codec.decode(bytes).unwrap();
    assert_eq!(result.content, "\u{201c}yo\u{201d}");
}

#[test]
fn default_impl() {
    let factory = LegacyCodecFactory;
    assert_eq!(factory.name(), "legacy");
}
