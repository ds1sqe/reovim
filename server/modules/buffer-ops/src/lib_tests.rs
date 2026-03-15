use super::*;

#[test]
fn test_module_id() {
    let module = BufferOps::new();
    assert_eq!(module.id().as_str(), "buffer-ops");
}

#[test]
fn test_module_version() {
    let module = BufferOps::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_module_name() {
    let module = BufferOps::new();
    assert_eq!(module.name(), "Buffer Operations");
}

#[test]
fn test_default_creates_same_as_new() {
    let from_new = BufferOps::new();
    let from_default = BufferOps::default();
    assert_eq!(from_new.id(), from_default.id());
    assert_eq!(from_new.version(), from_default.version());
    assert_eq!(from_new.name(), from_default.name());
}

#[test]
fn test_new_has_empty_subscriptions() {
    let module = BufferOps::new();
    // Subscriptions start empty before init()
    assert!(module.subscriptions.is_empty());
}

#[test]
fn test_exit_clears_subscriptions() {
    let mut module = BufferOps::new();
    // Before init, subscriptions are empty, exit should still succeed
    let result = module.exit();
    assert!(result.is_ok());
}

#[test]
fn test_dependencies_default_empty() {
    let module = BufferOps::new();
    // Module trait has default empty dependencies
    assert!(module.dependencies().is_empty());
}

/// Helper to create a `ModuleContext` for testing.
fn make_test_module_context() -> ModuleContext {
    use {
        reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry},
        std::path::PathBuf,
    };

    let event_bus = Arc::new(EventBus::new());
    let services = Arc::new(ServiceRegistry::new());
    let kernel = KernelContext::with_event_bus_services_and_options(
        event_bus,
        Arc::clone(&services),
        Arc::new(OptionRegistry::new()),
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
    let mut module = BufferOps::new();
    let result = module.init(&ctx);

    assert_eq!(result, ProbeResult::Success);
    // Should have 4 subscriptions: BufferCreated, BufferModified, BufferClosed, BufferSwitched
    assert_eq!(module.subscriptions.len(), 4);
}

#[test]
fn test_init_then_exit_clears_all_subscriptions() {
    let ctx = make_test_module_context();
    let mut module = BufferOps::new();
    module.init(&ctx);
    assert_eq!(module.subscriptions.len(), 4);

    module.exit().unwrap();
    assert!(module.subscriptions.is_empty());
}

#[test]
fn test_double_exit_is_safe() {
    let ctx = make_test_module_context();
    let mut module = BufferOps::new();
    module.init(&ctx);
    assert_eq!(module.subscriptions.len(), 4);

    assert!(module.exit().is_ok());
    assert!(module.subscriptions.is_empty());

    assert!(module.exit().is_ok());
    assert!(module.subscriptions.is_empty());
}

#[test]
fn test_init_returns_success() {
    let ctx = make_test_module_context();
    let mut module = BufferOps::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);
}

#[test]
fn test_event_bus_receives_buffer_created() {
    use reovim_kernel::api::v1::events::kernel::BufferCreated;

    let ctx = make_test_module_context();
    let mut module = BufferOps::new();
    module.init(&ctx);

    // Emit a BufferCreated event
    let event = BufferCreated { buffer_id: 1 };
    ctx.kernel.event_bus.emit(event);
    // Subscription handler was called (no panic)
}

#[test]
fn test_event_bus_receives_buffer_modified() {
    use reovim_kernel::api::v1::events::kernel::{BufferModified, Modification};

    let ctx = make_test_module_context();
    let mut module = BufferOps::new();
    module.init(&ctx);

    let event = BufferModified {
        buffer_id: 1,
        modification: Modification::Insert {
            start: (0, 0),
            text: "hello".to_string(),
            start_byte: 0,
        },
    };
    ctx.kernel.event_bus.emit(event);
}

#[test]
fn test_event_bus_receives_buffer_closed() {
    use reovim_kernel::api::v1::events::kernel::BufferClosed;

    let ctx = make_test_module_context();
    let mut module = BufferOps::new();
    module.init(&ctx);

    let event = BufferClosed { buffer_id: 1 };
    ctx.kernel.event_bus.emit(event);
}

#[test]
fn test_event_bus_receives_buffer_switched() {
    use reovim_kernel::api::v1::events::kernel::BufferSwitched;

    let ctx = make_test_module_context();
    let mut module = BufferOps::new();
    module.init(&ctx);

    let event = BufferSwitched {
        from: Some(1),
        to: 2,
    };
    ctx.kernel.event_bus.emit(event);
}

#[test]
fn test_event_bus_receives_buffer_switched_from_none() {
    use reovim_kernel::api::v1::events::kernel::BufferSwitched;

    let ctx = make_test_module_context();
    let mut module = BufferOps::new();
    module.init(&ctx);

    let event = BufferSwitched { from: None, to: 1 };
    ctx.kernel.event_bus.emit(event);
}

#[test]
fn test_reinit_after_exit() {
    let ctx = make_test_module_context();
    let mut module = BufferOps::new();

    // First init/exit cycle
    module.init(&ctx);
    assert_eq!(module.subscriptions.len(), 4);
    module.exit().unwrap();
    assert!(module.subscriptions.is_empty());

    // Second init/exit cycle
    module.init(&ctx);
    assert_eq!(module.subscriptions.len(), 4);
    module.exit().unwrap();
    assert!(module.subscriptions.is_empty());
}
