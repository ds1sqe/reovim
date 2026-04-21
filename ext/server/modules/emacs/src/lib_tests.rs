use {
    super::*,
    reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
    std::{path::PathBuf, sync::Arc},
};

#[test]
fn test_module_id() {
    let module = EmacsModule::new();
    assert_eq!(module.id().as_str(), "emacs");
}

#[test]
fn test_module_name() {
    let module = EmacsModule::new();
    assert_eq!(module.name(), "Emacs Personality");
}

#[test]
fn test_module_version() {
    let module = EmacsModule::new();
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
    let from_default: EmacsModule = create_default();
    let from_new = EmacsModule::new();
    assert_eq!(from_new.id(), from_default.id());
    assert_eq!(from_new.version(), from_default.version());
}

#[test]
fn test_module_exit() {
    let mut module = EmacsModule::new();
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

    let mut module = EmacsModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);
}

#[test]
fn test_init_registers_initial_mode_provider() {
    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = EmacsModule::new();
    module.init(&ctx);

    let provider = services
        .get::<InitialModeProvider>()
        .expect("InitialModeProvider should be registered");
    let mode = provider.get().expect("initial mode should be set");
    assert_eq!(mode, ModeId::new(ModuleId::new("emacs"), "default"));
}

#[test]
fn test_provides_mode_management() {
    let module = EmacsModule::new();
    let caps = module.provides();
    assert!(caps.contains(&reovim_subsys_input::capabilities::MODE_MANAGEMENT));
}

#[test]
fn test_keybindings_non_empty() {
    let module = EmacsModule::new();
    let bindings = module.keybindings();
    assert!(!bindings.is_empty());
}
