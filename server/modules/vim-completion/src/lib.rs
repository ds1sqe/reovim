#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim keybinding adapter for completion commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `completion` (completion engine). It maps vim insert-mode keys
//! to completion commands:
//! - `<C-y>` → Confirm completion
//! - `<C-e>` → Dismiss completion
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because
//! `completion` should not know about vim's mode names:
//! - `completion` provides commands and completion engine (no vim knowledge)
//! - `vim` provides modes and core keybindings (no completion knowledge)
//! - This module bridges them with explicit, visible coupling

use {
    reovim_driver_input::KeybindingStore,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_completion::ids as completion,
};

const MODULE: ModuleId = ModuleId::new("vim-completion");

/// Vim-completion keybinding adapter module.
///
/// Registers completion confirm/dismiss keybindings in `vim:insert` mode,
/// targeting completion commands defined in `completion`.
pub struct VimCompletionModule;

impl VimCompletionModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimCompletionModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimCompletionModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Completion Keybindings"
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
            KeybindingRegistration::new("<C-y>", completion::CONFIRM)
                .with_modes(&["vim:insert"])
                .with_category("completion")
                .with_description("Confirm completion"),
            KeybindingRegistration::new("<C-e>", completion::DISMISS)
                .with_modes(&["vim:insert"])
                .with_category("completion")
                .with_description("Dismiss completion"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimCompletionModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
