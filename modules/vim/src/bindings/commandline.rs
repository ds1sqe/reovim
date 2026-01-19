//! Command-line mode keybindings.
//!
//! Command-line mode is entered by pressing `:` in normal mode.
//! This mode allows entering Ex commands like `:w`, `:q`, `:set`, etc.

use reovim_kernel::api::v1::KeybindingRegistration;

use crate::ids as vim;

/// Command-line mode keybindings.
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Exit command-line mode
        // ====================================================================
        KeybindingRegistration::new("<Esc>", vim::EXIT_COMMANDLINE)
            .with_modes(&["vim:command"])
            .with_category("mode")
            .with_description("Cancel and exit command-line mode"),
        KeybindingRegistration::new("<C-c>", vim::EXIT_COMMANDLINE)
            .with_modes(&["vim:command"])
            .with_category("mode")
            .with_description("Cancel and exit command-line mode"),
        // Note: Enter key for executing commands will be added when command
        // execution is implemented. For now, Enter just exits.
        KeybindingRegistration::new("<CR>", vim::EXIT_COMMANDLINE)
            .with_modes(&["vim:command"])
            .with_category("mode")
            .with_description("Execute command and exit command-line mode"),
    ]
}
