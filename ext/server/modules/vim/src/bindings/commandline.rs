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
        // ====================================================================
        // Command-line editing (#451)
        // ====================================================================
        KeybindingRegistration::new("<Left>", vim::CMDLINE_CURSOR_LEFT)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Move cursor left"),
        KeybindingRegistration::new("<Right>", vim::CMDLINE_CURSOR_RIGHT)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Move cursor right"),
        KeybindingRegistration::new("<Home>", vim::CMDLINE_CURSOR_HOME)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Move cursor to start"),
        KeybindingRegistration::new("<C-a>", vim::CMDLINE_CURSOR_HOME)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Move cursor to start"),
        KeybindingRegistration::new("<End>", vim::CMDLINE_CURSOR_END)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Move cursor to end"),
        KeybindingRegistration::new("<C-e>", vim::CMDLINE_CURSOR_END)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Move cursor to end"),
        KeybindingRegistration::new("<BS>", vim::CMDLINE_BACKSPACE)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Delete character before cursor"),
        KeybindingRegistration::new("<Del>", vim::CMDLINE_DELETE_CHAR)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Delete character at cursor"),
        KeybindingRegistration::new("<C-w>", vim::CMDLINE_DELETE_WORD)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Delete word before cursor"),
        KeybindingRegistration::new("<C-u>", vim::CMDLINE_DELETE_TO_START)
            .with_modes(&["vim:command"])
            .with_category("editing")
            .with_description("Delete to start of line"),
        // ====================================================================
        // Command-line history (#451)
        // ====================================================================
        KeybindingRegistration::new("<Up>", vim::CMDLINE_HISTORY_UP)
            .with_modes(&["vim:command"])
            .with_category("history")
            .with_description("Navigate to older history entry"),
        KeybindingRegistration::new("<C-p>", vim::CMDLINE_HISTORY_UP)
            .with_modes(&["vim:command"])
            .with_category("history")
            .with_description("Navigate to older history entry"),
        KeybindingRegistration::new("<Down>", vim::CMDLINE_HISTORY_DOWN)
            .with_modes(&["vim:command"])
            .with_category("history")
            .with_description("Navigate to newer history entry"),
        KeybindingRegistration::new("<C-n>", vim::CMDLINE_HISTORY_DOWN)
            .with_modes(&["vim:command"])
            .with_category("history")
            .with_description("Navigate to newer history entry"),
        // ====================================================================
        // Command-line completion (#451)
        // ====================================================================
        KeybindingRegistration::new("<Tab>", vim::CMDLINE_COMPLETE_NEXT)
            .with_modes(&["vim:command"])
            .with_category("completion")
            .with_description("Cycle to next completion"),
        KeybindingRegistration::new("<S-Tab>", vim::CMDLINE_COMPLETE_PREV)
            .with_modes(&["vim:command"])
            .with_category("completion")
            .with_description("Cycle to previous completion"),
    ]
}
