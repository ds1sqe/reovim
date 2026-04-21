//! Module-level tests for codec-rlib.

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
    let _codec = factory.create(&ct).unwrap();
}

#[test]
fn text_not_classified() {
    let c = RlibClassifier::new();
    assert!(c.classify(b"hello world", "test.txt").is_none());
}

#[test]
fn default_has_no_registry_handle() {
    let m = CodecRlibModule::default();
    assert!(m.registry.is_none());
    assert!(m.content_type.is_none());
}

#[test]
fn init_acquires_registry_and_registers_codec() {
    let ctx = test_ctx();
    let mut m = CodecRlibModule::new();
    assert!(matches!(m.init(&ctx), reovim_kernel::api::v1::ProbeResult::Success));
    assert!(m.registry.is_some());
    assert!(m.content_type.is_some());

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    let ct = ContentType::new(crate::classifier::RLIB);
    assert!(registry.get(&ct).is_some());
}

#[test]
fn exit_releases_registry_and_unregisters_codec() {
    let ctx = test_ctx();
    let mut m = CodecRlibModule::new();
    let _ = m.init(&ctx);

    m.exit().expect("exit should succeed");
    assert!(m.registry.is_none());
    assert!(m.content_type.is_none());

    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    let ct = ContentType::new(crate::classifier::RLIB);
    assert!(registry.get(&ct).is_none());
}

#[test]
fn exit_without_init_is_noop() {
    let mut m = CodecRlibModule::new();
    m.exit().expect("exit should succeed even without init");
    assert!(m.registry.is_none());
}

#[test]
fn double_exit_is_safe() {
    let ctx = test_ctx();
    let mut m = CodecRlibModule::new();
    let _ = m.init(&ctx);
    m.exit().expect("first exit should succeed");
    m.exit().expect("second exit should succeed (idempotent)");
    assert!(m.registry.is_none());
}

#[test]
fn module_exit() {
    let mut m = CodecRlibModule::new();
    assert!(m.exit().is_ok());
}

#[test]
fn default_impl() {
    let m = CodecRlibModule::default();
    assert_eq!(m.id().as_str(), "codec-rlib");
}
