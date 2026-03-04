//! Command and mode ID constants for the snippet module (#136).
//!
//! # Constants
//!
//! - `MODULE` - Module identity
//! - `NAVIGATING_MODE` - Active snippet navigation mode
//! - Command IDs for expand, jump-next, jump-prev, cancel

use reovim_kernel::api::v1::{CommandId, ModeId, ModuleId};

// =============================================================================
// Module identity
// =============================================================================

/// Snippet module ID.
pub const MODULE: ModuleId = ModuleId::new("snippet");

// =============================================================================
// Mode IDs
// =============================================================================

/// Active snippet navigation mode.
///
/// While in this mode, Tab/S-Tab navigate between tab stops.
/// Unhandled keys fall through to vim insert mode via `inherits_from()`.
pub const NAVIGATING_MODE: ModeId = ModeId::with_discriminant(MODULE, "navigating", 0);

// =============================================================================
// Command IDs
// =============================================================================

/// Expand the snippet whose prefix is the word before the cursor.
pub const EXPAND: CommandId = CommandId::new(MODULE, "expand");

/// Jump to the next tab stop in the active snippet.
pub const JUMP_NEXT: CommandId = CommandId::new(MODULE, "jump-next");

/// Jump to the previous tab stop in the active snippet.
pub const JUMP_PREV: CommandId = CommandId::new(MODULE, "jump-prev");

/// Cancel the active snippet and return to insert mode.
pub const CANCEL: CommandId = CommandId::new(MODULE, "cancel");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        assert_eq!(MODULE.as_str(), "snippet");
    }

    #[test]
    fn test_navigating_mode() {
        assert_eq!(NAVIGATING_MODE.module().as_str(), "snippet");
        assert_eq!(NAVIGATING_MODE.name(), "navigating");
        assert_eq!(NAVIGATING_MODE.discriminant(), 0);
    }

    #[test]
    fn test_command_ids() {
        assert_eq!(EXPAND.module().as_str(), "snippet");
        assert_eq!(EXPAND.name(), "expand");

        assert_eq!(JUMP_NEXT.module().as_str(), "snippet");
        assert_eq!(JUMP_NEXT.name(), "jump-next");

        assert_eq!(JUMP_PREV.module().as_str(), "snippet");
        assert_eq!(JUMP_PREV.name(), "jump-prev");

        assert_eq!(CANCEL.module().as_str(), "snippet");
        assert_eq!(CANCEL.name(), "cancel");
    }

    #[test]
    fn test_command_ids_are_distinct() {
        let commands = [&EXPAND, &JUMP_NEXT, &JUMP_PREV, &CANCEL];
        for (i, a) in commands.iter().enumerate() {
            for (j, b) in commands.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "commands {i} and {j} should differ");
                }
            }
        }
    }
}
