//! Module-level tests for codec-cjk.

use {
    reovim_driver_codec::{ContentClassifier, ContentCodecFactory, ContentType},
    reovim_kernel::api::v1::Module,
};

use super::*;

#[test]
fn module_id() {
    let m = CodecCjkModule::new();
    assert_eq!(m.id().as_str(), "codec-cjk");
}

#[test]
fn module_name() {
    let m = CodecCjkModule::new();
    assert_eq!(m.name(), "Codec CJK");
}

#[test]
fn module_version() {
    let m = CodecCjkModule::new();
    let v = m.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn module_provides_codec() {
    let m = CodecCjkModule::new();
    assert!(m.provides().contains(&reovim_capabilities::CODEC_PROVIDER));
}

#[test]
fn classifier_and_factory_integrate() {
    let classifier = CjkClassifier::new();
    let factory = CjkCodecFactory::new();

    // EUC-KR encoded "한글"
    let euc_kr_bytes: &[u8] = &[0xC7, 0xD1, 0xB1, 0xDB];
    let ct = classifier.classify(euc_kr_bytes, "test.txt");
    assert!(ct.is_some());

    let ct = ct.unwrap();
    let codec = factory.create(&ct).unwrap();
    let result = codec.decode(euc_kr_bytes).unwrap();
    assert_eq!(result.content, "한글");
    assert!(!result.lossy);
}

#[test]
fn utf8_not_classified() {
    let classifier = CjkClassifier::new();
    let factory = CjkCodecFactory::new();

    // Valid UTF-8 should not be classified as CJK
    assert!(classifier.classify("한글".as_bytes(), "test.txt").is_none());

    // UTF-8 content type rejected by factory
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecCjkModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecCjkModule;
    assert_eq!(m.id().as_str(), "codec-cjk");
}
