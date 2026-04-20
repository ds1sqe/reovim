//! Tests for `XxdContentCodecModule` lifecycle.
//!
//! Covers init → registry acquired + codec registered, exit → registry
//! released + codec unregistered, exit without init (no-op), double-exit
//! safety, and identity/version checks.

use {
    super::XxdContentCodecModule,
    crate::module::XXD_CONTENT_TYPE,
    reovim_content_codec::ContentType,
    reovim_kernel::api::v1::{Module, ModuleContext},
    reovim_subsys_content_codec::{ContentCodecRegistry, DefaultContentCodecRegistry},
};

fn test_ctx() -> ModuleContext {
    ModuleContext::default()
}

#[test]
fn default_has_no_registry_handle() {
    let module = XxdContentCodecModule::default();
    assert!(module.registry.is_none());
    assert!(module.content_type.is_none());
}

#[test]
fn new_has_no_registry_handle() {
    let module = XxdContentCodecModule::new();
    assert!(module.registry.is_none());
    assert!(module.content_type.is_none());
}

#[test]
fn init_acquires_registry_and_registers_codec() {
    let ctx = test_ctx();
    let mut module = XxdContentCodecModule::new();
    assert!(matches!(
        module.init(&ctx),
        reovim_kernel::api::v1::ProbeResult::Success
    ));
    assert!(module.registry.is_some());
    assert!(module.content_type.is_some());

    // Codec must be visible in the registry after init.
    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    let ct = ContentType::new(XXD_CONTENT_TYPE);
    assert!(registry.get(&ct).is_some());
}

#[test]
fn exit_releases_registry_and_unregisters_codec() {
    let ctx = test_ctx();
    let mut module = XxdContentCodecModule::new();
    let _ = module.init(&ctx);
    assert!(module.registry.is_some());

    module.exit().expect("exit should succeed");
    assert!(module.registry.is_none());
    assert!(module.content_type.is_none());

    // Codec must be removed from the registry after exit.
    let registry = ctx.services.get_or_create::<DefaultContentCodecRegistry>();
    let ct = ContentType::new(XXD_CONTENT_TYPE);
    assert!(registry.get(&ct).is_none());
}

#[test]
fn exit_without_init_is_noop() {
    let mut module = XxdContentCodecModule::new();
    module.exit().expect("exit should succeed even without init");
    assert!(module.registry.is_none());
    assert!(module.content_type.is_none());
}

#[test]
fn double_exit_is_safe() {
    let ctx = test_ctx();
    let mut module = XxdContentCodecModule::new();
    let _ = module.init(&ctx);
    module.exit().expect("first exit should succeed");
    module.exit().expect("second exit should succeed (idempotent)");
    assert!(module.registry.is_none());
    assert!(module.content_type.is_none());
}

#[test]
fn module_identity_and_version() {
    let module = XxdContentCodecModule::new();
    assert_eq!(module.id().as_str(), "reovim-content-codec-xxd");
    assert_eq!(module.name(), "Xxd Content Codec");
    let v = module.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 15);
    assert_eq!(v.patch, 0);
}
