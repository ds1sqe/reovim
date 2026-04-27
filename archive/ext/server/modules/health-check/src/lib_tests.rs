use {
    super::*,
    reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
    std::{path::PathBuf, sync::Arc},
};

#[test]
fn test_module_id() {
    let module = HealthCheckModule::new();
    assert_eq!(module.id().as_str(), "health-check");
}

#[test]
fn test_module_name() {
    let module = HealthCheckModule::new();
    assert_eq!(module.name(), "Health Check");
}

#[test]
fn test_module_version() {
    let module = HealthCheckModule::new();
    let v = module.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn test_module_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let from_default: HealthCheckModule = create_default();
    let from_new = HealthCheckModule::new();
    assert_eq!(from_new.id(), from_default.id());
    assert_eq!(from_new.version(), from_default.version());
}

#[test]
fn test_module_exit() {
    let mut module = HealthCheckModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_module_init_registers_command() {
    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = HealthCheckModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    let store = services.get_or_create::<CommandHandlerStore>();
    assert!(!store.is_empty());
}
