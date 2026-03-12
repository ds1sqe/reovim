use reovim_driver_module_config::ModulesConfig;

use super::*;

#[test]
fn test_create_session_state() {
    // This test verifies that module bootstrap doesn't panic
    let state = create_session_state();

    // #491: Use home_mode() instead of removed current_mode()
    let mode = state.home_mode();
    assert!(mode.name().contains("normal"), "Expected normal mode, got {}", mode.name());
}

#[test]
fn test_modules_register_services() {
    use reovim_driver_input::ResolverRegistry;

    let services = Arc::new(ServiceRegistry::new());
    let kernel = create_kernel_context(Arc::clone(&services));
    let ctx = create_module_context(kernel, Arc::clone(&services));

    let _tracked = initialize_modules(&ModulesConfig::official(), &ctx);

    // After module initialization, services should be registered
    // Check for ResolverRegistry (registered by VimModule)
    let resolver_registry = services.get::<ResolverRegistry>();
    assert!(
        resolver_registry.is_some(),
        "ResolverRegistry should be registered by VimModule"
    );
}

#[test]
fn test_resolve_mode_str_valid() {
    let state = create_session_state();
    let mode_reg = {
        let services = Arc::new(ServiceRegistry::new());
        let kernel = create_kernel_context(Arc::clone(&services));
        let ctx = create_module_context(kernel, Arc::clone(&services));
        let _tracked = initialize_modules(&ModulesConfig::official(), &ctx);
        let (mode_registry, _, _, _) = extract_registries(&services);
        mode_registry
    };
    // vim:normal should exist after module init
    let result = resolve_mode_str("vim:normal", &mode_reg);
    assert!(result.is_some());
    drop(state);
}

#[test]
fn test_resolve_mode_str_no_colon() {
    let mode_reg = ModeRegistry::new();
    let result = resolve_mode_str("normal", &mode_reg);
    assert!(result.is_none());
}

#[test]
fn test_resolve_mode_str_not_found() {
    let mode_reg = ModeRegistry::new();
    let result = resolve_mode_str("nonexistent:mode", &mode_reg);
    assert!(result.is_none());
}

#[test]
fn test_default_data_dir() {
    let dir = default_data_dir();
    let dir_str = dir.to_string_lossy();
    assert!(dir_str.contains("reovim"));
    assert!(dir_str.contains("modules"));
}

#[test]
fn test_default_cache_dir() {
    let dir = default_cache_dir();
    let dir_str = dir.to_string_lossy();
    assert!(dir_str.contains("reovim"));
    assert!(dir_str.contains("modules"));
}

#[test]
fn test_create_kernel_context_valid() {
    let services = Arc::new(ServiceRegistry::new());
    let kernel = create_kernel_context(Arc::clone(&services));
    // The kernel should have been constructed successfully
    drop(kernel);
}

#[test]
fn test_create_extra_module_textobjects() {
    let module = create_extra_module("textobjects");
    assert!(module.is_some());
    assert_eq!(module.unwrap().id().as_str(), "textobjects");
}

#[test]
fn test_create_extra_module_unknown() {
    assert!(create_extra_module("nonexistent").is_none());
    assert!(create_extra_module("").is_none());
}

#[test]
fn test_all_module_deps_resolve() {
    // Verify all default modules form a valid dependency graph (#582)
    let modules = DefaultsModule::create_modules();
    let entries: Vec<DepEntry<ModuleId>> = modules
        .iter()
        .map(|m| DepEntry {
            key: m.id(),
            required: m.dependencies(),
            optional: m.optional_dependencies(),
        })
        .collect();
    let result = resolve_dependencies(&entries);
    assert!(result.is_ok(), "Module dependency graph has errors: {result:?}");
    let order = result.unwrap();
    assert_eq!(order.order.len(), modules.len());
}

#[test]
fn test_tier_ordering() {
    // Verify dependency order constraints (#582)
    let modules = DefaultsModule::create_modules();
    let entries: Vec<DepEntry<ModuleId>> = modules
        .iter()
        .map(|m| DepEntry {
            key: m.id(),
            required: m.dependencies(),
            optional: m.optional_dependencies(),
        })
        .collect();
    let order = resolve_dependencies(&entries).unwrap();
    let pos = |name: &str| {
        order
            .order
            .iter()
            .position(|id| id.as_str() == name)
            .unwrap()
    };

    // vim must come after editor and motions
    assert!(pos("editor") < pos("vim"), "editor must init before vim");
    assert!(pos("motions") < pos("vim"), "motions must init before vim");
    // #585: snippet and range-finder optionally depend on vim (ModeBridgeStore)
    assert!(pos("vim") < pos("snippet"), "vim must init before snippet");
    assert!(pos("vim") < pos("range-finder"), "vim must init before range-finder");
}

#[test]
fn test_on_all_loaded_wired() {
    // Verify on_all_loaded() is called without panic (#582)
    let services = Arc::new(ServiceRegistry::new());
    let kernel = create_kernel_context(Arc::clone(&services));
    let ctx = create_module_context(kernel, Arc::clone(&services));
    let mut tracked = initialize_modules(&ModulesConfig::official(), &ctx);
    // Should not panic — currently no-op for all modules
    call_on_all_loaded(&mut tracked, &ctx);
    // Verify all modules are in Running state
    for tm in &tracked {
        assert_eq!(tm.state, ModuleState::Running);
    }
}
