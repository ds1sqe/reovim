use super::*;

#[test]
fn module_id() {
    let module = TetrominoModule::new();
    assert_eq!(module.id().as_str(), "tetromino");
}

#[test]
fn module_name() {
    let module = TetrominoModule::new();
    assert_eq!(module.name(), "Polyblocks");
}

#[test]
fn module_version() {
    let module = TetrominoModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = TetrominoModule::default();
    assert_eq!(module.id().as_str(), "tetromino");
}

#[test]
fn module_exit() {
    let mut module = TetrominoModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn module_init_registers_all() {
    use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

    let services = Arc::new(ServiceRegistry::new());
    let ctx = test_module_context(services.clone());

    let mut module = TetrominoModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Verify bridge was registered.
    let provider = services.get::<BridgeProvider>().unwrap();
    let bridges = provider.take_bridges();
    assert_eq!(bridges.len(), 1);
    assert_eq!(bridges[0].kind(), "polyblocks");

    // Verify modes were registered.
    let mode_store = services.get::<ModeInfoStore>();
    assert!(mode_store.is_some());
    let mode_infos = mode_store.unwrap().take_modes();
    assert_eq!(mode_infos.len(), 6);

    let names: Vec<&str> = mode_infos.iter().map(|m| m.display_name).collect();
    assert!(names.contains(&"PLAY"));
    assert!(names.contains(&"PAUSED"));
    assert!(names.contains(&"MENU"));
    assert!(names.contains(&"LOBBY"));
    assert!(names.contains(&"ROOM"));
    assert!(names.contains(&"RESULT"));

    // Verify commands were registered.
    let command_store = services.get::<CommandHandlerStore>();
    assert!(command_store.is_some());

    // Verify resolvers were registered.
    let resolver_registry = services.get::<ResolverRegistry>();
    assert!(resolver_registry.is_some());

    // Verify keybindings were registered.
    let keybinding_store = services.get::<KeybindingStore>();
    assert!(keybinding_store.is_some());

    // Verify TickSchedulerHandle was registered (#546).
    let tick_handle = services.get::<reovim_driver_session::TickSchedulerHandle>();
    assert!(tick_handle.is_some());
}

#[test]
fn keybindings_not_empty() {
    let module = TetrominoModule::new();
    let bindings = module.keybindings();
    assert!(!bindings.is_empty());
}

#[test]
fn keybindings_play_mode_count() {
    let module = TetrominoModule::new();
    let bindings = module.keybindings();
    let count = bindings
        .iter()
        .filter(|b| b.modes.contains(&"tetromino:PLAY"))
        .count();
    // h, Left, l, Right, j, Down, k, Up, z, c, Space, p, q, Esc, r = 15
    assert_eq!(count, 15);
}

#[test]
fn keybindings_paused_mode_count() {
    let module = TetrominoModule::new();
    let bindings = module.keybindings();
    let count = bindings
        .iter()
        .filter(|b| b.modes.contains(&"tetromino:PAUSED"))
        .count();
    // p, q, Esc = 3
    assert_eq!(count, 3);
}

#[test]
fn keybindings_menu_mode_count() {
    let module = TetrominoModule::new();
    let bindings = module.keybindings();
    let count = bindings
        .iter()
        .filter(|b| b.modes.contains(&"tetromino:MENU"))
        .count();
    // s, m, q, Esc = 4
    assert_eq!(count, 4);
}

#[test]
fn keybindings_lobby_mode_count() {
    let module = TetrominoModule::new();
    let bindings = module.keybindings();
    let count = bindings
        .iter()
        .filter(|b| b.modes.contains(&"tetromino:LOBBY"))
        .count();
    // c, q, Esc = 3
    assert_eq!(count, 3);
}

#[test]
fn keybindings_room_mode_count() {
    let module = TetrominoModule::new();
    let bindings = module.keybindings();
    let count = bindings
        .iter()
        .filter(|b| b.modes.contains(&"tetromino:ROOM"))
        .count();
    // r, q, Esc = 3
    assert_eq!(count, 3);
}

#[test]
fn keybindings_result_mode_count() {
    let module = TetrominoModule::new();
    let bindings = module.keybindings();
    let count = bindings
        .iter()
        .filter(|b| b.modes.contains(&"tetromino:RESULT"))
        .count();
    // q, Esc = 2
    assert_eq!(count, 2);
}

#[test]
fn no_vim_normal_keybindings() {
    let module = TetrominoModule::new();
    let bindings = module.keybindings();
    // No vim:normal bindings — use `:polyblocks` ex-command instead (#547)
    assert!(!bindings.iter().any(|b| b.modes.contains(&"vim:normal")));
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
