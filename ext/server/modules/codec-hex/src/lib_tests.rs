//! Module-level tests for codec-hex.

use {
    reovim_driver_codec::{ContentClassifier, ContentCodecFactory, ContentType},
    reovim_kernel::api::v1::Module,
};

use super::*;

#[test]
fn module_id() {
    let m = CodecHexModule::new();
    assert_eq!(m.id().as_str(), "codec-hex");
}

#[test]
fn module_name() {
    let m = CodecHexModule::new();
    assert_eq!(m.name(), "Codec Hex");
}

#[test]
fn module_version() {
    let m = CodecHexModule::new();
    let v = m.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn module_provides_codec() {
    let m = CodecHexModule::new();
    assert!(m.provides().contains(&reovim_capabilities::CODEC_PROVIDER));
}

#[test]
fn classifier_and_factory_integrate() {
    let classifier = BinaryClassifier::new();
    let factory = HexCodecFactory::new();

    // Classify binary content (null byte)
    let ct = classifier.classify(b"hello\x00world", "test.bin").unwrap();
    assert_eq!(ct.as_str(), ContentType::BINARY_RAW);

    // Factory creates codec for that content type
    let codec = factory.create(&ct).unwrap();
    let result = codec.decode(b"hello\x00world").unwrap();
    assert!(result.content.contains("68 65 6c 6c 6f 00"));
    assert!(result.readonly);
    assert!(result.lossy);
}

#[test]
fn classifier_passes_utf8_factory_rejects_utf8() {
    let classifier = BinaryClassifier::new();
    let factory = HexCodecFactory::new();

    // Valid UTF-8 not classified as binary
    assert!(classifier.classify(b"hello world", "test.txt").is_none());

    // UTF-8 content type rejected by hex factory
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecHexModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecHexModule;
    assert_eq!(m.id().as_str(), "codec-hex");
}
