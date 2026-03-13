use {
    super::*,
    reovim_driver_input::{ModeInfoStore, ResolverRegistry},
    reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
    std::{path::PathBuf, sync::Arc},
};

#[test]
fn test_module_id() {
    let module = ModuleManagerModule::new();
    assert_eq!(module.id().as_str(), "module-manager");
}

#[test]
fn test_module_name() {
    let module = ModuleManagerModule::new();
    assert_eq!(module.name(), "Module Manager");
}

#[test]
fn test_module_version() {
    let module = ModuleManagerModule::new();
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
    let from_default: ModuleManagerModule = create_default();
    let from_new = ModuleManagerModule::new();
    assert_eq!(from_new.id(), from_default.id());
    assert_eq!(from_new.version(), from_default.version());
}

#[test]
fn test_module_exit() {
    let mut module = ModuleManagerModule::new();
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

    let mut module = ModuleManagerModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    let store = services.get_or_create::<CommandHandlerStore>();
    assert!(!store.is_empty());
}

#[test]
fn test_init_registers_modes() {
    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = ModuleManagerModule::new();
    module.init(&ctx);

    let mode_store = services.get_or_create::<ModeInfoStore>();
    assert!(!mode_store.is_empty());
}

#[test]
fn test_init_registers_resolver() {
    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = ModuleManagerModule::new();
    module.init(&ctx);

    let resolver_registry = services.get_or_create::<ResolverRegistry>();
    assert!(!resolver_registry.is_empty());
}

#[test]
fn test_extension_kinds() {
    let module = ModuleManagerModule::new();
    let kinds = module.extension_kinds();
    assert_eq!(kinds, &[reovim_extension_kinds::MODULE_MANAGER]);
}

#[test]
fn test_keybindings_non_empty() {
    let module = ModuleManagerModule::new();
    let bindings = module.keybindings();
    assert!(!bindings.is_empty());
}
