//! Vim personality keybinding adapter for git-signs (#700).
//!
//! Provides git-signs keybindings with qualified `"vim:normal"` mode strings.
//! The `reovim-module-vim` dependency makes this coupling explicit and
//! compile-time enforced.

// The import of reovim_module_vim ensures the vim module exists in the
// dependency graph. This IS the declaration of personality support.
use reovim_module_vim as _;

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids;

/// Vim personality keybindings for git-signs.
///
/// All bindings use qualified `"vim:normal"` mode and `<leader>` notation
/// (expanded by bootstrap via `LeaderKeyProvider`).
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        // Hunk navigation
        KeybindingRegistration::new("]h", ids::NEXT_HUNK)
            .with_modes(&["vim:normal"])
            .with_description("Next git hunk"),
        KeybindingRegistration::new("[h", ids::PREV_HUNK)
            .with_modes(&["vim:normal"])
            .with_description("Previous git hunk"),
        // Hunk operations
        KeybindingRegistration::new("<leader>ghs", ids::STAGE_HUNK)
            .with_modes(&["vim:normal"])
            .with_description("Stage hunk"),
        KeybindingRegistration::new("<leader>ghr", ids::RESET_HUNK)
            .with_modes(&["vim:normal"])
            .with_description("Reset hunk"),
        KeybindingRegistration::new("<leader>ghS", ids::STAGE_BUFFER)
            .with_modes(&["vim:normal"])
            .with_description("Stage buffer"),
        KeybindingRegistration::new("<leader>ghR", ids::RESET_BUFFER)
            .with_modes(&["vim:normal"])
            .with_description("Reset buffer"),
        KeybindingRegistration::new("<leader>ghu", ids::UNSTAGE_FILE)
            .with_modes(&["vim:normal"])
            .with_description("Unstage file"),
        // Preview
        KeybindingRegistration::new("<leader>ghp", ids::PREVIEW_HUNK)
            .with_modes(&["vim:normal"])
            .with_description("Preview hunk"),
        KeybindingRegistration::new("<leader>ghd", ids::DIFF_THIS)
            .with_modes(&["vim:normal"])
            .with_description("Diff this file"),
    ]
}

#[cfg(test)]
#[path = "vim_tests.rs"]
mod tests;
