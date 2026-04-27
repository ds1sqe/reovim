//! Vim personality keybinding adapter for microscope (#700).
//!
//! Provides microscope keybindings with qualified `"vim:normal"` mode strings.
//! The `reovim-module-vim` dependency makes this coupling explicit and
//! compile-time enforced.

// The import of reovim_module_vim ensures the vim module exists in the
// dependency graph. This IS the declaration of personality support.
use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids;

/// Vim personality keybindings for microscope.
///
/// All bindings use qualified `"vim:normal"` mode and `<leader>` notation
/// (expanded by bootstrap via `LeaderKeyProvider`).
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("<leader>ff", ids::OPEN_FILES)
            .with_modes(&["vim:normal"])
            .with_description("Open file picker"),
        KeybindingRegistration::new("<leader>fb", ids::OPEN_BUFFERS)
            .with_modes(&["vim:normal"])
            .with_description("Open buffer picker"),
        KeybindingRegistration::new("<leader>sg", ids::OPEN_GREP)
            .with_modes(&["vim:normal"])
            .with_description("Open grep picker"),
        KeybindingRegistration::new("<leader>;", ids::OPEN_COMMANDS)
            .with_modes(&["vim:normal"])
            .with_description("Open command picker"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
