use std::sync::Arc;

use super::*;

// =========================================================================
// SnippetModule construction
// =========================================================================

#[test]
fn test_new_creates_module() {
    let module = SnippetModule::new();
    assert!(module.handle.is_none());
}

#[test]
fn test_default_creates_same_as_new() {
    let from_new = SnippetModule::new();
    let from_default = SnippetModule::default();
    assert!(from_new.handle.is_none());
    assert!(from_default.handle.is_none());
}

// =========================================================================
// Module trait
// =========================================================================

#[test]
fn test_module_id() {
    let module = SnippetModule::new();
    assert_eq!(module.id().as_str(), "snippet");
}

#[test]
fn test_module_name() {
    let module = SnippetModule::new();
    assert_eq!(module.name(), "Snippet");
}

#[test]
fn test_module_version() {
    let module = SnippetModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
    assert_eq!(version.patch, 0);
}

// =========================================================================
// Exit
// =========================================================================

#[test]
fn test_exit_clears_handle() {
    let mut module = SnippetModule::new();
    module.handle = Some(SnippetRegistryHandle::new(SnippetRegistry::new()));
    assert!(module.exit().is_ok());
    assert!(module.handle.is_none());
    assert!(module.data_dir.is_none());
}

#[test]
fn test_exit_ok_when_no_handle() {
    let mut module = SnippetModule::new();
    assert!(module.exit().is_ok());
}

// =========================================================================
// Keybindings
// =========================================================================

#[test]
fn test_keybindings_count() {
    let module = SnippetModule::new();
    // 3 vim personality bindings (#700) + 3 snippet:navigating bindings
    #[cfg(feature = "vim-keybindings")]
    assert_eq!(module.keybindings().len(), 6);
    #[cfg(not(feature = "vim-keybindings"))]
    assert_eq!(module.keybindings().len(), 3);
}

#[test]
fn test_keybindings_jump_next() {
    let module = SnippetModule::new();
    let bindings = module.keybindings();
    let next = bindings
        .iter()
        .find(|b| b.keys == "<Tab>")
        .expect("<Tab> binding missing");
    assert_eq!(next.command_id, ids::JUMP_NEXT);
}

#[test]
fn test_keybindings_jump_prev() {
    let module = SnippetModule::new();
    let bindings = module.keybindings();
    let prev = bindings
        .iter()
        .find(|b| b.keys == "<S-Tab>")
        .expect("<S-Tab> binding missing");
    assert_eq!(prev.command_id, ids::JUMP_PREV);
}

#[test]
fn test_keybindings_cancel() {
    let module = SnippetModule::new();
    let bindings = module.keybindings();
    let cancel = bindings
        .iter()
        .find(|b| b.keys == "<Esc>")
        .expect("<Esc> binding missing");
    assert_eq!(cancel.command_id, ids::CANCEL);
}

// =========================================================================
// CommandProvider
// =========================================================================

#[test]
fn test_command_provider_returns_handlers() {
    let module = SnippetModule::new();
    let handlers = module.command_handlers();
    assert_eq!(handlers.len(), 6);
}

#[test]
fn test_command_provider_matches_all_commands() {
    use std::path::PathBuf;
    let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
    let fallback = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "insert");
    let module_handlers = SnippetModule::new().command_handlers();
    let all = command::all_commands(handle, fallback, PathBuf::from("/tmp"));
    assert_eq!(module_handlers.len(), all.len());
}

// =========================================================================
// Dependencies
// =========================================================================

#[test]
fn test_dependencies() {
    let module = SnippetModule::new();
    assert!(module.dependencies().is_empty());
}

#[test]
fn test_optional_dependencies_contain_vim() {
    let module = SnippetModule::new();
    let opt_deps = module.optional_dependencies();
    assert_eq!(opt_deps.len(), 1);
    assert_eq!(opt_deps[0].as_str(), "vim");
}

// =========================================================================
// Init with real context
// =========================================================================

