//! Tests for hex codec factory.

use reovim_driver_codec::{ContentCodecFactory, ContentType};

use super::HexCodecFactory;

#[test]
fn creates_hex_codec_for_binary_raw() {
    let factory = HexCodecFactory::new();
    let ct = ContentType::new(ContentType::BINARY_RAW);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn rejects_non_binary() {
    let factory = HexCodecFactory::new();
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_unknown_type() {
    let factory = HexCodecFactory::new();
    let ct = ContentType::new("application/json");
    assert!(factory.create(&ct).is_none());
}

#[test]
fn supported_content_types() {
    let factory = HexCodecFactory::new();
    assert_eq!(factory.supported_content_types(), vec![ContentType::BINARY_RAW]);
}

#[test]
fn name() {
    let factory = HexCodecFactory::new();
    assert_eq!(factory.name(), "hex");
}

#[test]
fn created_codec_decodes() {
    let factory = HexCodecFactory::new();
    let ct = ContentType::new(ContentType::BINARY_RAW);
    let codec = factory.create(&ct).unwrap();
    let result = codec.decode(b"AB").unwrap();
    assert!(result.content.contains("41 42"));
    assert!(result.readonly);
    assert!(result.lossy);
}

#[test]
fn default_impl() {
    let factory = HexCodecFactory;
    assert_eq!(factory.name(), "hex");
}
