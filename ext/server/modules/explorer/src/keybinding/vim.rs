//! Vim personality keybinding adapter for explorer (#700).
//!
//! Provides explorer keybindings with qualified `"vim:normal"` mode strings.
//! The `reovim-module-vim` dependency makes this coupling explicit and
//! compile-time enforced.

// The import of reovim_module_vim ensures the vim module exists in the
// dependency graph. This IS the declaration of personality support.
use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids;

/// Vim personality keybindings for explorer.
///
/// All bindings use qualified `"vim:normal"` mode and `<leader>` notation
/// (expanded by bootstrap via `LeaderKeyProvider`).
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("<leader>e", ids::TOGGLE)
            .with_modes(&["vim:normal"])
            .with_description("Toggle file explorer"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
