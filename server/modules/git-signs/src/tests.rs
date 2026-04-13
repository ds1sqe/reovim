use {
    super::*,
    reovim_kernel::api::v1::{Module, ModuleContext, ProbeResult},
    reovim_subsys_annotation::AnnotationSourceRegistry,
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
    assert_eq!(v.minor, 2);
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

/// Registers annotation source in `AnnotationSourceRegistry`.
#[test]
fn test_init_registers_source() {
    let mut module = GitSignsModule::new();
    let ctx = ModuleContext::default();

    assert_eq!(module.init(&ctx), ProbeResult::Success);

    let registry = ctx
        .services
        .get::<AnnotationSourceRegistry>()
        .expect("AnnotationSourceRegistry should be created");
    assert_eq!(registry.len(), 1, "should register one source");
}

#[test]
fn test_keybindings_count() {
    let module = GitSignsModule::new();
    let bindings = module.keybindings();
    // 2 navigation + 5 operations + 2 preview
    assert_eq!(bindings.len(), 9);
}

#[test]
fn test_keybindings_have_descriptions() {
    let module = GitSignsModule::new();
    for binding in module.keybindings() {
        assert!(!binding.description.is_empty(), "Keybinding missing description");
    }
}
