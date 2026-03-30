//! Vim personality keybinding adapter for bufferline (#700).
//!
//! Provides buffer pin/unpin/close bindings with qualified `"vim:normal"` mode.

use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids;

/// Vim personality keybindings for bufferline.
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("<leader>bp", ids::PIN_BUFFER)
            .with_modes(&["vim:normal"])
            .with_description("Toggle buffer pin"),
        KeybindingRegistration::new("<leader>bu", ids::UNPIN_BUFFER)
            .with_modes(&["vim:normal"])
            .with_description("Unpin buffer"),
        KeybindingRegistration::new("<leader>bc", ids::CLOSE_BUFFER)
            .with_modes(&["vim:normal"])
            .with_description("Close buffer"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
