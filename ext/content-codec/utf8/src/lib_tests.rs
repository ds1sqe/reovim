//! Module-level tests for codec-utf8.

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
fn default_has_no_registry_handle() {
    let m = CodecUtf8Module::default();
    assert!(m.registry.is_none());
    assert!(m.content_type.is_none());
}

#[test]
fn init_acquires_registry_and_registers_codec() {
    let ctx = test_ctx();
    let mut m = CodecUtf8Module::new();
    assert!(matches!(
        m.init(&ctx),
        reovim_kernel::api::v1::ProbeResult::Success
    ));
    assert!(m.registry.is_some());
    assert!(m.content_type.is_some());

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    let ct = ContentType::new(ContentType::UTF8);
    assert!(registry.get(&ct).is_some());
}

#[test]
fn exit_releases_registry_and_unregisters_codec() {
    let ctx = test_ctx();
    let mut m = CodecUtf8Module::new();
    let _ = m.init(&ctx);

    m.exit().expect("exit should succeed");
    assert!(m.registry.is_none());
    assert!(m.content_type.is_none());

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    let ct = ContentType::new(ContentType::UTF8);
    assert!(registry.get(&ct).is_none());
}

#[test]
fn exit_without_init_is_noop() {
    let mut m = CodecUtf8Module::new();
    m.exit().expect("exit should succeed even without init");
    assert!(m.registry.is_none());
}

#[test]
fn double_exit_is_safe() {
    let ctx = test_ctx();
    let mut m = CodecUtf8Module::new();
    let _ = m.init(&ctx);
    m.exit().expect("first exit should succeed");
    m.exit().expect("second exit should succeed (idempotent)");
    assert!(m.registry.is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecUtf8Module::new();
    assert!(m.exit().is_ok());
}
