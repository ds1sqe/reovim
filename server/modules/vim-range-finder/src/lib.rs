#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim keybinding adapter for range-finder commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `range-finder` (jump navigation + code folding). It maps vim
//! normal-mode keys to range-finder commands:
//! - `s` → Jump search (two-char pattern)
//! - `za` → Toggle fold at cursor
//! - `zo` → Open fold at cursor
//! - `zc` → Close fold at cursor
//! - `zR` → Open all folds
//! - `zM` → Close all folds
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because
//! `range-finder` should not know about vim's mode names:
//! - `range-finder` provides commands and fold/jump logic (no vim knowledge)
//! - `vim` provides modes and core keybindings (no folding knowledge)
//! - This module bridges them with explicit, visible coupling

mod find_char;

use {
    reovim_driver_input::{KeybindingStore, ModeInfoStore},
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_range_finder::{JumpParentMode, fold::ids as fold, jump::ids as jump},
};

const MODULE: ModuleId = ModuleId::new("vim-range-finder");

/// Vim-range-finder keybinding adapter module.
///
/// Registers jump and fold keybindings in `vim:normal` mode,
/// targeting commands defined in `range-finder`.
pub struct VimRangeFinderModule;

impl VimRangeFinderModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimRangeFinderModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimRangeFinderModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Range Finder Keybindings"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Resolve vim:normal and register as JumpParentMode for range-finder
        let modes = ctx.services.get_or_create::<ModeInfoStore>();
        let vim_normal = modes
            .find_by_name("vim", "normal")
            .expect("vim:normal must be registered before vim-range-finder");
        ctx.services
            .register(std::sync::Arc::new(JumpParentMode::new(vim_normal)));

        let store = ctx.services.get_or_create::<KeybindingStore>();
        store.add_all(self.keybindings());

        // Register enhanced find-char command (#535).
        // This overrides vim's basic EXECUTE_FIND_CHAR handler in the command
        // registry (HashMap::insert replaces existing entries). When multiple
        // matches exist for f/F/t/T, jump labels are shown instead of
        // jumping to the first match.
        let command_store = ctx
            .services
            .get_or_create::<reovim_driver_command::CommandHandlerStore>();
        command_store.add(Box::new(find_char::EnhancedFindCharCommand));

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // Jump navigation
            KeybindingRegistration::new("s", jump::JUMP_SEARCH)
                .with_modes(&["vim:normal"])
                .with_category("jump")
                .with_description("Jump search (two-char pattern)"),
            // Fold operations
            KeybindingRegistration::new("za", fold::FOLD_TOGGLE)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Toggle fold at cursor"),
            KeybindingRegistration::new("zo", fold::FOLD_OPEN)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Open fold at cursor"),
            KeybindingRegistration::new("zc", fold::FOLD_CLOSE)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Close fold at cursor"),
            KeybindingRegistration::new("zR", fold::FOLD_OPEN_ALL)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Open all folds"),
            KeybindingRegistration::new("zM", fold::FOLD_CLOSE_ALL)
                .with_modes(&["vim:normal"])
                .with_category("folding")
                .with_description("Close all folds"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimRangeFinderModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
