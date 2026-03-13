//! Module-level tests for codec-legacy.

use {
    reovim_driver_codec::{ContentClassifier, ContentCodecFactory},
    reovim_kernel::api::v1::Module,
};

use super::*;

#[test]
fn module_id() {
    let m = CodecLegacyModule::new();
    assert_eq!(m.id().as_str(), "codec-legacy");
}

#[test]
fn module_name() {
    let m = CodecLegacyModule::new();
    assert_eq!(m.name(), "Codec Legacy");
}

#[test]
fn module_version() {
    let m = CodecLegacyModule::new();
    let v = m.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn module_provides_codec() {
    let m = CodecLegacyModule::new();
    assert!(m.provides().contains(&reovim_capabilities::CODEC_PROVIDER));
}

#[test]
fn classifier_and_factory_integrate_latin1() {
    let classifier = LegacyClassifier::new();
    let factory = LegacyCodecFactory::new();

    // Latin-1 "café": 63 61 66 E9 (0xE9 is invalid UTF-8 alone)
    let latin1_bytes: &[u8] = &[0x63, 0x61, 0x66, 0xE9];
    let ct = classifier.classify(latin1_bytes, "test.txt").unwrap();
    assert_eq!(ct.as_str(), "encoding/latin-1");

    let codec = factory.create(&ct).unwrap();
    let result = codec.decode(latin1_bytes).unwrap();
    assert_eq!(result.content, "caf\u{e9}");
    assert!(!result.lossy);
}

#[test]
fn classifier_and_factory_integrate_cp1252() {
    let classifier = LegacyClassifier::new();
    let factory = LegacyCodecFactory::new();

    // CP1252 with smart quotes (0x93, 0x94)
    let cp1252_bytes: &[u8] = &[0x93, b'y', b'o', 0x94];
    let ct = classifier.classify(cp1252_bytes, "test.txt").unwrap();
    assert_eq!(ct.as_str(), "encoding/windows-1252");

    let codec = factory.create(&ct).unwrap();
    let result = codec.decode(cp1252_bytes).unwrap();
    assert_eq!(result.content, "\u{201c}yo\u{201d}");
}

#[test]
fn utf8_not_classified() {
    let classifier = LegacyClassifier::new();
    assert!(classifier.classify("café".as_bytes(), "test.txt").is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecLegacyModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecLegacyModule;
    assert_eq!(m.id().as_str(), "codec-legacy");
}
