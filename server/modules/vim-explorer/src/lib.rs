#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim keybinding adapter for file explorer commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `explorer` (file tree sidebar). It maps the vim normal-mode
//! leader key to the explorer toggle:
//! - `<Space>e` → Toggle file explorer
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because
//! `explorer` should not know about vim's mode names:
//! - `explorer` provides commands, modes, and tree UI (no vim knowledge)
//! - `vim` provides modes and core keybindings (no explorer knowledge)
//! - This module bridges them with explicit, visible coupling

use {
    reovim_driver_input::KeybindingStore,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_explorer::ids as explorer,
};

const MODULE: ModuleId = ModuleId::new("vim-explorer");

/// Vim-explorer keybinding adapter module.
///
/// Registers the explorer toggle keybinding in `vim:normal` mode,
/// targeting the toggle command defined in `explorer`.
pub struct VimExplorerModule;

impl VimExplorerModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimExplorerModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimExplorerModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Explorer Keybindings"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn dependencies(&self) -> Vec<ModuleId> {
        vec![ModuleId::new("vim"), ModuleId::new("explorer")]
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let store = ctx.services.get_or_create::<KeybindingStore>();
        store.add_all(self.keybindings());
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            KeybindingRegistration::new("<Space>e", explorer::TOGGLE)
                .with_modes(&["vim:normal"])
                .with_category("explorer")
                .with_description("Toggle file explorer"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimExplorerModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
