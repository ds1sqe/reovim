//! Vim personality keybinding adapter for snippet (#700).
//!
//! Provides snippet keybindings with qualified vim mode strings.
//! The `reovim-module-vim` dependency makes this coupling explicit and
//! compile-time enforced.

// The import of reovim_module_vim ensures the vim module exists in the
// dependency graph. This IS the declaration of personality support.
use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids;

/// Vim personality keybindings for snippet.
///
/// Insert-mode binding for expansion, normal-mode bindings for catalog
/// and reload. Leader bindings are expanded by bootstrap via
/// `LeaderKeyProvider`.
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("<C-s>", ids::EXPAND)
            .with_modes(&["vim:insert"])
            .with_description("Expand snippet at cursor"),
        KeybindingRegistration::new("<leader>sc", ids::CATALOG)
            .with_modes(&["vim:normal"])
            .with_description("List available snippets"),
        KeybindingRegistration::new("<leader>sr", ids::RELOAD)
            .with_modes(&["vim:normal"])
            .with_description("Reload snippet files"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