/// Register a mock parent mode and `ModeBridgeStore` config
/// (personality module initializes before snippet).
fn register_mock_parent(services: &Arc<reovim_kernel::api::v1::ServiceRegistry>) {
    use {
        reovim_driver_manifest::{ManifestModeBridge, ModeBridgeStore},
        reovim_kernel::api::v1::ModeId,
    };

    let parent = ModeId::new(ModuleId::new("vim"), "insert");
    let modes = services.get_or_create::<ModeInfoStore>();
    modes.add(ModeInfo {
        id: parent,
        display_name: "INSERT",
        cursor_style: CursorStyle::Bar,
        accepts_char_input: true,
        has_selection: false,
        inherits_from: None,
        is_entry: false,
    });

    let bridge_store = ModeBridgeStore::new(vec![ManifestModeBridge {
        feature_mode: "snippet:navigating".to_string(),
        parent_mode: "vim:insert".to_string(),
    }]);
    services.register(Arc::new(bridge_store));
}

#[test]
fn test_init_registers_commands() {
    use {
        reovim_kernel::api::v1::{EventBus, KernelContext, ServiceRegistry},
        std::path::PathBuf,
    };

    let event_bus = Arc::new(EventBus::new());
    let services = Arc::new(ServiceRegistry::new());
    register_mock_parent(&services);

    let kernel = KernelContext::with_event_bus_services_and_options(
        event_bus,
        Arc::clone(&services),
        Arc::new(reovim_kernel::api::v1::OptionRegistry::new()),
    );
    let ctx = ModuleContext::new(
        kernel,
        services,
        PathBuf::from("/tmp/reovim-snippet-test/data"),
        PathBuf::from("/tmp/reovim-snippet-test/cache"),
    );

    let mut module = SnippetModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);
    assert!(module.handle.is_some());

    // Verify CommandHandlerStore is populated
    let store = ctx.services.get::<CommandHandlerStore>();
    assert!(store.is_some());

    // Verify ResolverRegistry is populated
    let resolvers = ctx.services.get::<ResolverRegistry>();
    assert!(resolvers.is_some());

    // Verify ModeInfoStore is populated
    let modes = ctx.services.get::<ModeInfoStore>();
    assert!(modes.is_some());
}

#[test]
fn test_init_then_exit() {
    use {
        reovim_kernel::api::v1::{EventBus, KernelContext, ServiceRegistry},
        std::path::PathBuf,
    };

    let event_bus = Arc::new(EventBus::new());
    let services = Arc::new(ServiceRegistry::new());
    register_mock_parent(&services);

    let kernel = KernelContext::with_event_bus_services_and_options(
        event_bus,
        Arc::clone(&services),
        Arc::new(reovim_kernel::api::v1::OptionRegistry::new()),
    );
    let ctx = ModuleContext::new(
        kernel,
        services,
        PathBuf::from("/tmp/reovim-snippet-test/data"),
        PathBuf::from("/tmp/reovim-snippet-test/cache"),
    );

    let mut module = SnippetModule::new();
    module.init(&ctx);
    assert!(module.handle.is_some());
    module.exit().unwrap();
    assert!(module.handle.is_none());
}

// =========================================================================
// build_registry (#529)
// =========================================================================

