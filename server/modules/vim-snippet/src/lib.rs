#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim keybinding adapter for snippet commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `snippet` (snippet expansion engine). It maps vim mode keys
//! to snippet commands:
//! - `<C-s>` → Expand snippet at cursor (insert mode)
//! - `<Space>sc` → List available snippets (normal mode)
//! - `<Space>sr` → Reload snippet files (normal mode)
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because
//! `snippet` should not know about vim's mode names:
//! - `snippet` provides commands and snippet engine (no vim knowledge)
//! - `vim` provides modes and core keybindings (no snippet knowledge)
//! - This module bridges them with explicit, visible coupling
//!
//! Note: snippet's own-mode keybindings (`snippet:navigating`) remain
//! in the snippet module itself, as they target snippet's own mode.

use {
    reovim_driver_input::{KeybindingStore, ModeInfoStore},
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_snippet::{SnippetParentMode, ids as snippet},
};

const MODULE: ModuleId = ModuleId::new("vim-snippet");

/// Vim-snippet keybinding adapter module.
///
/// Registers snippet expansion and management keybindings in
/// `vim:insert` and `vim:normal` modes, targeting commands
/// defined in `snippet`.
pub struct VimSnippetModule;

impl VimSnippetModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimSnippetModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimSnippetModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Snippet Keybindings"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn dependencies(&self) -> Vec<ModuleId> {
        vec![ModuleId::new("vim")]
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Resolve vim:insert and register as SnippetParentMode for snippet
        let modes = ctx.services.get_or_create::<ModeInfoStore>();
        let vim_insert = modes
            .find_by_name("vim", "insert")
            .expect("vim:insert must be registered before vim-snippet");
        ctx.services
            .register(std::sync::Arc::new(SnippetParentMode::new(vim_insert)));

        let store = ctx.services.get_or_create::<KeybindingStore>();
        store.add_all(self.keybindings());
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            KeybindingRegistration::new("<C-s>", snippet::EXPAND)
                .with_modes(&["vim:insert"])
                .with_category("snippet")
                .with_description("Expand snippet at cursor"),
            KeybindingRegistration::new("<Space>sc", snippet::CATALOG)
                .with_modes(&["vim:normal"])
                .with_category("snippet")
                .with_description("List available snippets"),
            KeybindingRegistration::new("<Space>sr", snippet::RELOAD)
                .with_modes(&["vim:normal"])
                .with_category("snippet")
                .with_description("Reload snippet files"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimSnippetModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
