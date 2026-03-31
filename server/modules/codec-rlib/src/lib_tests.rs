//! Module-level tests for codec-rlib.

use {
    reovim_driver_codec::{ContentClassifier, ContentCodecFactory},
    reovim_kernel::api::v1::Module,
};

use super::*;

#[test]
fn module_id() {
    let m = CodecRlibModule::new();
    assert_eq!(m.id().as_str(), "codec-rlib");
}

#[test]
fn module_name() {
    let m = CodecRlibModule::new();
    assert_eq!(m.name(), "Codec Rlib");
}

#[test]
fn module_version() {
    let m = CodecRlibModule::new();
    let v = m.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn module_provides_codec() {
    let m = CodecRlibModule::new();
    assert!(m.provides().contains(&reovim_capabilities::CODEC_PROVIDER));
}

#[test]
fn classifier_and_factory_integrate() {
    let classifier = RlibClassifier::new();
    let factory = RlibCodecFactory::new();

    let ct = classifier
        .classify(b"!<arch>\n", "libfoo-abc123.rlib")
        .unwrap();
    assert_eq!(ct.as_str(), crate::classifier::RLIB);

    // Factory creates codec for rlib type
    let codec = factory.create(&ct).unwrap();
    let metadata = reovim_driver_codec::CodecMetadata::new(ct);
    assert!(codec.encode("text", &metadata).is_none());
}

#[test]
fn text_not_classified() {
    let c = RlibClassifier::new();
    assert!(c.classify(b"hello world", "test.txt").is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecRlibModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecRlibModule;
    assert_eq!(m.id().as_str(), "codec-rlib");
}
