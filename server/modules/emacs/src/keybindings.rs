//! Emacs-style keybindings (skeleton proof-of-concept).
//!
//! Maps emacs cursor movement keys to the same editor commands used by vim,
//! demonstrating that personality = keybinding vocabulary, not separate commands.

use reovim_kernel::api::v1::KeybindingRegistration;

// Reuse editor command IDs — the commands are mechanism (same for all personalities),
// only the key bindings (policy) change between vim and emacs.
use reovim_module_editor::ids as editor;

/// Emacs mode name constant.
const EMACS_MODE: &[&str] = &["emacs:default"];

/// Returns all emacs-style keybindings.
///
/// This is a minimal skeleton demonstrating the personality pattern.
/// A full emacs implementation would add: M-x, C-x C-f, kill ring,
/// region/mark system, minibuffer, etc.
#[must_use]
pub fn all() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Cursor Movement (C-f/b/p/n = forward/back/previous/next)
        // ====================================================================
        KeybindingRegistration::new("<C-f>", editor::CURSOR_RIGHT)
            .with_modes(EMACS_MODE)
            .with_category("motion")
            .with_description("Forward char"),
        KeybindingRegistration::new("<C-b>", editor::CURSOR_LEFT)
            .with_modes(EMACS_MODE)
            .with_category("motion")
            .with_description("Backward char"),
        KeybindingRegistration::new("<C-p>", editor::CURSOR_UP)
            .with_modes(EMACS_MODE)
            .with_category("motion")
            .with_description("Previous line"),
        KeybindingRegistration::new("<C-n>", editor::CURSOR_DOWN)
            .with_modes(EMACS_MODE)
            .with_category("motion")
            .with_description("Next line"),
        // ====================================================================
        // Line editing
        // ====================================================================
        KeybindingRegistration::new("<C-a>", editor::LINE_START)
            .with_modes(EMACS_MODE)
            .with_category("motion")
            .with_description("Beginning of line"),
        KeybindingRegistration::new("<C-e>", editor::LINE_END)
            .with_modes(EMACS_MODE)
            .with_category("motion")
            .with_description("End of line"),
    ]
}

#[cfg(test)]
#[path = "keybindings_tests.rs"]
mod tests;
