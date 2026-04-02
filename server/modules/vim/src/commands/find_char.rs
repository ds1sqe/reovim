//! Find-char motion execution command.
//!
//! This command executes find-char motions (f, F, t, T) with the target
//! character passed via command context. It's part of Epic #385's resolver-based
//! approach where the vim resolver handles `pending_char` state and calls this
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
    reovim_driver_session::{SessionRuntime, api::ChangeTracker},
    reovim_kernel::api::v1::CommandId,
    reovim_types_text::{Cursor, Direction, Motion, MotionEngine, Position},
};

use crate::ids::EXECUTE_FIND_CHAR;

/// Execute a find-char motion (f, F, t, T) with character from context.
///
/// Reads the target character and direction from command context metadata
/// and moves the cursor to the appropriate position.
///
/// # Arguments (via `CommandContext`)
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get the target character (required)
        let Some(target_char) = args.char("find_char") else {
            return CommandResult::error("find_char argument required");
        };

        // Get direction (default: forward)
        let forward = args.string("find_direction") != Some("backward");

        // Get inclusive flag (default: true for find, false for till).
        let inclusive = match args.get("find_inclusive") {
            Some(ArgValue::Bool(b)) => *b,
            _ => true,
        };

        // Get count (default: 1)
        let count = args.count().unwrap_or(1);

        // Get active buffer
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor position from active window
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let cursor_line = window.cursor.line;
        let cursor_col = window.cursor.column;

        let cursor = Cursor::new(Position::new(cursor_line, cursor_col));
        let motion = Motion::FindChar {
            char: target_char,
            direction: if forward {
                Direction::Forward
            } else {
                Direction::Backward
            },
            till: !inclusive,
        };

        let target = runtime.with_text_geometry(buffer_id, |buffer| {
            MotionEngine::calculate(buffer, &cursor, motion, count)
        });

        let Some(target) = target else {
            return CommandResult::error("Buffer not found");
        };

        // Apply motion if target found
        if let Some(pos) = target {
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = pos.into();
            }
            // #474: Record cursor move so notification pipeline and
            // centralized selection extension are triggered.
            runtime.record_cursor_move(buffer_id);
        }
        // Character not found - this is not an error, just don't move
        // (Vim behavior: cursor stays in place, no beep)
        CommandResult::Success
    }
}
