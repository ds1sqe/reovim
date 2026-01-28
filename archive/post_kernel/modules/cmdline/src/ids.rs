//! Command ID constants for the cmdline module.
//!
//! Defines command identifiers for cmdline editing operations.
//! These are used for keybinding registration and command execution.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Cmdline module ID.
pub const MODULE: ModuleId = ModuleId::new("cmdline");

// =============================================================================
// Cmdline Editing Commands
// =============================================================================

/// Delete character before cursor (Backspace).
pub const CMDLINE_BACKSPACE: CommandId = CommandId::new(MODULE, "backspace");

/// Move cursor left.
pub const CMDLINE_CURSOR_LEFT: CommandId = CommandId::new(MODULE, "cursor-left");

/// Move cursor right.
pub const CMDLINE_CURSOR_RIGHT: CommandId = CommandId::new(MODULE, "cursor-right");

/// Delete character at cursor (Delete key).
pub const CMDLINE_DELETE_CHAR: CommandId = CommandId::new(MODULE, "delete-char");

/// Move cursor to start of line.
pub const CMDLINE_CURSOR_HOME: CommandId = CommandId::new(MODULE, "cursor-home");

/// Move cursor to end of line.
pub const CMDLINE_CURSOR_END: CommandId = CommandId::new(MODULE, "cursor-end");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        assert_eq!(MODULE.as_str(), "cmdline");
    }

    #[test]
    fn test_command_ids() {
        assert_eq!(CMDLINE_BACKSPACE.name(), "backspace");
        assert_eq!(CMDLINE_CURSOR_LEFT.name(), "cursor-left");
        assert_eq!(CMDLINE_CURSOR_RIGHT.name(), "cursor-right");
        assert_eq!(CMDLINE_DELETE_CHAR.name(), "delete-char");
        assert_eq!(CMDLINE_CURSOR_HOME.name(), "cursor-home");
        assert_eq!(CMDLINE_CURSOR_END.name(), "cursor-end");
    }

    #[test]
    fn test_command_id_module() {
        assert_eq!(CMDLINE_BACKSPACE.module().as_str(), "cmdline");
    }
}
