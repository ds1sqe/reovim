//! Vim personality keybinding adapter for completion (#700).
//!
//! Provides completion keybindings with qualified `"vim:insert"` mode strings.
//! The `reovim-module-vim` dependency makes this coupling explicit and
//! compile-time enforced.

// The import of reovim_module_vim ensures the vim module exists in the
// dependency graph. This IS the declaration of personality support.
use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids;

/// Vim personality keybindings for completion.
///
/// All bindings use qualified `"vim:insert"` mode.
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("<C-y>", ids::CONFIRM)
            .with_modes(&["vim:insert"])
            .with_description("Confirm completion"),
        KeybindingRegistration::new("<C-e>", ids::DISMISS)
            .with_modes(&["vim:insert"])
            .with_description("Dismiss completion"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
