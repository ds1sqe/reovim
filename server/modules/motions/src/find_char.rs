//! Find-char motion commands.
//!
//! Implements vim find-char motions: `f`, `F`, `t`, `T`, `;`, `,`.
//!
//! # Architecture (Epic #385)
//!
//! These commands are **intercepted by the vim resolver** before execution.
//! The resolver handles `pending_char` state via `VimSessionState`. These command
//! definitions exist only for:
//! 1. Keybinding registration (command IDs)
//! 2. Command metadata (description, args)
//!
//! The actual find-char logic lives in:
//! - `VimNormalResolver::classify_find_char_command()` - intercepts these commands
//! - `vim::commands::ExecuteFindChar` - executes the motion with char from context

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

// =============================================================================
// Find Char Forward (f)
// =============================================================================

/// Find character forward - cursor lands on the character.
///
/// Press `f` followed by a character to move cursor to next occurrence
/// of that character on the current line.
///
/// Note: This command is intercepted by the vim resolver. The `execute()`
/// method should never be called in normal operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindCharForward;

impl Command for FindCharForward {
    fn id(&self) -> CommandId {
        ids::FIND_CHAR_FORWARD
    }

    fn description(&self) -> &'static str {
        "Find character forward (f)"
    }
}

impl CommandHandler for FindCharForward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Find Char Backward (F)
// =============================================================================

/// Find character backward - cursor lands on the character.
///
/// Press `F` followed by a character to move cursor to previous occurrence
/// of that character on the current line.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindCharBackward;

impl Command for FindCharBackward {
    fn id(&self) -> CommandId {
        ids::FIND_CHAR_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Find character backward (F)"
    }
}

impl CommandHandler for FindCharBackward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Till Char Forward (t)
// =============================================================================

/// Till character forward - cursor stops before the character.
///
/// Press `t` followed by a character to move cursor to position just
/// before the next occurrence of that character on the current line.
#[derive(Debug, Clone, Copy, Default)]
pub struct TillCharForward;

impl Command for TillCharForward {
    fn id(&self) -> CommandId {
        ids::TILL_CHAR_FORWARD
    }

    fn description(&self) -> &'static str {
        "Till character forward (t)"
    }
}

impl CommandHandler for TillCharForward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Till Char Backward (T)
// =============================================================================

/// Till character backward - cursor stops after the character.
///
/// Press `T` followed by a character to move cursor to position just
/// after the previous occurrence of that character on the current line.
#[derive(Debug, Clone, Copy, Default)]
pub struct TillCharBackward;

impl Command for TillCharBackward {
    fn id(&self) -> CommandId {
        ids::TILL_CHAR_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Till character backward (T)"
    }
}

impl CommandHandler for TillCharBackward {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Intercepted by vim resolver - should not reach here
        CommandResult::Success
    }
}

// =============================================================================
// Repeat Find Same (;)
// =============================================================================

/// Repeat last find-char in the same direction.
///
/// After using `f`, `F`, `t`, or `T`, press `;` to repeat that motion
/// in the same direction.
///
/// Note: Repeat logic is handled by vim resolver via `VimSessionState.last_find`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatFindSame;

impl Command for RepeatFindSame {
    fn id(&self) -> CommandId {
        ids::REPEAT_FIND_SAME
    }

    fn description(&self) -> &'static str {
        "Repeat last find in same direction (;)"
    }
}

impl CommandHandler for RepeatFindSame {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via VimSessionState.last_find (escape hatch until API supports this)
        CommandResult::Success
    }
}

// =============================================================================
// Repeat Find Reverse (,)
// =============================================================================

/// Repeat last find-char in the opposite direction.
///
/// After using `f`, `F`, `t`, or `T`, press `,` to repeat that motion
/// in the opposite direction.
///
/// Note: Repeat logic is handled by vim resolver via `VimSessionState.last_find`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatFindReverse;

impl Command for RepeatFindReverse {
    fn id(&self) -> CommandId {
        ids::REPEAT_FIND_REVERSE
    }

    fn description(&self) -> &'static str {
        "Repeat last find in opposite direction (,)"
    }
}

impl CommandHandler for RepeatFindReverse {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via VimSessionState.last_find (escape hatch until API supports this)
        CommandResult::Success
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Get all find-char motion commands.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(FindCharForward),
        Box::new(FindCharBackward),
        Box::new(TillCharForward),
        Box::new(TillCharBackward),
        Box::new(RepeatFindSame),
        Box::new(RepeatFindReverse),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_char_forward_id() {
        let cmd = FindCharForward;
        assert_eq!(cmd.id().name(), "find-char-forward");
        assert_eq!(cmd.description(), "Find character forward (f)");
    }

    #[test]
    fn test_find_char_backward_id() {
        let cmd = FindCharBackward;
        assert_eq!(cmd.id().name(), "find-char-backward");
        assert_eq!(cmd.description(), "Find character backward (F)");
    }

    #[test]
    fn test_till_char_forward_id() {
        let cmd = TillCharForward;
        assert_eq!(cmd.id().name(), "till-char-forward");
        assert_eq!(cmd.description(), "Till character forward (t)");
    }

    #[test]
    fn test_till_char_backward_id() {
        let cmd = TillCharBackward;
        assert_eq!(cmd.id().name(), "till-char-backward");
        assert_eq!(cmd.description(), "Till character backward (T)");
    }

    #[test]
    fn test_repeat_find_same_id() {
        let cmd = RepeatFindSame;
        assert_eq!(cmd.id().name(), "repeat-find-same");
        assert_eq!(cmd.description(), "Repeat last find in same direction (;)");
    }

    #[test]
    fn test_repeat_find_reverse_id() {
        let cmd = RepeatFindReverse;
        assert_eq!(cmd.id().name(), "repeat-find-reverse");
        assert_eq!(cmd.description(), "Repeat last find in opposite direction (,)");
    }

    #[test]
    fn test_all_commands_count() {
        let cmds = all_commands();
        assert_eq!(cmds.len(), 6); // f, F, t, T, ;, ,
    }
}
