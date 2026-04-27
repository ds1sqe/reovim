//! Module-level tests for codec-binary-struct.

use {
    reovim_content_codec::{ContentClassifier, ContentCodecFactory, ContentType},
    reovim_kernel::api::v1::{Module, ModuleContext},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
};

use super::*;

fn test_ctx() -> ModuleContext {
    ModuleContext::default()
}

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
    assert!(
        m.provides()
            .contains(&reovim_subsys_content_codec::capabilities::CODEC_PROVIDER)
    );
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
    let _codec = factory.create(&ct).unwrap();
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
    let _codec = factory.create(&ct).unwrap();
}

#[test]
fn text_not_classified() {
    let elf_c = ElfClassifier::new();
    let zip_c = ZipClassifier::new();

    assert!(elf_c.classify(b"hello world", "test.txt").is_none());
    assert!(zip_c.classify(b"hello world", "test.txt").is_none());
}

#[test]
fn default_has_no_registry_handle() {
    let m = CodecBinaryStructModule::default();
    assert!(m.registry.is_none());
    assert!(m.content_types.is_empty());
}

#[test]
fn init_acquires_registry_and_registers_both_codecs() {
    let ctx = test_ctx();
    let mut m = CodecBinaryStructModule::new();
    assert!(matches!(m.init(&ctx), reovim_kernel::api::v1::ProbeResult::Success));
    assert!(m.registry.is_some());
    assert_eq!(m.content_types.len(), 2);

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    assert!(
        registry
            .get(&ContentType::new(crate::classifier::ELF))
            .is_some()
    );
    assert!(
        registry
            .get(&ContentType::new(crate::classifier::ZIP))
            .is_some()
    );
}

#[test]
fn exit_releases_registry_and_unregisters_both_codecs() {
    let ctx = test_ctx();
    let mut m = CodecBinaryStructModule::new();
    let _ = m.init(&ctx);

    m.exit().expect("exit should succeed");
    assert!(m.registry.is_none());
    assert!(m.content_types.is_empty());

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    assert!(
        registry
            .get(&ContentType::new(crate::classifier::ELF))
            .is_none()
    );
    assert!(
        registry
            .get(&ContentType::new(crate::classifier::ZIP))
            .is_none()
    );
}

#[test]
fn exit_without_init_is_noop() {
    let mut m = CodecBinaryStructModule::new();
    m.exit().expect("exit should succeed even without init");
    assert!(m.registry.is_none());
    assert!(m.content_types.is_empty());
}

#[test]
fn double_exit_is_safe() {
    let ctx = test_ctx();
    let mut m = CodecBinaryStructModule::new();
    let _ = m.init(&ctx);
    m.exit().expect("first exit should succeed");
    m.exit().expect("second exit should succeed (idempotent)");
    assert!(m.registry.is_none());
    assert!(m.content_types.is_empty());
}

#[test]
fn module_exit() {
    let mut m = CodecBinaryStructModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecBinaryStructModule::default();
    assert_eq!(m.id().as_str(), "codec-binary-struct");
}
