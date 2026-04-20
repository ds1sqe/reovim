#![cfg_attr(coverage_nightly, allow(unused_features))]
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

const KIND: &str = "polyblocks";

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_text_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_text_session::bridges::BridgeProvider,
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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
            .get_or_create::<reovim_driver_text_session::TickSchedulerHandle>();

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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
#[path = "lib_tests.rs"]
mod tests;
