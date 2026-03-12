use super::*;

#[test]
fn test_module_id() {
    let module = NotificationModule::new();
    assert_eq!(module.id().as_str(), "notification");
}

#[test]
fn test_module_name() {
    let module = NotificationModule::new();
    assert_eq!(module.name(), "notification");
}

#[test]
fn test_module_version() {
    let module = NotificationModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
    assert_eq!(version.patch, 0);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn test_module_default() {
    let module = NotificationModule::default();
    assert_eq!(module.id().as_str(), "notification");
}

#[test]
fn test_module_exit() {
    let mut module = NotificationModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_module_init_registers_bridge() {
    use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = NotificationModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Verify bridge was registered
    let provider = services.get::<BridgeProvider>().unwrap();
    let bridges = provider.take_bridges();
    assert_eq!(bridges.len(), 1);
    assert_eq!(bridges[0].kind(), "notification");
}

#[test]
fn test_module_init_twice_reuses_provider() {
    use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module1 = NotificationModule::new();
    let mut module2 = NotificationModule::new();
    let _ = module1.init(&ctx);
    let _ = module2.init(&ctx);

    // Both registrations go through get_or_create (second uses "get" path)
    let provider = services.get::<BridgeProvider>().unwrap();
    let bridges = provider.take_bridges();
    assert_eq!(bridges.len(), 2);
}

/// Create a minimal `ModuleContext` for testing.
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_context(
    services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
) -> ModuleContext {
    ModuleContext::new(
        reovim_kernel::api::v1::KernelContext::default(),
        services,
        std::path::PathBuf::from("/tmp"),
        std::path::PathBuf::from("/tmp"),
    )
}
