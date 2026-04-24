use reovim_subsys_module_config::ModulesConfig;

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
fn test_discover_and_load_externals_graceful() {
    // External module discovery never panics, regardless of what's on disk.
    // Duplicate builtins are filtered, failed loads are logged and skipped.
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
fn test_fallback_without_personality_module() {
    // If no personality module registers an initial mode, bootstrap falls back
    // to vim:normal.
    let services = Arc::new(ServiceRegistry::new());
    let fallback = services
        .get::<reovim_driver_text_session::InitialModeProvider>()
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

// ============================================================================
// #725 Phase 2 — load_registry_modules tests
// ============================================================================

/// P2-T1: registry path does not exist → `load_registry_modules` returns
/// without loading, logs at debug level, and leaves the loader unchanged.
#[test]
fn test_load_registry_modules_missing_registry_noop() {
    use {
        reovim_subsys_module_loader::loader::ModuleLoader,
        reovim_subsys_module_registry::workflow::RegistryPaths,
    };

    let tmp = std::env::temp_dir().join("reovim-p2-t1-missing");
    // Ensure it really doesn't exist.
    let _ = std::fs::remove_dir_all(&tmp);

    let paths = RegistryPaths::new(tmp);
    let mut loader = ModuleLoader::new();
    let config = ModulesConfig::official();
    let builtin_ids: Vec<reovim_kernel::api::v1::ModuleId> = Vec::new();

    load_registry_modules(&mut loader, &config, &builtin_ids, &paths);

    assert_eq!(loader.len(), 0, "loader should remain empty when registry path does not exist");
}

/// P2-T3: registry module ID conflicts with a builtin → skipped. Verified
/// by constructing a registry with a fake installed entry whose ID matches
/// a provided builtin ID; expected outcome is loader count = 0 (entry
/// skipped before any dlopen attempt).
#[test]
fn test_load_registry_modules_builtin_conflict_skipped() {
    use std::fs;

    use {
        reovim_kernel::api::v1::ModuleId, reovim_subsys_module_loader::loader::ModuleLoader,
        reovim_subsys_module_registry::workflow::RegistryPaths,
    };

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

    assert_eq!(loader.len(), 0, "registry module matching builtin should be skipped");

    let _ = fs::remove_dir_all(&tmp);
}

/// P2-T5: registry module has `library_path: None` → skipped with warning,
/// loader unchanged.
#[test]
fn test_load_registry_modules_missing_library_path_skipped() {
    use std::fs;

    use {
        reovim_subsys_module_loader::loader::ModuleLoader,
        reovim_subsys_module_registry::workflow::RegistryPaths,
    };

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

    assert_eq!(loader.len(), 0, "registry module without library_path should be skipped");

    let _ = fs::remove_dir_all(&tmp);
}

/// P2-T6: installed.json is corrupt (invalid JSON) → `load_registry_modules`
/// recovers gracefully, logs the error, and leaves the loader unchanged.
#[test]
fn test_load_registry_modules_corrupt_installed_json() {
    use std::fs;

    use {
        reovim_subsys_module_loader::loader::ModuleLoader,
        reovim_subsys_module_registry::workflow::RegistryPaths,
    };

    let tmp = std::env::temp_dir().join("reovim-p2-t6-corrupt-json");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Write garbage to installed.json
    fs::write(tmp.join("installed.json"), "{{not valid json!!!").unwrap();

    let paths = RegistryPaths::new(tmp.clone());
    let mut loader = ModuleLoader::new();
    let config = ModulesConfig::official();
    let builtin_ids = Vec::new();

    load_registry_modules(&mut loader, &config, &builtin_ids, &paths);

    assert_eq!(loader.len(), 0, "corrupt installed.json should be skipped gracefully");

    let _ = fs::remove_dir_all(&tmp);
}