mod build_registry_tests {
    use {
        super::*,
        std::{
            io::Write,
            sync::atomic::{AtomicU32, Ordering},
        },
    };

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn make_temp_dir() -> TempDir {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("reovim-build-registry-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    #[test]
    fn test_build_registry_empty_dir() {
        let dir = make_temp_dir();
        let registry = build_registry(dir.path());
        assert!(registry.find_by_prefix("rust", "fn").is_none());
    }

    #[test]
    fn test_build_registry_user_dir() {
        let dir = make_temp_dir();
        let user_dir = dir.path().join("user");
        std::fs::create_dir_all(&user_dir).unwrap();
        let mut f = std::fs::File::create(user_dir.join("rust.json")).unwrap();
        writeln!(f, r#"{{ "function": {{ "prefix": "fn", "body": "fn $1() {{}}" }} }}"#).unwrap();

        let registry = build_registry(dir.path());
        assert!(registry.find_by_prefix("rust", "fn").is_some());
    }

    #[test]
    fn test_build_registry_legacy_flat_layout() {
        let dir = make_temp_dir();
        // Legacy: snippets/ directory (no user/ dir)
        let snippets_dir = dir.path().join("snippets");
        std::fs::create_dir_all(&snippets_dir).unwrap();
        let mut f = std::fs::File::create(snippets_dir.join("global.json")).unwrap();
        writeln!(f, r#"{{ "test": {{ "prefix": "tst", "body": "TEST" }} }}"#).unwrap();

        let registry = build_registry(dir.path());
        // Legacy snippets/ loaded as user source
        assert!(registry.find_by_prefix("global", "tst").is_some());
    }

    #[test]
    fn test_build_registry_user_takes_precedence_over_legacy() {
        let dir = make_temp_dir();
        // Both user/ and snippets/ exist — user/ wins
        let user_dir = dir.path().join("user");
        std::fs::create_dir_all(&user_dir).unwrap();
        let mut f = std::fs::File::create(user_dir.join("global.json")).unwrap();
        writeln!(f, r#"{{ "from_user": {{ "prefix": "tst", "body": "USER" }} }}"#).unwrap();

        let snippets_dir = dir.path().join("snippets");
        std::fs::create_dir_all(&snippets_dir).unwrap();
        let mut f2 = std::fs::File::create(snippets_dir.join("global.json")).unwrap();
        writeln!(f2, r#"{{ "from_legacy": {{ "prefix": "tst", "body": "LEGACY" }} }}"#).unwrap();

        let registry = build_registry(dir.path());
        let def = registry.find_by_prefix("global", "tst").unwrap();
        assert_eq!(def.name, "from_user");
    }

    #[test]
    fn test_build_registry_builtin_is_lowest_priority() {
        let dir = make_temp_dir();
        // User snippet
        let user_dir = dir.path().join("user");
        std::fs::create_dir_all(&user_dir).unwrap();
        let mut f = std::fs::File::create(user_dir.join("global.json")).unwrap();
        writeln!(f, r#"{{ "user_fn": {{ "prefix": "fn", "body": "USER" }} }}"#).unwrap();

        // Built-in snippet with same prefix
        let builtin_dir = dir.path().join("built-in");
        std::fs::create_dir_all(&builtin_dir).unwrap();
        let mut f2 = std::fs::File::create(builtin_dir.join("global.json")).unwrap();
        writeln!(f2, r#"{{ "builtin_fn": {{ "prefix": "fn", "body": "BUILTIN" }} }}"#).unwrap();

        let registry = build_registry(dir.path());
        // User wins over built-in
        let def = registry.find_by_prefix("global", "fn").unwrap();
        assert_eq!(def.name, "user_fn");
    }

    #[test]
    fn test_build_registry_builtin_found_when_no_user() {
        let dir = make_temp_dir();
        let builtin_dir = dir.path().join("built-in");
        std::fs::create_dir_all(&builtin_dir).unwrap();
        let mut f = std::fs::File::create(builtin_dir.join("rust.json")).unwrap();
        writeln!(f, r#"{{ "builtin": {{ "prefix": "fn", "body": "BUILTIN" }} }}"#).unwrap();

        let registry = build_registry(dir.path());
        let def = registry.find_by_prefix("rust", "fn").unwrap();
        assert_eq!(def.name, "builtin");
    }

    #[test]
    fn test_build_registry_friendly_snippets() {
        let dir = make_temp_dir();
        let friendly_dir = dir.path().join("friendly-snippets");
        std::fs::create_dir_all(friendly_dir.join("snippets")).unwrap();

        // Create package.json manifest
        let mut pkg = std::fs::File::create(friendly_dir.join("package.json")).unwrap();
        writeln!(
            pkg,
            r#"{{ "contributes": {{ "snippets": [{{ "language": "rust", "path": "./snippets/rust.json" }}] }} }}"#
        )
        .unwrap();

        // Create snippet file
        let mut snip = std::fs::File::create(friendly_dir.join("snippets/rust.json")).unwrap();
        writeln!(snip, r#"{{ "friendly_fn": {{ "prefix": "ffn", "body": "FRIENDLY" }} }}"#)
            .unwrap();

        let registry = build_registry(dir.path());
        let def = registry.find_by_prefix("rust", "ffn").unwrap();
        assert_eq!(def.name, "friendly_fn");
    }

    #[test]
    fn test_build_registry_friendly_without_package_json_ignored() {
        let dir = make_temp_dir();
        // Create friendly-snippets dir WITHOUT package.json
        let friendly_dir = dir.path().join("friendly-snippets");
        std::fs::create_dir_all(&friendly_dir).unwrap();

        let registry = build_registry(dir.path());
        assert!(registry.find_by_prefix("rust", "ffn").is_none());
    }

    #[test]
    fn test_build_registry_user_dir_with_invalid_json() {
        let dir = make_temp_dir();
        let user_dir = dir.path().join("user");
        std::fs::create_dir_all(&user_dir).unwrap();
        // Write invalid JSON to trigger Err branch
        let mut f = std::fs::File::create(user_dir.join("rust.json")).unwrap();
        writeln!(f, "{{invalid json!!!").unwrap();

        // Should not panic — Err is silently ignored
        let registry = build_registry(dir.path());
        assert!(registry.find_by_prefix("rust", "fn").is_none());
    }

    #[test]
    fn test_build_registry_friendly_with_bad_package_json() {
        let dir = make_temp_dir();
        let friendly_dir = dir.path().join("friendly-snippets");
        std::fs::create_dir_all(&friendly_dir).unwrap();
        // Create invalid package.json — exists() is true but load() returns Err
        let mut pkg = std::fs::File::create(friendly_dir.join("package.json")).unwrap();
        writeln!(pkg, "not valid json").unwrap();

        let registry = build_registry(dir.path());
        // friendly-snippets NOT loaded (bad JSON), but user/builtin empty providers registered
        // Provider count should be 2 (empty user + empty builtin), not 3
        assert_eq!(registry.provider_count(), 2);
    }

    #[test]
    fn test_build_registry_builtin_with_invalid_json() {
        let dir = make_temp_dir();
        let builtin_dir = dir.path().join("built-in");
        std::fs::create_dir_all(&builtin_dir).unwrap();
        let mut f = std::fs::File::create(builtin_dir.join("rust.json")).unwrap();
        writeln!(f, "{{not json").unwrap();

        let registry = build_registry(dir.path());
        assert!(registry.find_by_prefix("rust", "fn").is_none());
    }

    #[test]
    fn test_build_registry_all_three_sources() {
        let dir = make_temp_dir();

        // User
        let user_dir = dir.path().join("user");
        std::fs::create_dir_all(&user_dir).unwrap();
        let mut f = std::fs::File::create(user_dir.join("global.json")).unwrap();
        writeln!(f, r#"{{ "user_s": {{ "prefix": "us", "body": "USER" }} }}"#).unwrap();

        // Friendly
        let friendly_dir = dir.path().join("friendly-snippets");
        std::fs::create_dir_all(friendly_dir.join("snippets")).unwrap();
        let mut pkg = std::fs::File::create(friendly_dir.join("package.json")).unwrap();
        writeln!(
            pkg,
            r#"{{ "contributes": {{ "snippets": [{{ "language": "global", "path": "./snippets/global.json" }}] }} }}"#
        )
        .unwrap();
        let mut snip = std::fs::File::create(friendly_dir.join("snippets/global.json")).unwrap();
        writeln!(snip, r#"{{ "friendly_s": {{ "prefix": "fs", "body": "FRIENDLY" }} }}"#).unwrap();

        // Built-in
        let builtin_dir = dir.path().join("built-in");
        std::fs::create_dir_all(&builtin_dir).unwrap();
        let mut f2 = std::fs::File::create(builtin_dir.join("global.json")).unwrap();
        writeln!(f2, r#"{{ "builtin_s": {{ "prefix": "bs", "body": "BUILTIN" }} }}"#).unwrap();

        let registry = build_registry(dir.path());
        assert_eq!(registry.provider_count(), 3);
        assert!(registry.find_by_prefix("global", "us").is_some());
        assert!(registry.find_by_prefix("global", "fs").is_some());
        assert!(registry.find_by_prefix("global", "bs").is_some());
    }
}
