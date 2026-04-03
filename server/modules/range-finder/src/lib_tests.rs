use super::*;

#[test]
fn test_module_id() {
    let module = RangeFinderModule::new();
    assert_eq!(module.id().as_str(), "range-finder");
}

#[test]
fn test_module_name() {
    let module = RangeFinderModule::new();
    assert_eq!(module.name(), "range-finder");
}

#[test]
fn test_module_version() {
    let module = RangeFinderModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn test_module_default() {
    let module = RangeFinderModule::default();
    assert_eq!(module.id().as_str(), "range-finder");
}

#[test]
fn test_module_exit() {
    let mut module = RangeFinderModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_extension_kinds() {
    let module = RangeFinderModule::new();
    assert_eq!(module.extension_kinds(), &["range-finder-jump", "range-finder-fold"]);
}

#[test]
fn test_module_init_with_bridge_store() {
    use {
        reovim_driver_manifest::{ManifestModeBridge, ModeBridgeStore},
        reovim_kernel::api::v1::{ModeId, ServiceRegistry},
        std::sync::Arc,
    };

    let services = Arc::new(ServiceRegistry::new());

    // Register mock parent mode
    let parent = ModeId::new(ModuleId::new("vim"), "normal");
    let modes = services.get_or_create::<ModeInfoStore>();
    modes.add(ModeInfo {
        id: parent,
        display_name: "NORMAL",
        cursor_style: CursorStyle::Block,
        accepts_char_input: false,
        has_selection: false,
        inherits_from: None,
        is_entry: true,
    });

    // Register ModeBridgeStore (as VimModule would)
    let bridge_store = ModeBridgeStore::new(vec![ManifestModeBridge {
        feature_mode: "range-finder:jump-input".to_string(),
        parent_mode: "vim:normal".to_string(),
    }]);
    services.register(Arc::new(bridge_store));

    let ctx = test_module_context(services.clone());

    let mut module = RangeFinderModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Verify bridges were registered (jump + fold = 2)
    let provider = services
        .get::<reovim_driver_session::bridges::BridgeProvider>()
        .unwrap();
    let bridges = provider.take_bridges();
    assert_eq!(bridges.len(), 2);
    assert_eq!(bridges[0].kind(), "range-finder-jump");
    assert_eq!(bridges[1].kind(), "range-finder-fold");

    // Verify commands were registered (4 jump + 5 fold + 1 enhanced find-char = 10)
    let command_store = services
        .get::<reovim_driver_command::CommandHandlerStore>()
        .unwrap();
    let handlers = command_store.take_handlers();
    assert_eq!(handlers.len(), 10);

    // Verify resolver was registered (#524)
    let resolvers = services.get::<ResolverRegistry>().unwrap();
    assert!(resolvers.get(&crate::jump::ids::JUMP_INPUT_MODE).is_some());

    // Verify mode info was registered (#524)
    let modes = services.get::<ModeInfoStore>().unwrap();
    let mode_list = modes.take_modes();
    // 1 mock vim:normal + 1 JUMP = 2
    assert_eq!(mode_list.len(), 2);
    assert_eq!(mode_list[1].display_name, "JUMP");
    assert_eq!(mode_list[1].cursor_style, CursorStyle::Block);
    assert!(mode_list[1].accepts_char_input);
    assert!(!mode_list[1].is_entry);
    // Verify inherits_from is set to the parent
    assert!(mode_list[1].inherits_from.is_some());
}

#[test]
fn test_module_init_without_bridge_store() {
    use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = RangeFinderModule::new();
    let result = module.init(&ctx);
    // Should succeed even without ModeBridgeStore (reduced functionality)
    assert!(matches!(result, ProbeResult::Success));

    // Verify commands still registered (4 jump + 5 fold = 9, no enhanced find-char)
    let command_store = services
        .get::<reovim_driver_command::CommandHandlerStore>()
        .unwrap();
    let handlers = command_store.take_handlers();
    assert_eq!(handlers.len(), 9);
}

#[test]
fn test_dependencies_empty() {
    let module = RangeFinderModule::new();
    assert!(module.dependencies().is_empty());
}

#[test]
fn test_optional_dependencies_contain_vim() {
    let module = RangeFinderModule::new();
    let opt_deps = module.optional_dependencies();
    assert_eq!(opt_deps.len(), 1);
    assert_eq!(opt_deps[0].as_str(), "vim");
}

/// Create a minimal `ModuleContext` for testing.
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_context(
    services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
) -> ModuleContext {
    use {
        parking_lot::RwLock,
        reovim_kernel::api::v1::{EventBus, KernelContext, MarkBank, OptionRegistry},
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(reovim_driver_buffer::TestBufferManager::new()),
        Arc::new(RwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        services.clone(),
    );
    ModuleContext::new(
        kernel,
        services,
        std::path::PathBuf::from("/tmp"),
        std::path::PathBuf::from("/tmp"),
    )
}
