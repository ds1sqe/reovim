//! Vim personality keybinding adapter for diagnostics-panel (#700).
//!
//! Provides the normal-mode toggle binding with qualified `"vim:normal"` mode.

use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids;

/// Vim personality keybindings for diagnostics-panel.
///
/// Only the normal-mode toggle binding lives here. Panel-mode bindings
/// (j/k/q/etc.) use the module's own qualified mode and stay in `lib.rs`.
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("<leader>xx", ids::TROUBLE_TOGGLE)
            .with_modes(&["vim:normal"])
            .with_description("Toggle diagnostics panel"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
