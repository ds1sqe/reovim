use super::*;

#[test]
fn test_module_id() {
    let module = CmdlineModule::new();
    assert_eq!(module.id().as_str(), "cmdline");
}

#[test]
fn test_module_name() {
    let module = CmdlineModule::new();
    assert_eq!(module.name(), "cmdline");
}

#[test]
fn test_module_version() {
    let module = CmdlineModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn test_module_default() {
    let module = CmdlineModule::default();
    assert_eq!(module.id().as_str(), "cmdline");
}

#[test]
fn test_module_exit() {
    let mut module = CmdlineModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_module_init_registers_bridge() {
    use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = CmdlineModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Verify bridge was registered
    let provider = services.get::<BridgeProvider>().unwrap();
    let bridges = provider.take_bridges();
    assert_eq!(bridges.len(), 1);
    assert_eq!(bridges[0].kind(), "cmdline");
}

/// Create a minimal `ModuleContext` for testing.
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_context(
    services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
) -> ModuleContext {
    use {
        parking_lot::RwLock,
        reovim_kernel::api::v1::{EventBus, KernelContext, MarkBank, OptionRegistry},
        reovim_types_text::{MotionEngine, TextObjectEngine},
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

#[test]
fn test_extension_kinds() {
    let module = CmdlineModule::new();
    assert_eq!(module.extension_kinds(), &["cmdline"]);
}
