use {
    super::*,
    reovim_driver_display::GutterRenderer,
    reovim_kernel::api::v1::{Module, ModuleContext, ProbeResult},
    std::sync::Arc,
};

#[test]
fn test_module_id() {
    let module = GitSignsModule::new();
    assert_eq!(module.id().as_str(), "git-signs");
}

#[test]
fn test_module_name() {
    let module = GitSignsModule::new();
    assert_eq!(module.name(), "Git Signs");
}

#[test]
fn test_module_version() {
    let module = GitSignsModule::new();
    let v = module.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn test_module_default() {
    let m1 = GitSignsModule::new();
    let m2 = GitSignsModule;
    assert_eq!(m1.id(), m2.id());
}

#[test]
fn test_exit_succeeds() {
    let mut module = GitSignsModule::new();
    assert!(module.exit().is_ok());
}

// ========================================================================
// init() coverage
// ========================================================================

/// No default renderer registered → nothing registered, returns Success.
#[test]
fn test_init_no_default_renderer() {
    let mut module = GitSignsModule::new();
    let ctx = ModuleContext::default();
    assert_eq!(module.init(&ctx), ProbeResult::Success);
}

/// Default renderer registered → registers source and presenter.
#[test]
fn test_init_with_default_renderer() {
    let mut module = GitSignsModule::new();
    let ctx = ModuleContext::default();

    let registry = ctx.services.get_or_create::<GutterRendererRegistry>();
    registry.register(GutterRendererKey::Default, Arc::new(GutterRenderer::new()));

    assert_eq!(module.init(&ctx), ProbeResult::Success);

    let renderer = registry
        .get(&GutterRendererKey::Default)
        .expect("default renderer");
    assert_eq!(renderer.source_count(), 1, "should register one source");
    assert_eq!(renderer.presenter_count(), 1, "should register one presenter");
}
