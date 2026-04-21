//! Module-level tests for codec-hex.

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
fn default_has_no_registry_handle() {
    let m = CodecHexModule::default();
    assert!(m.registry.is_none());
    assert!(m.content_type.is_none());
}

#[test]
fn init_acquires_registry_and_registers_codec() {
    let ctx = test_ctx();
    let mut m = CodecHexModule::new();
    assert!(matches!(m.init(&ctx), reovim_kernel::api::v1::ProbeResult::Success));
    assert!(m.registry.is_some());
    assert!(m.content_type.is_some());

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    let ct = ContentType::new(ContentType::BINARY_RAW);
    assert!(registry.get(&ct).is_some());
}

#[test]
fn exit_releases_registry_and_unregisters_codec() {
    let ctx = test_ctx();
    let mut m = CodecHexModule::new();
    let _ = m.init(&ctx);

    m.exit().expect("exit should succeed");
    assert!(m.registry.is_none());
    assert!(m.content_type.is_none());

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    let ct = ContentType::new(ContentType::BINARY_RAW);
    assert!(registry.get(&ct).is_none());
}

#[test]
fn exit_without_init_is_noop() {
    let mut m = CodecHexModule::new();
    m.exit().expect("exit should succeed even without init");
    assert!(m.registry.is_none());
}

#[test]
fn double_exit_is_safe() {
    let ctx = test_ctx();
    let mut m = CodecHexModule::new();
    let _ = m.init(&ctx);
    m.exit().expect("first exit should succeed");
    m.exit().expect("second exit should succeed (idempotent)");
    assert!(m.registry.is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecHexModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecHexModule::default();
    assert_eq!(m.id().as_str(), "codec-hex");
}
