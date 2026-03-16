use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{KernelContext, Module, ModuleContext, ServiceRegistry},
    std::{path::PathBuf, sync::Arc},
};

use super::*;

#[test]
fn test_module_id() {
    let module = IlluminateModule::new();
    assert_eq!(module.id().as_str(), "illuminate");
}

#[test]
fn test_module_name() {
    let module = IlluminateModule::new();
    assert_eq!(module.name(), "Illuminate");
}

#[test]
fn test_module_version() {
    let module = IlluminateModule::new();
    assert_eq!(module.version(), Version::new(0, 1, 0));
}

#[test]
fn test_module_extension_kinds() {
    let module = IlluminateModule::new();
    assert_eq!(module.extension_kinds(), &["illuminate"]);
}

#[test]
fn test_module_default() {
    let module = <IlluminateModule as Default>::default();
    assert_eq!(module.id().as_str(), "illuminate");
}

#[test]
fn test_module_exit() {
    let mut module = IlluminateModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_module_init() {
    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = IlluminateModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    // Verify bridge was registered
    let bridge_provider = services.get::<BridgeProvider>();
    assert!(bridge_provider.is_some(), "BridgeProvider should be registered");

    // Verify commands were registered
    let store = services.get::<CommandHandlerStore>();
    assert!(store.is_some(), "CommandHandlerStore should be registered");
    let handlers = store.unwrap().take_handlers();
    assert_eq!(handlers.len(), 2, "Both navigation commands should be registered");
}
