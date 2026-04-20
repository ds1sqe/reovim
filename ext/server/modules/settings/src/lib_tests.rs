use super::*;

use {
    reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
    std::{path::PathBuf, sync::Arc},
};

#[test]
fn test_module_id() {
    let module = SettingsModule::new();
    assert_eq!(module.id().as_str(), "settings");
}

#[test]
fn test_module_name() {
    let module = SettingsModule::new();
    assert_eq!(module.name(), "Interactive Settings");
}

#[test]
fn test_module_version() {
    let module = SettingsModule::new();
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
    let from_default: SettingsModule = create_default();
    let from_new = SettingsModule::new();
    assert_eq!(from_new.id(), from_default.id());
}

#[test]
fn test_module_exit() {
    let mut module = SettingsModule::new();
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

    let mut module = SettingsModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);
}

#[test]
fn test_module_extension_kinds() {
    let module = SettingsModule::new();
    let kinds = module.extension_kinds();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0], "settings");
}
