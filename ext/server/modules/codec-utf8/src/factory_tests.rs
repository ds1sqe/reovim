//! Tests for UTF-8 codec factory.

use reovim_driver_codec::{ContentCodecFactory, ContentType};

use super::Utf8CodecFactory;

#[test]
fn creates_utf8_codec() {
    let factory = Utf8CodecFactory::new();
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn rejects_non_utf8() {
    let factory = Utf8CodecFactory::new();
    let ct = ContentType::new("binary/raw");
    assert!(factory.create(&ct).is_none());
}

#[test]
fn supported_content_types() {
    let factory = Utf8CodecFactory::new();
    assert_eq!(factory.supported_content_types(), vec!["text/utf-8"]);
}

#[test]
fn name() {
    let factory = Utf8CodecFactory::new();
    assert_eq!(factory.name(), "utf-8");
}

#[test]
fn created_codec_works() {
    let factory = Utf8CodecFactory::new();
    let ct = ContentType::new(ContentType::UTF8);
    let codec = factory.create(&ct).unwrap();
    let result = codec.decode(b"hello").unwrap();
    assert_eq!(result.content, "hello");
}
