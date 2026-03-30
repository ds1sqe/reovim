//! Personality-aware keybinding adapters for diagnostics-panel (#700).

#[cfg(feature = "vim-keybindings")]
mod vim;

use reovim_kernel::api::v1::KeybindingRegistration;

/// Collect keybindings from all enabled personality adapters.
pub fn all() -> Vec<KeybindingRegistration> {
    #[allow(unused_mut)] // no extensions when all personality features are off
    let mut bindings = Vec::new();
    #[cfg(feature = "vim-keybindings")]
    bindings.extend(vim::keybindings());
    bindings
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
