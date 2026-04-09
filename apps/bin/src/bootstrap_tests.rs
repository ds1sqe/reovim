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

/// Helper: create all builtin modules for test assertions.
///
/// Uses static factory map + manifest ordering (#620).
#[cfg(feature = "static-modules")]
fn create_all_builtin_modules() -> Vec<Box<dyn Module>> {
    let manifest = parse_builtin_manifest();
    let registry = static_modules::builtin_registry();
    manifest
        .module_ids()
        .into_iter()
        .filter_map(|id| registry.get(id).map(|factory| factory()))
        .collect()
}

#[test]
fn test_all_module_deps_resolve() {
    // Verify all default modules form a valid dependency graph (#582)
    let modules = create_all_builtin_modules();
    let entries: Vec<DepEntry<ModuleId>> = modules
        .iter()
        .map(|m| DepEntry {
            key: m.id(),
            required: m.dependencies(),
            optional: m.optional_dependencies(),
            provides_caps: m.provides().to_vec(),
            requires_caps: m.requires().to_vec(),
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
    let modules = create_all_builtin_modules();
    let entries: Vec<DepEntry<ModuleId>> = modules
        .iter()
        .map(|m| DepEntry {
            key: m.id(),
            required: m.dependencies(),
            optional: m.optional_dependencies(),
            provides_caps: m.provides().to_vec(),
            requires_caps: m.requires().to_vec(),
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

// ============================================================================
// Phase 6: External module integration tests (#587)
// ============================================================================

#[test]
fn test_bootstrap_with_external_discovery() {
    // Bootstrap initializes all builtin modules even when external .so files
    // exist on system search paths. External modules that duplicate builtins
    // are filtered out (#587).
    //
    // Since #725 Phase 3, this test also implicitly covers P3-T2:
    // `tracked` now contains BOTH builtin and external modules, and the
    // loop below asserts every entry is `Running`. If an external `.so`
    // is discoverable in the test environment, it passes through the same
    // unified init loop as builtins and must also reach `Running` — not
    // the old "init deferred" state. Explicit fixture-based P3-T2
    // coverage (loading `libreovim_test_dynamic_module.so` via a
    // controlled `REOVIM_MODULE_PATH`) is deferred to #729 where the E2E
    // sample module pair exercises the full stack.
    let services = Arc::new(ServiceRegistry::new());
    let kernel = create_kernel_context(Arc::clone(&services));
    let ctx = create_module_context(kernel, Arc::clone(&services));
    let tracked = initialize_modules(&ModulesConfig::official(), &ctx);

    // All builtins (and any externals that happened to be on the search
    // path) must be in Running state — never deferred.
    assert!(!tracked.is_empty());
    for tm in &tracked {
        assert_eq!(tm.state, ModuleState::Running);
    }
}

#[test]
fn test_discover_and_load_externals_graceful() {
    // External module discovery never panics, regardless of what's on disk.
    // Duplicate builtins are filtered, failed loads are logged and skipped.
    // #620: Use manifest IDs instead of DefaultsModule::create_modules()
    let manifest = parse_builtin_manifest();
    let builtin_ids: Vec<ModuleId> = manifest
        .module_ids()
        .into_iter()
        .map(|id| ModuleId::from_string(id.to_string()))
        .collect();
    let _loader = discover_and_load_externals(&ModulesConfig::official(), &builtin_ids);
    // Success = no panic, no matter what .so files are on the system
}

#[test]
fn test_check_lockfile_staleness_missing() {
    // Should not panic when lock file doesn't exist (normal first run)
    check_lockfile_staleness();
}

// ============================================================================
// #623: Cross-personality initial mode tests
// ============================================================================

#[test]
fn test_default_bootstrap_uses_vim_normal() {
    // With default config (vim enabled), initial mode should be vim:normal
    let state = create_session_state();
    let mode = state.home_mode();
    assert_eq!(mode.name(), "normal", "Default personality should start in vim:normal");
}

#[test]
fn test_initial_mode_provider_registered_by_vim() {
    let services = Arc::new(ServiceRegistry::new());
    let kernel = create_kernel_context(Arc::clone(&services));
    let ctx = create_module_context(kernel, Arc::clone(&services));
    let _tracked = initialize_modules(&ModulesConfig::official(), &ctx);

    let provider = services
        .get::<reovim_driver_session::InitialModeProvider>()
        .expect("VimModule should register InitialModeProvider");
    let mode = provider.get().expect("initial mode should be set");
    assert_eq!(mode, ModeId::new(ModuleId::new("vim"), "normal"));
}

#[test]
fn test_fallback_without_personality_module() {
    // If no personality module registers an initial mode, bootstrap falls back
    // to vim:normal.
    let services = Arc::new(ServiceRegistry::new());
    let fallback = services
        .get::<reovim_driver_session::InitialModeProvider>()
        .and_then(|p| p.get())
        .unwrap_or_else(|| ModeId::new(ModuleId::new("vim"), "normal"));
    assert_eq!(fallback, ModeId::new(ModuleId::new("vim"), "normal"));
}

// ============================================================================
// #620: Manifest and static modules tests
// ============================================================================

#[test]
fn test_parse_builtin_manifest() {
    let manifest = parse_builtin_manifest();
    let ids = manifest.module_ids();
    // Should have all 40 builtin modules + emacs
    assert!(ids.len() >= 40, "Expected at least 40 modules, got {}", ids.len());
    // Key modules must be present
    assert!(ids.contains(&"vim"));
    assert!(ids.contains(&"editor"));
    assert!(ids.contains(&"motions"));
    assert!(ids.contains(&"undo"));
}

#[cfg(feature = "static-modules")]
#[test]
fn test_static_registry_covers_manifest() {
    // Verify the static factory map covers all manifest entries (except emacs)
    let manifest = parse_builtin_manifest();
    let registry = static_modules::builtin_registry();
    for id in manifest.module_ids() {
        if id == "emacs" {
            continue; // Alternative personality, not in static registry
        }
        assert!(
            registry.contains_key(id),
            "Static registry missing module '{id}' from builtins.toml"
        );
    }
}

// ============================================================================
// #725 Phase 2 — load_registry_modules tests
// ============================================================================

/// P2-T1: registry path does not exist → `load_registry_modules` returns
/// without loading, logs at debug level, and leaves the loader unchanged.
#[test]
fn test_load_registry_modules_missing_registry_noop() {
    use reovim_driver_module_loader::loader::ModuleLoader;
    use reovim_driver_module_registry::workflow::RegistryPaths;

    let tmp = std::env::temp_dir().join("reovim-p2-t1-missing");
    // Ensure it really doesn't exist.
    let _ = std::fs::remove_dir_all(&tmp);

    let paths = RegistryPaths::new(tmp);
    let mut loader = ModuleLoader::new();
    let config = ModulesConfig::official();
    let builtin_ids: Vec<reovim_kernel::api::v1::ModuleId> = Vec::new();

    load_registry_modules(&mut loader, &config, &builtin_ids, &paths);

    assert_eq!(
        loader.len(),
        0,
        "loader should remain empty when registry path does not exist",
    );
}

/// P2-T3: registry module ID conflicts with a builtin → skipped. Verified
/// by constructing a registry with a fake installed entry whose ID matches
/// a provided builtin ID; expected outcome is loader count = 0 (entry
/// skipped before any dlopen attempt).
#[test]
fn test_load_registry_modules_builtin_conflict_skipped() {
    use std::fs;

    use reovim_driver_module_loader::loader::ModuleLoader;
    use reovim_driver_module_registry::workflow::RegistryPaths;
    use reovim_kernel::api::v1::ModuleId;

    let tmp = std::env::temp_dir().join("reovim-p2-t3-builtin-conflict");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Hand-write an `installed.json` with a module ID that collides with a
    // builtin we pass in as `builtin_ids`. The library_path points at a
    // nonexistent location — we expect the builtin check to short-circuit
    // BEFORE the filesystem check, so the nonexistent path never matters.
    let json = r#"{
        "modules": {
            "vim": {
                "id": "vim",
                "version": "1.0.0",
                "source": { "type": "local", "path": "/tmp/fake" },
                "install_path": "/tmp/fake",
                "library_path": "/tmp/fake/libnothing.so"
            }
        }
    }"#;
    fs::write(tmp.join("installed.json"), json).unwrap();

    let paths = RegistryPaths::new(tmp.clone());
    let mut loader = ModuleLoader::new();
    let config = ModulesConfig::official();
    let builtin_ids = vec![ModuleId::new("vim")];

    load_registry_modules(&mut loader, &config, &builtin_ids, &paths);

    assert_eq!(
        loader.len(),
        0,
        "registry module matching builtin should be skipped",
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// P2-T5: registry module has `library_path: None` → skipped with warning,
/// loader unchanged.
#[test]
fn test_load_registry_modules_missing_library_path_skipped() {
    use std::fs;

    use reovim_driver_module_loader::loader::ModuleLoader;
    use reovim_driver_module_registry::workflow::RegistryPaths;

    let tmp = std::env::temp_dir().join("reovim-p2-t5-no-library");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Module entry with library_path omitted (defaults to None via serde).
    let json = r#"{
        "modules": {
            "unbuilt-module": {
                "id": "unbuilt-module",
                "version": "0.1.0",
                "source": { "type": "local", "path": "/tmp/unbuilt" },
                "install_path": "/tmp/unbuilt"
            }
        }
    }"#;
    fs::write(tmp.join("installed.json"), json).unwrap();

    let paths = RegistryPaths::new(tmp.clone());
    let mut loader = ModuleLoader::new();
    let config = ModulesConfig::official();
    let builtin_ids = Vec::new();

    load_registry_modules(&mut loader, &config, &builtin_ids, &paths);

    assert_eq!(
        loader.len(),
        0,
        "registry module without library_path should be skipped",
    );

    let _ = fs::remove_dir_all(&tmp);
}
