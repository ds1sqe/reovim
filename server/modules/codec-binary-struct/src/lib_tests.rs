//! Module-level tests for codec-binary-struct.

use {
    reovim_driver_codec::{ContentClassifier, ContentCodecFactory},
    reovim_kernel::api::v1::Module,
};

use super::*;

#[test]
fn module_id() {
    let m = CodecBinaryStructModule::new();
    assert_eq!(m.id().as_str(), "codec-binary-struct");
}

#[test]
fn module_name() {
    let m = CodecBinaryStructModule::new();
    assert_eq!(m.name(), "Codec Binary Struct");
}

#[test]
fn module_version() {
    let m = CodecBinaryStructModule::new();
    let v = m.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn module_provides_codec() {
    let m = CodecBinaryStructModule::new();
    assert!(m.provides().contains(&reovim_capabilities::CODEC_PROVIDER));
}

#[test]
fn elf_classifier_and_factory_integrate() {
    let classifier = ElfClassifier::new();
    let factory = BinaryStructCodecFactory::new();

    let ct = classifier
        .classify(b"\x7fELF\x02\x01\x01\x00", "test.bin")
        .unwrap();
    assert_eq!(ct.as_str(), crate::classifier::ELF);

    // Factory creates codec for ELF type
    let codec = factory.create(&ct).unwrap();
    let metadata = reovim_driver_codec::CodecMetadata::new(ct);
    assert!(codec.encode("text", &metadata).is_none());
}

#[test]
fn zip_classifier_and_factory_integrate() {
    let classifier = ZipClassifier::new();
    let factory = BinaryStructCodecFactory::new();

    let ct = classifier
        .classify(b"PK\x03\x04\x14\x00", "test.bin")
        .unwrap();
    assert_eq!(ct.as_str(), crate::classifier::ZIP);

    // Factory creates codec for ZIP type
    let codec = factory.create(&ct).unwrap();
    let metadata = reovim_driver_codec::CodecMetadata::new(ct);
    assert!(codec.encode("text", &metadata).is_none());
}

#[test]
fn text_not_classified() {
    let elf_c = ElfClassifier::new();
    let zip_c = ZipClassifier::new();

    assert!(elf_c.classify(b"hello world", "test.txt").is_none());
    assert!(zip_c.classify(b"hello world", "test.txt").is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecBinaryStructModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecBinaryStructModule;
    assert_eq!(m.id().as_str(), "codec-binary-struct");
}
