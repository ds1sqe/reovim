use {
    super::*,
    reovim_kernel::api::v1::{KernelContext, Module},
    std::sync::Arc,
};

#[test]
fn test_editor_module_new() {
    let module = EditorModule::new();
    assert_eq!(module.name(), "Editor");
}

#[test]
fn test_editor_module_default() {
    let module = EditorModule;
    assert_eq!(module.name(), "Editor");
}

#[test]
fn test_editor_module_default_trait() {
    let module = <EditorModule as Default>::default();
    assert_eq!(module.name(), "Editor");
    assert_eq!(module.id().as_str(), "editor");
}

#[test]
fn test_editor_module_id() {
    let module = EditorModule::new();
    assert_eq!(module.id().as_str(), "editor");
}

#[test]
fn test_editor_module_version() {
    let module = EditorModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_editor_module_exit() {
    let mut module = EditorModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_editor_module_constant() {
    assert_eq!(EDITOR_MODULE.as_str(), "editor");
}

#[test]
fn test_editor_module_command_provider() {
    let module = EditorModule::new();
    let handlers = module.command_handlers();
    assert!(!handlers.is_empty());
    // Should match all_commands() count
    assert_eq!(handlers.len(), command::all_commands().len());
}

#[test]
fn test_editor_module_init() {
    use reovim_kernel::api::ServiceRegistry;

    let mut module = EditorModule::new();
    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services,
        std::path::PathBuf::from("/tmp/test-data"),
        std::path::PathBuf::from("/tmp/test-cache"),
    );
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);
}

// ========================================================================
// Epic #570: Editor options (#573)
// ========================================================================

#[test]
fn test_editor_option_specs_count() {
    let specs = editor_option_specs();
    assert_eq!(specs.len(), 5);
}

#[test]
fn test_editor_options_registered_after_init() {
    let mut module = EditorModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let expected = [
        "tabstop",
        "shiftwidth",
        "expandtab",
        "autoindent",
        "textwidth",
    ];
    for name in &expected {
        assert!(ctx.kernel.options.contains(name), "'{name}' should be registered");
    }
}

#[test]
fn test_editor_options_aliases() {
    let mut module = EditorModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let aliases = [
        ("ts", "tabstop"),
        ("sw", "shiftwidth"),
        ("et", "expandtab"),
        ("ai", "autoindent"),
        ("tw", "textwidth"),
    ];
    for (short, full) in &aliases {
        assert_eq!(
            ctx.kernel.options.resolve_name(short),
            Some(full.to_string()),
            "'{short}' should resolve to '{full}'"
        );
    }
}

#[test]
fn test_editor_options_defaults() {
    let mut module = EditorModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    assert_eq!(ctx.kernel.options.get_global("tabstop"), Some(OptionValue::int(4)));
    assert_eq!(ctx.kernel.options.get_global("shiftwidth"), Some(OptionValue::int(4)));
    assert_eq!(ctx.kernel.options.get_global("expandtab"), Some(OptionValue::bool(true)));
    assert_eq!(ctx.kernel.options.get_global("autoindent"), Some(OptionValue::bool(true)));
    assert_eq!(ctx.kernel.options.get_global("textwidth"), Some(OptionValue::int(0)));
}

#[test]
fn test_editor_options_ownership() {
    let mut module = EditorModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let editor_options = ctx.kernel.options.list_by_module(&EDITOR_MODULE);
    assert_eq!(editor_options.len(), 5);
}

#[test]
fn test_editor_options_scopes() {
    let specs = editor_option_specs();
    for spec in &specs {
        assert_eq!(
            spec.scope,
            OptionScope::Buffer,
            "'{name}' should have Buffer scope",
            name = spec.name
        );
    }
}

#[test]
fn test_editor_init_fails_on_duplicate_option() {
    let ctx = ModuleContext::default();

    // Pre-register one of our options to trigger a conflict
    let _ = ctx.kernel.options.register(OptionSpec::new(
        "tabstop",
        "Already taken",
        OptionValue::int(1),
    ));

    let mut module = EditorModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Failed(_)), "init should fail on duplicate option");
}
