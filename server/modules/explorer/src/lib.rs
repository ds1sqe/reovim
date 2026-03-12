#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! File explorer module for reovim - POLICY layer.
//!
//! Provides a sidebar tree view for navigating and managing files.
//! Follows the server module pattern (like microscope): state is managed
//! server-side, serialized to JSON via bridge, and rendered client-side
//! by the TUI extension.
//!
//! # Architecture (#523)
//!
//! This module owns the per-client `ExplorerState` stored in `ExtensionMap`.
//! The bridge serializes state to JSON consumed by both TUI and Web extensions.

pub mod bridge;
pub mod commands;
pub mod ids;
pub mod modes;
pub mod resolver;
pub mod state;
pub mod tree;

pub use {bridge::ExplorerBridge, state::ExplorerState};

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

/// File explorer sidebar module.
///
/// Registers [`ExplorerBridge`] during `init()`, along with modes,
/// commands, and keybindings for the file explorer.
pub struct ExplorerModule;

impl ExplorerModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ExplorerModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ExplorerModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "Explorer"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register ExplorerBridge via BridgeProvider.
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(ExplorerBridge);

        // Register modes.
        let mode_store = ctx.services.get_or_create::<ModeInfoStore>();
        for mode in modes::ExplorerMode::ALL {
            mode_store.add(ModeInfo::from_mode(*mode));
        }

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Register key resolvers.
        let resolver_registry = ctx.services.get_or_create::<ResolverRegistry>();
        resolver_registry.register(resolver::BrowseResolver::new());
        resolver_registry.register(resolver::InputResolver::new());

        // Register keybindings.
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // Browse mode: toggle off
            KeybindingRegistration::new("<Space>e", ids::TOGGLE)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Toggle file explorer"),
            // Browse mode: close
            KeybindingRegistration::new("q", ids::CLOSE)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Close explorer"),
            KeybindingRegistration::new("<Esc>", ids::CLOSE)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Close explorer"),
            // Browse mode: navigation
            KeybindingRegistration::new("k", ids::CURSOR_UP)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Cursor up"),
            KeybindingRegistration::new("<Up>", ids::CURSOR_UP)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Cursor up"),
            KeybindingRegistration::new("j", ids::CURSOR_DOWN)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Cursor down"),
            KeybindingRegistration::new("<Down>", ids::CURSOR_DOWN)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Cursor down"),
            KeybindingRegistration::new("gg", ids::GOTO_FIRST)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Go to first"),
            KeybindingRegistration::new("G", ids::GOTO_LAST)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Go to last"),
            KeybindingRegistration::new("l", ids::EXPAND)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Expand directory"),
            KeybindingRegistration::new("<Right>", ids::EXPAND)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Expand directory"),
            KeybindingRegistration::new("h", ids::COLLAPSE)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Collapse directory"),
            KeybindingRegistration::new("<Left>", ids::COLLAPSE)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Collapse directory"),
            KeybindingRegistration::new("<CR>", ids::OPEN)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Open file / toggle directory"),
            KeybindingRegistration::new("-", ids::GOTO_PARENT)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Go to parent"),
            KeybindingRegistration::new("H", ids::TOGGLE_HIDDEN)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Toggle hidden files"),
            KeybindingRegistration::new("R", ids::REFRESH)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Refresh tree"),
            // Browse mode: file operations
            KeybindingRegistration::new("a", ids::CREATE_FILE)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Create file"),
            KeybindingRegistration::new("A", ids::CREATE_DIR)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Create directory"),
            KeybindingRegistration::new("r", ids::RENAME)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Rename"),
            KeybindingRegistration::new("d", ids::DELETE)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Delete"),
            KeybindingRegistration::new("y", ids::YANK_PATH)
                .with_modes(&["explorer:EXPLORER"])
                .with_description("Copy path to clipboard"),
            // Input mode: actions
            KeybindingRegistration::new("<CR>", ids::CONFIRM_INPUT)
                .with_modes(&["explorer:EXPLORER_INPUT"])
                .with_description("Confirm input"),
            KeybindingRegistration::new("<Esc>", ids::CANCEL_INPUT)
                .with_modes(&["explorer:EXPLORER_INPUT"])
                .with_description("Cancel input"),
            KeybindingRegistration::new("<BS>", ids::INPUT_BACKSPACE)
                .with_modes(&["explorer:EXPLORER_INPUT"])
                .with_description("Delete character"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ExplorerModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
