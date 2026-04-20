use super::*;

use {
    reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
    std::{path::PathBuf, sync::Arc},
};

#[test]
fn test_module_id() {
    let module = ContextModule::new();
    assert_eq!(module.id().as_str(), "context");
}

#[test]
fn test_module_name() {
    let module = ContextModule::new();
    assert_eq!(module.name(), "Context");
}

#[test]
fn test_module_version() {
    let module = ContextModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_module_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let from_default: ContextModule = create_default();
    let from_new = ContextModule::new();
    assert_eq!(from_new.id(), from_default.id());
}

#[test]
fn test_module_exit() {
    let mut module = ContextModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_module_init() {
    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services,
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = ContextModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);
}
