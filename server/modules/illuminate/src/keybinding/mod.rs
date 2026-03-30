//! Personality-aware keybinding adapters for illuminate (#700).
//!
//! Each personality adapter is gated behind an optional feature flag.
//! The `all()` function aggregates bindings from all enabled adapters.

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
