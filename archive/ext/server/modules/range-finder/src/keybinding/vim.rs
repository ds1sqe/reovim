//! Vim personality keybinding adapter for range-finder (#700).
//!
//! Provides jump navigation and fold keybindings with qualified
//! `"vim:normal"` mode strings.  The `reovim-module-vim` dependency makes
//! this coupling explicit and compile-time enforced.

// The import of reovim_module_vim ensures the vim module exists in the
// dependency graph. This IS the declaration of personality support.
use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::{fold::ids as fold_ids, jump::ids as jump_ids};

/// Vim personality keybindings for range-finder.
///
/// All bindings use qualified `"vim:normal"` mode.
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        // Jump navigation
        KeybindingRegistration::new("s", jump_ids::JUMP_SEARCH)
            .with_modes(&["vim:normal"])
            .with_description("Jump search forward"),
        KeybindingRegistration::new("S", jump_ids::JUMP_SEARCH_BACKWARD)
            .with_modes(&["vim:normal"])
            .with_description("Jump search backward"),
        // Fold operations
        KeybindingRegistration::new("za", fold_ids::FOLD_TOGGLE)
            .with_modes(&["vim:normal"])
            .with_description("Toggle fold at cursor"),
        KeybindingRegistration::new("zo", fold_ids::FOLD_OPEN)
            .with_modes(&["vim:normal"])
            .with_description("Open fold at cursor"),
        KeybindingRegistration::new("zc", fold_ids::FOLD_CLOSE)
            .with_modes(&["vim:normal"])
            .with_description("Close fold at cursor"),
        KeybindingRegistration::new("zR", fold_ids::FOLD_OPEN_ALL)
            .with_modes(&["vim:normal"])
            .with_description("Open all folds"),
        KeybindingRegistration::new("zM", fold_ids::FOLD_CLOSE_ALL)
            .with_modes(&["vim:normal"])
            .with_description("Close all folds"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
