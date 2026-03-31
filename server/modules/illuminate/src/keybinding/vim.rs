//! Vim personality keybinding adapter for illuminate (#700).
//!
//! Provides reference navigation keybindings with qualified `"vim:normal"`
//! mode strings.  The `reovim-module-vim` dependency makes this coupling
//! explicit and compile-time enforced.

// The import of reovim_module_vim ensures the vim module exists in the
// dependency graph. This IS the declaration of personality support.
use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids;

/// Vim personality keybindings for illuminate.
///
/// All bindings use qualified `"vim:normal"` mode.
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("]]", ids::NEXT_REFERENCE)
            .with_modes(&["vim:normal"])
            .with_description("Next reference"),
        KeybindingRegistration::new("[[", ids::PREV_REFERENCE)
            .with_modes(&["vim:normal"])
            .with_description("Previous reference"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
