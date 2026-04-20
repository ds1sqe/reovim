//! Tests for binary struct codec factory.

use reovim_driver_codec::{ContentCodecFactory, ContentType};

use super::BinaryStructCodecFactory;

#[test]
fn creates_elf_codec() {
    let factory = BinaryStructCodecFactory::new();
    let ct = ContentType::new(crate::classifier::ELF);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn creates_zip_codec() {
    let factory = BinaryStructCodecFactory::new();
    let ct = ContentType::new(crate::classifier::ZIP);
    assert!(factory.create(&ct).is_some());
}

#[test]
fn rejects_binary_raw() {
    let factory = BinaryStructCodecFactory::new();
    let ct = ContentType::new(ContentType::BINARY_RAW);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_utf8() {
    let factory = BinaryStructCodecFactory::new();
    let ct = ContentType::new(ContentType::UTF8);
    assert!(factory.create(&ct).is_none());
}

#[test]
fn rejects_unknown() {
    let factory = BinaryStructCodecFactory::new();
    let ct = ContentType::new("application/json");
    assert!(factory.create(&ct).is_none());
}

#[test]
fn supported_content_types() {
    let factory = BinaryStructCodecFactory::new();
    let types = factory.supported_content_types();
    assert!(types.contains(&crate::classifier::ELF));
    assert!(types.contains(&crate::classifier::ZIP));
    assert_eq!(types.len(), 2);
}

#[test]
fn name() {
    let factory = BinaryStructCodecFactory::new();
    assert_eq!(factory.name(), "binary-struct");
}

#[test]
fn default_impl() {
    let factory = BinaryStructCodecFactory;
    assert_eq!(factory.name(), "binary-struct");
}
