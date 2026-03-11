use super::*;

#[test]
fn module_id() {
    let module = ExplorerModule::new();
    assert_eq!(module.id().as_str(), "explorer");
}

#[test]
fn module_name() {
    let module = ExplorerModule::new();
    assert_eq!(module.name(), "Explorer");
}

#[test]
fn module_version() {
    let module = ExplorerModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = ExplorerModule::default();
    assert_eq!(module.id().as_str(), "explorer");
}

#[test]
fn module_exit() {
    let mut module = ExplorerModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn module_init_registers_bridge_and_modes() {
    use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = ExplorerModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Verify bridge was registered.
    let provider = services.get::<BridgeProvider>().unwrap();
    let bridges = provider.take_bridges();
    assert_eq!(bridges.len(), 1);
    assert_eq!(bridges[0].kind(), "explorer");

    // Verify modes were registered.
    let mode_store = services.get::<ModeInfoStore>();
    assert!(mode_store.is_some());
    let mode_infos = mode_store.unwrap().take_modes();
    assert_eq!(mode_infos.len(), 2);

    let names: Vec<&str> = mode_infos.iter().map(|m| m.display_name).collect();
    assert!(names.contains(&"EXPLORER"));
    assert!(names.contains(&"EXPLORER_INPUT"));

    // Verify commands were registered.
    let command_store = services.get::<CommandHandlerStore>();
    assert!(command_store.is_some());

    // Verify resolvers were registered.
    let resolver_registry = services.get::<ResolverRegistry>();
    assert!(resolver_registry.is_some());

    // Verify keybindings were registered.
    let keybinding_store = services.get::<KeybindingStore>();
    assert!(keybinding_store.is_some());
}

#[test]
fn keybindings_not_empty() {
    let module = ExplorerModule::new();
    let bindings = module.keybindings();
    assert!(!bindings.is_empty());
}

#[test]
fn keybindings_have_browse_mode_actions() {
    let module = ExplorerModule::new();
    let bindings = module.keybindings();
    let count = bindings
        .iter()
        .filter(|b| b.modes.contains(&"explorer:EXPLORER"))
        .count();
    // <Space>e, q, Esc, k, Up, j, Down, gg, G, l, Right, h, Left, CR, -, H, R, a, A, r, d, y
    assert_eq!(count, 22);
}

#[test]
fn keybindings_have_input_mode_actions() {
    let module = ExplorerModule::new();
    let bindings = module.keybindings();
    let count = bindings
        .iter()
        .filter(|b| b.modes.contains(&"explorer:EXPLORER_INPUT"))
        .count();
    // CR, Esc, BS
    assert_eq!(count, 3);
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
    let module = ExplorerModule::new();
    assert_eq!(module.extension_kinds(), &["explorer"]);
}
