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
fn test_module_init() {
    use {
        reovim_kernel::api::v1::{ModeId, ServiceRegistry},
        std::sync::Arc,
    };

    let services = Arc::new(ServiceRegistry::new());

    // Register mock parent mode and JumpParentMode config
    // (adapter initializes before range-finder)
    let parent = ModeId::new(ModuleId::new("test"), "normal");
    let modes = services.get_or_create::<ModeInfoStore>();
    modes.add(ModeInfo {
        id: parent.clone(),
        display_name: "NORMAL",
        cursor_style: CursorStyle::Block,
        accepts_char_input: false,
        has_selection: false,
        inherits_from: None,
        is_entry: true,
    });
    services.register(std::sync::Arc::new(JumpParentMode::new(parent)));

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

    // Verify commands were registered (3 jump + 5 fold = 8)
    let command_store = services
        .get::<reovim_driver_command::CommandHandlerStore>()
        .unwrap();
    let handlers = command_store.take_handlers();
    assert_eq!(handlers.len(), 8);

    // Verify resolver was registered (#524)
    let resolvers = services.get::<ResolverRegistry>().unwrap();
    assert!(resolvers.get(&crate::jump::ids::JUMP_INPUT_MODE).is_some());

    // Verify mode info was registered (#524)
    // 1 mock vim:normal + 1 JUMP = 2 modes total
    let modes = services.get::<ModeInfoStore>().unwrap();
    let mode_list = modes.take_modes();
    assert_eq!(mode_list.len(), 2);
    assert_eq!(mode_list[1].display_name, "JUMP");
    assert_eq!(mode_list[1].cursor_style, CursorStyle::Block);
    assert!(mode_list[1].accepts_char_input);
    assert!(!mode_list[1].is_entry);
}

/// Create a minimal `ModuleContext` for testing.
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_context(
    services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
) -> ModuleContext {
    use {
        parking_lot::RwLock,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, TextObjectEngine,
        },
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(reovim_driver_buffer::TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
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
