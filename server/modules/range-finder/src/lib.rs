#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Range-finder module - POLICY layer.
//!
//! This module provides jump navigation and code folding:
//! - **Jump navigation**: `s{char}{char}` two-char search with label overlay
//! - **Code folding**: `za`/`zo`/`zc`/`zR`/`zM` fold commands
//!
//! # Architecture (#524)
//!
//! Jump state (`JumpSessionState`) and fold state (`FoldSessionState`) are
//! independent `SessionExtension` types. Jump labels are rendered by client
//! extensions via `ExtensionStateBridge`.
//!
//! The module registers its own mode (`range-finder:jump-input`) for label
//! selection. The parent mode for keybinding inheritance is injected by
//! the adapter module (e.g., `vim-range-finder`) via [`JumpParentMode`].

pub mod config;
pub mod fold;
pub mod jump;

pub use config::JumpParentMode;

use {
    reovim_driver_input::{ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_kernel::api::v1::{
        CursorStyle, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

const MODULE_ID: ModuleId = ModuleId::new("range-finder");

/// Range-finder module providing jump navigation and code folding.
pub struct RangeFinderModule;

impl RangeFinderModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RangeFinderModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for RangeFinderModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn name(&self) -> &'static str {
        "range-finder"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register bridges (#524)
        let provider = ctx
            .services
            .get_or_create::<reovim_driver_session::bridges::BridgeProvider>();
        provider.register(jump::bridge::JumpBridge);
        provider.register(fold::bridge::FoldBridge);

        // Register jump commands (#524)
        let command_store = ctx
            .services
            .get_or_create::<reovim_driver_command::CommandHandlerStore>();
        for handler in jump::command::all_commands() {
            command_store.add(handler);
        }

        // Register fold commands (#524)
        for handler in fold::command::all_commands() {
            command_store.add(handler);
        }

        // Read parent mode from adapter-injected config (e.g., vim-range-finder)
        let parent_mode = ctx
            .services
            .get::<JumpParentMode>()
            .expect("JumpParentMode must be registered (by adapter) before range-finder")
            .mode()
            .clone();
        let modes = ctx.services.get_or_create::<ModeInfoStore>();

        // Register jump resolver for jump-input mode (#524)
        let resolvers = ctx.services.get_or_create::<ResolverRegistry>();
        resolvers.register(jump::resolver::JumpResolver::with_parent(parent_mode.clone()));

        // Register mode info for jump-input mode (#524)
        modes.add(ModeInfo {
            id: jump::ids::JUMP_INPUT_MODE,
            display_name: "JUMP",
            cursor_style: CursorStyle::Block,
            accepts_char_input: true,
            has_selection: false,
            inherits_from: Some(parent_mode),
            is_entry: false,
        });

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(RangeFinderModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
