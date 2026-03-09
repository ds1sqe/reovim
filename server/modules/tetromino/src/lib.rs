#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Tetromino game module for reovim - POLICY layer.
//!
//! Architecture proof-of-concept: validates that the module/bridge/extension
//! system supports non-text-editing use cases with zero kernel/driver changes.
//! Each client gets an independent game via per-client `ExtensionMap` state.

pub mod bridge;
pub mod commands;
pub mod game;
pub mod ids;
pub mod modes;
pub mod resolver;
pub mod state;

pub use {bridge::TetrominoBridge, state::TetrominoState};

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

/// Tetromino game module.
///
/// Registers [`TetrominoBridge`] during `init()`, along with modes,
/// commands, resolvers, and keybindings for the tetromino game.
pub struct TetrominoModule;

impl TetrominoModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TetrominoModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TetrominoModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Polyblocks"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register TetrominoBridge via BridgeProvider.
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(TetrominoBridge);

        // Register modes.
        let mode_store = ctx.services.get_or_create::<ModeInfoStore>();
        for mode in modes::TetrominoMode::ALL {
            mode_store.add(ModeInfo::from_mode(*mode));
        }

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Register key resolvers.
        let resolver_registry = ctx.services.get_or_create::<ResolverRegistry>();
        resolver_registry.register(resolver::PlayResolver::new());
        resolver_registry.register(resolver::PausedResolver::new());
        resolver_registry.register(resolver::MenuResolver::new());
        resolver_registry.register(resolver::LobbyResolver::new());
        resolver_registry.register(resolver::RoomResolver::new());
        resolver_registry.register(resolver::ResultResolver::new());

        // Register keybindings.
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        // Ensure TickSchedulerHandle exists for server layer to populate (#546).
        let _ = ctx
            .services
            .get_or_create::<reovim_driver_session::TickSchedulerHandle>();

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // Menu mode
            KeybindingRegistration::new("s", ids::START_SINGLE)
                .with_modes(&["tetromino:MENU"])
                .with_description("Single player"),
            KeybindingRegistration::new("m", ids::ENTER_LOBBY)
                .with_modes(&["tetromino:MENU"])
                .with_description("Multiplayer"),
            KeybindingRegistration::new("q", ids::QUIT)
                .with_modes(&["tetromino:MENU"])
                .with_description("Quit"),
            KeybindingRegistration::new("<Esc>", ids::QUIT)
                .with_modes(&["tetromino:MENU"])
                .with_description("Quit"),
            // Lobby mode
            KeybindingRegistration::new("c", ids::CREATE_ROOM)
                .with_modes(&["tetromino:LOBBY"])
                .with_description("Create room"),
            KeybindingRegistration::new("q", ids::LEAVE_LOBBY)
                .with_modes(&["tetromino:LOBBY"])
                .with_description("Back to menu"),
            KeybindingRegistration::new("<Esc>", ids::LEAVE_LOBBY)
                .with_modes(&["tetromino:LOBBY"])
                .with_description("Back to menu"),
            // Room mode
            KeybindingRegistration::new("r", ids::READY_TOGGLE)
                .with_modes(&["tetromino:ROOM"])
                .with_description("Toggle ready"),
            KeybindingRegistration::new("q", ids::LEAVE_ROOM)
                .with_modes(&["tetromino:ROOM"])
                .with_description("Leave room"),
            KeybindingRegistration::new("<Esc>", ids::LEAVE_ROOM)
                .with_modes(&["tetromino:ROOM"])
                .with_description("Leave room"),
            // Play mode: movement
            KeybindingRegistration::new("h", ids::MOVE_LEFT)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Move piece left"),
            KeybindingRegistration::new("<Left>", ids::MOVE_LEFT)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Move piece left"),
            KeybindingRegistration::new("l", ids::MOVE_RIGHT)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Move piece right"),
            KeybindingRegistration::new("<Right>", ids::MOVE_RIGHT)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Move piece right"),
            KeybindingRegistration::new("j", ids::SOFT_DROP)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Soft drop"),
            KeybindingRegistration::new("<Down>", ids::SOFT_DROP)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Soft drop"),
            KeybindingRegistration::new("k", ids::ROTATE_CW)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Rotate clockwise"),
            KeybindingRegistration::new("<Up>", ids::ROTATE_CW)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Rotate clockwise"),
            KeybindingRegistration::new("z", ids::ROTATE_CCW)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Rotate counter-clockwise"),
            KeybindingRegistration::new("c", ids::HOLD)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Hold piece"),
            KeybindingRegistration::new("<Space>", ids::HARD_DROP)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Hard drop"),
            // Play mode: control
            KeybindingRegistration::new("p", ids::PAUSE)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Pause game"),
            KeybindingRegistration::new("q", ids::QUIT)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Quit game"),
            KeybindingRegistration::new("<Esc>", ids::QUIT)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Quit game"),
            KeybindingRegistration::new("r", ids::RESTART)
                .with_modes(&["tetromino:PLAY"])
                .with_description("Restart game"),
            // Paused mode: limited controls
            KeybindingRegistration::new("p", ids::PAUSE)
                .with_modes(&["tetromino:PAUSED"])
                .with_description("Unpause game"),
            KeybindingRegistration::new("q", ids::QUIT)
                .with_modes(&["tetromino:PAUSED"])
                .with_description("Quit game"),
            KeybindingRegistration::new("<Esc>", ids::QUIT)
                .with_modes(&["tetromino:PAUSED"])
                .with_description("Quit game"),
            // Result mode
            KeybindingRegistration::new("q", ids::RETURN_LOBBY)
                .with_modes(&["tetromino:RESULT"])
                .with_description("Return to lobby"),
            KeybindingRegistration::new("<Esc>", ids::RETURN_LOBBY)
                .with_modes(&["tetromino:RESULT"])
                .with_description("Return to lobby"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(TetrominoModule);

#[cfg(test)]
mod tests {
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
}
