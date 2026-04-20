//! Module-level tests for codec-utf8.

use {
    reovim_driver_codec::{ContentClassifier, ContentCodecFactory, ContentType},
    reovim_kernel::api::v1::Module,
};

use super::*;

#[test]
fn module_id() {
    let m = CodecUtf8Module::new();
    assert_eq!(m.id().as_str(), "codec-utf8");
}

#[test]
fn module_name() {
    let m = CodecUtf8Module::new();
    assert_eq!(m.name(), "Codec UTF-8");
}

#[test]
fn module_provides_codec() {
    let m = CodecUtf8Module::new();
    assert!(m.provides().contains(&reovim_capabilities::CODEC_PROVIDER));
}

#[test]
fn classifier_and_factory_integrate() {
    let classifier = Utf8Classifier::new();
    let factory = Utf8CodecFactory::new();

    // Classify valid UTF-8
    let ct = classifier.classify(b"hello world", "test.txt").unwrap();
    assert_eq!(ct.as_str(), ContentType::UTF8);

    // Factory creates codec for that content type
    let codec = factory.create(&ct).unwrap();
    let result = codec.decode(b"hello world").unwrap();
    assert_eq!(result.content, "hello world");
}

#[test]
fn classifier_rejects_binary_factory_rejects_unknown() {
    let classifier = Utf8Classifier::new();
    let factory = Utf8CodecFactory::new();

    // Binary content fails classification
    assert!(classifier.classify(&[0xFF, 0x00], "test.bin").is_none());

    // Unknown content type fails factory
    let ct = ContentType::new("binary/raw");
    assert!(factory.create(&ct).is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecUtf8Module::new();
    assert!(m.exit().is_ok());
}
