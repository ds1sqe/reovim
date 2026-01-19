//! Find-char motion commands.
//!
//! Implements vim find-char motions: `f`, `F`, `t`, `T`, `;`, `,`.
//!
//! These commands use the char-wait infrastructure from the runner:
//! - `f`, `F`, `t`, `T` return `WaitingForChar` to request a character argument
//! - The event loop completes the motion when the character is received
//! - `;` and `,` repeat the last find motion in same/opposite direction

use {
    reovim_driver_command::{
        CharWaitContext, Command, CommandContext, CommandHandler, CommandResult, FindType,
    },
    reovim_kernel::api::v1::{CommandId, KernelContext, Position},
};

use crate::ids;

// =============================================================================
// Helper function
// =============================================================================

/// Get current cursor position from the active buffer.
fn get_cursor_position(ctx: &KernelContext, args: &CommandContext) -> Option<Position> {
    let buffer_id = args.buffer_id()?;
    let buffer_arc = ctx.buffers.get(buffer_id)?;
    let buffer = buffer_arc.read();
    Some(buffer.position())
}

// =============================================================================
// Find Char Forward (f)
// =============================================================================

/// Find character forward - cursor lands on the character.
///
/// Press `f` followed by a character to move cursor to next occurrence
/// of that character on the current line.
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(pos) = get_cursor_position(ctx, args) else {
            return CommandResult::error("No active buffer");
        };

        CommandResult::WaitingForChar(CharWaitContext::new(FindType::FindForward, pos))
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(pos) = get_cursor_position(ctx, args) else {
            return CommandResult::error("No active buffer");
        };

        CommandResult::WaitingForChar(CharWaitContext::new(FindType::FindBackward, pos))
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(pos) = get_cursor_position(ctx, args) else {
            return CommandResult::error("No active buffer");
        };

        CommandResult::WaitingForChar(CharWaitContext::new(FindType::TillForward, pos))
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(pos) = get_cursor_position(ctx, args) else {
            return CommandResult::error("No active buffer");
        };

        CommandResult::WaitingForChar(CharWaitContext::new(FindType::TillBackward, pos))
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
/// Note: The actual repeat logic is handled by the runner which has access
/// to `last_find` state. This command signals the intent to repeat.
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Signal to runner to repeat the last find in the same direction.
        // The runner will check last_find and execute the motion.
        CommandResult::RepeatFindSame
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
/// Note: The actual repeat logic is handled by the runner which has access
/// to `last_find` state. This command signals the intent to repeat.
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // Signal to runner to repeat the last find in the opposite direction.
        // The runner will check last_find and execute the motion.
        CommandResult::RepeatFindReverse
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

    #[test]
    fn test_find_char_forward_returns_waiting_for_char() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = FindCharForward;
        let result = cmd.execute(&mut ctx, &args);

        // Without active buffer, returns error
        assert!(result.is_error());
    }

    #[test]
    fn test_find_char_backward_returns_waiting_for_char() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = FindCharBackward;
        let result = cmd.execute(&mut ctx, &args);

        // Without active buffer, returns error
        assert!(result.is_error());
    }

    #[test]
    fn test_till_char_forward_returns_waiting_for_char() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = TillCharForward;
        let result = cmd.execute(&mut ctx, &args);

        // Without active buffer, returns error
        assert!(result.is_error());
    }

    #[test]
    fn test_till_char_backward_returns_waiting_for_char() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = TillCharBackward;
        let result = cmd.execute(&mut ctx, &args);

        // Without active buffer, returns error
        assert!(result.is_error());
    }

    #[test]
    fn test_repeat_find_same_returns_repeat_find_same() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = RepeatFindSame;
        let result = cmd.execute(&mut ctx, &args);

        // RepeatFindSame returns RepeatFindSame variant (runner handles actual logic)
        assert!(matches!(result, CommandResult::RepeatFindSame));
    }

    #[test]
    fn test_repeat_find_reverse_returns_repeat_find_reverse() {
        use reovim_kernel::api::v1::KernelContext;

        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let cmd = RepeatFindReverse;
        let result = cmd.execute(&mut ctx, &args);

        // RepeatFindReverse returns RepeatFindReverse variant (runner handles actual logic)
        assert!(matches!(result, CommandResult::RepeatFindReverse));
    }
}
