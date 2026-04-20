//! Tests for `TextContentCodecModule` lifecycle.
//!
//! Covers init → retained registry, exit → registry released, exit
//! without init (no-op), and double-exit safety. The design rule
//! "codec lifetime = module lifetime" is verified symmetrically even
//! though this module does not currently register a codec — the
//! lifecycle contract is the same shape that the sibling format
//! modules will follow in Phase C.5.

use {
    super::TextContentCodecModule,
    reovim_kernel::api::v1::{Module, ModuleContext},
};

fn test_ctx() -> ModuleContext {
    ModuleContext::default()
}

#[test]
fn default_has_no_registry_handle() {
    let module = TextContentCodecModule::default();
    assert!(module.registry.is_none());
}

#[test]
fn new_has_no_registry_handle() {
    let module = TextContentCodecModule::new();
    assert!(module.registry.is_none());
}

#[test]
fn init_acquires_registry_handle() {
    let ctx = test_ctx();
    let mut module = TextContentCodecModule::new();
    assert!(matches!(
        module.init(&ctx),
        reovim_kernel::api::v1::ProbeResult::Success
    ));
    assert!(module.registry.is_some());
}

#[test]
fn exit_releases_registry_handle() {
    let ctx = test_ctx();
    let mut module = TextContentCodecModule::new();
    let _ = module.init(&ctx);
    assert!(module.registry.is_some());
    module.exit().expect("exit should succeed");
    assert!(module.registry.is_none());
}

#[test]
fn exit_without_init_is_noop() {
    let mut module = TextContentCodecModule::new();
    // Never init'd, so registry is None.
    module.exit().expect("exit should succeed even without init");
    assert!(module.registry.is_none());
}

#[test]
fn double_exit_is_safe() {
    let ctx = test_ctx();
    let mut module = TextContentCodecModule::new();
    let _ = module.init(&ctx);
    module.exit().expect("first exit should succeed");
    module.exit().expect("second exit should succeed (idempotent)");
    assert!(module.registry.is_none());
}

#[test]
fn module_identity_and_version() {
    let module = TextContentCodecModule::new();
    assert_eq!(module.id().as_str(), "reovim-content-codec-text");
    assert_eq!(module.name(), "Text Content Codec");
    let v = module.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 15);
    assert_eq!(v.patch, 0);
}
