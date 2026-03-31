//! Vim personality keybinding adapter for lsp-navigation (#700).
//!
//! Provides LSP navigation keybindings with qualified vim mode strings.
//! The `reovim-module-vim` dependency makes this coupling explicit and
//! compile-time enforced.

// The import of reovim_module_vim ensures the vim module exists in the
// dependency graph. This IS the declaration of personality support.
use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids;

/// Vim personality keybindings for lsp-navigation.
///
/// Bindings use `"vim:normal"` for navigation and `"vim:insert"` for
/// signature help.
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        KeybindingRegistration::new("gd", ids::GOTO_DEFINITION)
            .with_modes(&["vim:normal"])
            .with_description("Go to definition (LSP)"),
        KeybindingRegistration::new("gr", ids::REFERENCES)
            .with_modes(&["vim:normal"])
            .with_description("Find references (LSP)"),
        KeybindingRegistration::new("K", ids::HOVER)
            .with_modes(&["vim:normal"])
            .with_description("Show hover information (LSP)"),
        KeybindingRegistration::new("<C-k>", ids::SIGNATURE_HELP)
            .with_modes(&["vim:insert"])
            .with_description("Show signature help (LSP)"),
        // Diagnostic navigation (#699)
        KeybindingRegistration::new("]d", ids::NEXT_DIAGNOSTIC)
            .with_modes(&["vim:normal"])
            .with_description("Next diagnostic"),
        KeybindingRegistration::new("[d", ids::PREV_DIAGNOSTIC)
            .with_modes(&["vim:normal"])
            .with_description("Previous diagnostic"),
        KeybindingRegistration::new("]e", ids::NEXT_ERROR)
            .with_modes(&["vim:normal"])
            .with_description("Next error"),
        KeybindingRegistration::new("[e", ids::PREV_ERROR)
            .with_modes(&["vim:normal"])
            .with_description("Previous error"),
        KeybindingRegistration::new("]w", ids::NEXT_WARNING)
            .with_modes(&["vim:normal"])
            .with_description("Next warning"),
        KeybindingRegistration::new("[w", ids::PREV_WARNING)
            .with_modes(&["vim:normal"])
            .with_description("Previous warning"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
