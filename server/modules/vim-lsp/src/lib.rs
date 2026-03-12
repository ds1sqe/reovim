#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim keybinding adapter for LSP navigation commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `lsp-navigation` (code intelligence). It maps vim normal-mode
//! keys to LSP commands:
//! - `gd` → Go to Definition
//! - `gr` → Find References
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because neither
//! `vim` nor `lsp-navigation` should know about each other:
//! - `lsp-navigation` provides commands and picker (no keybinding knowledge)
//! - `vim` provides modes and core keybindings (no LSP knowledge)
//! - This module bridges them with explicit, visible coupling

use {
    reovim_driver_input::KeybindingStore,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_lsp_navigation::ids as lsp_nav,
};

const MODULE: ModuleId = ModuleId::new("vim-lsp");

/// Vim-LSP keybinding adapter module.
///
/// Registers `gd` and `gr` keybindings in `vim:normal` mode,
/// targeting LSP navigation commands defined in `lsp-navigation`.
pub struct VimLspModule;

impl VimLspModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimLspModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimLspModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim LSP Keybindings"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
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
            KeybindingRegistration::new("gd", lsp_nav::GOTO_DEFINITION)
                .with_modes(&["vim:normal"])
                .with_category("lsp")
                .with_description("Go to definition (LSP)"),
            KeybindingRegistration::new("gr", lsp_nav::REFERENCES)
                .with_modes(&["vim:normal"])
                .with_category("lsp")
                .with_description("Find references (LSP)"),
            KeybindingRegistration::new("K", lsp_nav::HOVER)
                .with_modes(&["vim:normal"])
                .with_category("lsp")
                .with_description("Show hover information (LSP)"),
            KeybindingRegistration::new("<C-k>", lsp_nav::SIGNATURE_HELP)
                .with_modes(&["vim:insert"])
                .with_category("lsp")
                .with_description("Show signature help (LSP)"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimLspModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
