//! Find-char motion execution command.
//!
//! This command executes find-char motions (f, F, t, T) with the target
//! character passed via command context. It's part of Epic #385's resolver-based
//! approach where the vim resolver handles pending_char state and calls this
//! command to execute the actual motion.
//!
//! # Architecture
//!
//! ```text
//! Key press → VimNormalResolver → Sets vim.pending_char → Returns Pending
//! Next char → VimNormalResolver → Returns Execute(EXECUTE_FIND_CHAR, ctx_with_char)
//! Runner    → Calls this command with char in context
//! Command   → Calculates motion, moves cursor, returns Success
//! ```
//!
//! # Note
//!
//! This command does NOT update `last_find` because commands don't have
//! access to `VimSessionState` via extensions yet. That will be added in
//! a future phase when commands have API support (#394).

use {
    reovim_driver_command::{ArgValue, Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, Motion, MotionEngine},
};

use crate::ids::EXECUTE_FIND_CHAR;

/// Execute a find-char motion (f, F, t, T) with character from context.
///
/// Reads the target character and direction from command context metadata
/// and moves the cursor to the appropriate position.
///
/// # Arguments (via CommandContext)
///
/// - `find_char`: The target character (required)
/// - `find_direction`: "forward" or "backward" (default: "forward")
/// - `find_inclusive`: true for f/F (on char), false for t/T (before char) (default: true)
/// - `count`: Number of occurrences to skip (default: 1)
pub struct ExecuteFindChar;

impl Command for ExecuteFindChar {
    fn id(&self) -> CommandId {
        EXECUTE_FIND_CHAR
    }

    fn description(&self) -> &'static str {
        "Execute a find-char motion (f, F, t, T) with the target character from context"
    }
}

impl CommandHandler for ExecuteFindChar {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get the target character (required)
        let Some(target_char) = args.char("find_char") else {
            return CommandResult::error("find_char argument required");
        };

        // Get direction (default: forward)
        let forward = args.string("find_direction") != Some("backward");

        // Get inclusive flag (default: true for find, false for till)
        let inclusive = match args.get("find_inclusive") {
            Some(ArgValue::Bang(b)) => *b,
            _ => true,
        };

        // Get count (default: 1)
        let count = args.count().unwrap_or(1);

        // Get active buffer
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Build the find-char motion
        let motion = Motion::FindChar {
            char: target_char,
            direction: if forward {
                reovim_kernel::api::v1::Direction::Forward
            } else {
                reovim_kernel::api::v1::Direction::Backward
            },
            till: !inclusive,
        };

        // Calculate motion target
        let buffer = buffer_arc.read();
        let target = MotionEngine::calculate(&buffer, buffer.cursor(), motion, count);
        drop(buffer);

        // Apply motion if target found
        if let Some(pos) = target {
            buffer_arc.write().set_position(pos);
            CommandResult::Success
        } else {
            // Character not found - this is not an error, just don't move
            // (Vim behavior: cursor stays in place, no beep)
            CommandResult::Success
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_metadata() {
        let cmd = ExecuteFindChar;
        assert_eq!(cmd.id(), EXECUTE_FIND_CHAR);
        assert!(cmd.description().contains("find-char"));
    }
}
