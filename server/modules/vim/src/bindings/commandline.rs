//! Command-line mode keybindings.
//!
//! Command-line mode is entered by pressing `:` in normal mode.
//! This mode allows entering Ex commands like `:w`, `:q`, `:set`, etc.

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids as vim;

/// Command-line mode keybindings.
#[must_use]
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Cancel command-line mode (Escape)
        // ====================================================================
        KeybindingRegistration::new("<Esc>", vim::CANCEL_COMMANDLINE)
            .with_modes(&["vim:command"])
            .with_category("mode")
            .with_description("Cancel and exit command-line mode"),
        KeybindingRegistration::new("<C-c>", vim::CANCEL_COMMANDLINE)
            .with_modes(&["vim:command"])
            .with_category("mode")
            .with_description("Cancel and exit command-line mode"),
        // ====================================================================
        // Execute command-line (Enter)
        // ====================================================================
        KeybindingRegistration::new("<CR>", vim::EXIT_COMMANDLINE)
            .with_modes(&["vim:command"])
            .with_category("mode")
            .with_description("Execute command and exit command-line mode"),
    ]
}
