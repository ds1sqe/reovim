use super::*;

#[test]
fn module_id() {
    let module = MicroscopeModule::new();
    assert_eq!(module.id().as_str(), "microscope");
}

#[test]
fn module_name() {
    let module = MicroscopeModule::new();
    assert_eq!(module.name(), "Microscope");
}

#[test]
fn module_version() {
    let module = MicroscopeModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = MicroscopeModule::default();
    assert_eq!(module.id().as_str(), "microscope");
}

#[test]
fn module_exit() {
    let mut module = MicroscopeModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn module_init_registers_bridge_and_services() {
    use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = MicroscopeModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Verify bridge was registered.
    let provider = services.get::<BridgeProvider>().unwrap();
    let bridges = provider.take_bridges();
    assert_eq!(bridges.len(), 1);
    assert_eq!(bridges[0].kind(), "microscope");

    // Verify modes were registered.
    let mode_store = services.get::<ModeInfoStore>();
    assert!(mode_store.is_some());
    let modes = mode_store.unwrap().take_modes();
    assert_eq!(modes.len(), 1);
    assert_eq!(modes[0].display_name, "MICROSCOPE");

    // Verify commands were registered.
    let command_store = services.get::<CommandHandlerStore>();
    assert!(command_store.is_some());

    // Verify keybindings were registered.
    let keybinding_store = services.get::<KeybindingStore>();
    assert!(keybinding_store.is_some());

    // Verify resolver was registered.
    let resolver_registry = services.get::<ResolverRegistry>();
    assert!(resolver_registry.is_some());
    let reg = resolver_registry.unwrap();
    assert!(reg.get(&modes::MicroscopeMode::PICKER_ID).is_some());
}

#[test]
fn keybindings_not_empty() {
    let module = MicroscopeModule::new();
    let bindings = module.keybindings();
    assert!(!bindings.is_empty());
}

#[test]
fn keybindings_have_picker_mode_actions() {
    let module = MicroscopeModule::new();
    let bindings = module.keybindings();
    // C-n, Down, C-p, Up, CR, Esc, BS = 7
    let count = bindings
        .iter()
        .filter(|b| b.modes.contains(&"microscope:MICROSCOPE"))
        .count();
    assert_eq!(count, 7);
}

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

#[test]
fn test_extension_kinds() {
    let module = MicroscopeModule::new();
    assert_eq!(module.extension_kinds(), &["microscope"]);
}
