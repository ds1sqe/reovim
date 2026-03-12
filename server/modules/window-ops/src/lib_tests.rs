use super::*;

// =========================================================================
// WindowOps construction
// =========================================================================

#[test]
fn test_new_creates_empty_subscriptions() {
    let module = WindowOps::new();
    assert!(module.subscriptions.is_empty());
}

#[test]
fn test_default_creates_same_as_new() {
    let from_new = WindowOps::new();
    let from_default = WindowOps::default();
    assert_eq!(from_new.subscriptions.len(), from_default.subscriptions.len());
}

// =========================================================================
// Module trait implementation
// =========================================================================

#[test]
fn test_module_id() {
    let module = WindowOps::new();
    assert_eq!(module.id().as_str(), "window-ops");
}

#[test]
fn test_module_name() {
    let module = WindowOps::new();
    assert_eq!(module.name(), "Window Operations");
}

#[test]
fn test_module_version() {
    let module = WindowOps::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_module_version_is_pre_release() {
    let module = WindowOps::new();
    let version = module.version();
    // Version 0.x.y is pre-release
    assert_eq!(version.major, 0);
}

// =========================================================================
// Exit behavior
// =========================================================================

#[test]
fn test_exit_clears_subscriptions() {
    let mut module = WindowOps::new();
    // Initially empty
    assert!(module.subscriptions.is_empty());
    // Exit should succeed even with no subscriptions
    let result = module.exit();
    assert!(result.is_ok());
    assert!(module.subscriptions.is_empty());
}

#[test]
fn test_exit_returns_ok() {
    let mut module = WindowOps::new();
    assert!(module.exit().is_ok());
}

// =========================================================================
// CommandProvider implementation
// =========================================================================

#[test]
fn test_command_provider_returns_handlers() {
    let module = WindowOps::new();
    let handlers = module.command_handlers();
    assert_eq!(handlers.len(), 28);
}

#[test]
fn test_command_provider_matches_all_commands() {
    let module = WindowOps::new();
    let handlers = module.command_handlers();
    let all = command::all_commands();
    assert_eq!(handlers.len(), all.len());
}

// =========================================================================
// Init with real kernel context (event bus integration)
// =========================================================================

/// Helper to create a `ModuleContext` for testing.
fn make_test_module_context() -> ModuleContext {
    use {
        reovim_kernel::api::v1::{EventBus, KernelContext, ServiceRegistry},
        std::path::PathBuf,
    };

    let event_bus = Arc::new(EventBus::new());
    let services = Arc::new(ServiceRegistry::new());
    let kernel = KernelContext::with_event_bus_services_and_options(
        event_bus,
        Arc::clone(&services),
        Arc::new(reovim_kernel::api::v1::OptionRegistry::new()),
    );

    ModuleContext::new(
        kernel,
        services,
        PathBuf::from("/tmp/reovim-test/data"),
        PathBuf::from("/tmp/reovim-test/cache"),
    )
}

#[test]
fn test_init_creates_subscriptions() {
    let ctx = make_test_module_context();

    let mut module = WindowOps::new();
    let result = module.init(&ctx);

    assert_eq!(result, ProbeResult::Success);
    // Should have 4 subscriptions: WindowCreated, WindowClosed, WindowFocused, ViewportScrolled
    assert_eq!(module.subscriptions.len(), 4);
}

#[test]
fn test_init_then_exit_clears_all_subscriptions() {
    let ctx = make_test_module_context();

    let mut module = WindowOps::new();
    module.init(&ctx);
    assert_eq!(module.subscriptions.len(), 4);

    module.exit().unwrap();
    assert!(module.subscriptions.is_empty());
}

#[test]
fn test_init_registers_commands_in_service_registry() {
    let ctx = make_test_module_context();

    let mut module = WindowOps::new();
    module.init(&ctx);

    // The CommandHandlerStore should be accessible from services
    let store = ctx.services.get::<CommandHandlerStore>();
    assert!(store.is_some(), "CommandHandlerStore should be registered in services");
}

#[test]
fn test_double_exit_is_safe() {
    let ctx = make_test_module_context();

    let mut module = WindowOps::new();
    module.init(&ctx);
    assert_eq!(module.subscriptions.len(), 4);

    // First exit
    assert!(module.exit().is_ok());
    assert!(module.subscriptions.is_empty());

    // Second exit should also be safe (no-op)
    assert!(module.exit().is_ok());
    assert!(module.subscriptions.is_empty());
}

#[test]
fn test_event_handler_window_created() {
    use reovim_kernel::api::v1::events::kernel::WindowCreated;

    let ctx = make_test_module_context();
    let mut module = WindowOps::new();
    module.init(&ctx);

    // Publish a WindowCreated event to exercise the subscription closure
    ctx.kernel.event_bus.emit(WindowCreated { window_id: 1 });

    // If we get here without panicking, the handler ran successfully
    module.exit().unwrap();
}

#[test]
fn test_event_handler_window_closed() {
    use reovim_kernel::api::v1::events::kernel::WindowClosed;

    let ctx = make_test_module_context();
    let mut module = WindowOps::new();
    module.init(&ctx);

    ctx.kernel.event_bus.emit(WindowClosed { window_id: 1 });

    module.exit().unwrap();
}

#[test]
fn test_event_handler_window_focused() {
    use reovim_kernel::api::v1::events::kernel::WindowFocused;

    let ctx = make_test_module_context();
    let mut module = WindowOps::new();
    module.init(&ctx);

    ctx.kernel.event_bus.emit(WindowFocused {
        from: Some(1),
        to: 2,
    });

    module.exit().unwrap();
}

#[test]
fn test_event_handler_viewport_scrolled() {
    use reovim_kernel::api::v1::events::kernel::ViewportScrolled;

    let ctx = make_test_module_context();
    let mut module = WindowOps::new();
    module.init(&ctx);

    ctx.kernel.event_bus.emit(ViewportScrolled {
        window_id: 1,
        buffer_id: 1,
        top_line: 0,
        bottom_line: 50,
    });

    module.exit().unwrap();
}

#[test]
fn test_dependencies_default_empty() {
    let module = WindowOps::new();
    assert!(module.dependencies().is_empty());
}
