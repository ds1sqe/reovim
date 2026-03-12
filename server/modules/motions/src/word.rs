//! Word motion commands.
//!
//! Implements vim word motions: `w`, `b`, `e`, `W`, `B`, `E`, `ge`, `gE`.
//!
//! These commands wire to the kernel's `MotionEngine::calculate()` which
//! already implements all the motion logic.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{ChangeTracker, SessionRuntime},
    reovim_kernel::api::v1::{
        CommandId, Cursor, Direction, Motion, MotionEngine, Position, WordBoundary,
    },
};

use crate::ids;

// =============================================================================
// Helper function
// =============================================================================

/// Execute a word motion and update cursor position.
///
/// In operator-pending mode, returns an `OperatorRange` instead of moving the cursor.
/// Word motions are characterwise.
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_word_motion(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    direction: Direction,
    boundary: WordBoundary,
    end: bool,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let old_pos = Position::new(window.cursor.line, window.cursor.column);

    let count = args.count().unwrap_or(1);
    let motion = Motion::Word {
        direction,
        boundary,
        end,
    };

    // Calculate motion using with_buffer_read callback
    let motion_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let cursor = Cursor::new(old_pos);
        MotionEngine::calculate(buffer, &cursor, motion, count)
    });

    let Some(Some(new_pos)) = motion_result else {
        // Buffer not found or motion calculation failed
        return if motion_result.is_none() {
            CommandResult::error("Buffer not found")
        } else {
            CommandResult::Success // No-op if motion fails
        };
    };

    if new_pos == old_pos {
        return CommandResult::Success; // No movement
    }

    // Move cursor via per-client Window (#471)
    if let Some(window) = runtime.windows_mut().active_mut() {
        window.cursor = new_pos.into();
    }

    // Record cursor move via ChangeTracker
    runtime.record_cursor_move(buffer_id);

    // Per #388: motions just return Success. The vim resolver stores motion
    // type info in VimSessionState BEFORE dispatching, then completes the
    // operator in its post-command hook.
    CommandResult::Success
}

// =============================================================================
// Word Forward (w)
// =============================================================================

/// Move cursor to start of next word.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordForward;

impl Command for WordForward {
    fn id(&self) -> CommandId {
        ids::WORD_FORWARD
    }

    fn description(&self) -> &'static str {
        "Move to next word"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for WordForward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Forward, WordBoundary::Word, false)
    }
}

// =============================================================================
// Word Backward (b)
// =============================================================================

/// Move cursor to start of previous word.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordBackward;

impl Command for WordBackward {
    fn id(&self) -> CommandId {
        ids::WORD_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Move to previous word"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for WordBackward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Backward, WordBoundary::Word, false)
    }
}

// =============================================================================
// Word End (e)
// =============================================================================

/// Move cursor to end of current/next word.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordEnd;

impl Command for WordEnd {
    fn id(&self) -> CommandId {
        ids::WORD_END
    }

    fn description(&self) -> &'static str {
        "Move to end of word"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for WordEnd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Forward, WordBoundary::Word, true)
    }
}

// =============================================================================
// WORD Forward (W)
// =============================================================================

/// Move cursor to start of next WORD (whitespace-delimited).
#[derive(Debug, Clone, Copy, Default)]
pub struct WordForwardBig;

impl Command for WordForwardBig {
    fn id(&self) -> CommandId {
        ids::WORD_FORWARD_BIG
    }

    fn description(&self) -> &'static str {
        "Move to next WORD"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for WordForwardBig {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Forward, WordBoundary::BigWord, false)
    }
}

// =============================================================================
// WORD Backward (B)
// =============================================================================

/// Move cursor to start of previous WORD (whitespace-delimited).
#[derive(Debug, Clone, Copy, Default)]
pub struct WordBackwardBig;

impl Command for WordBackwardBig {
    fn id(&self) -> CommandId {
        ids::WORD_BACKWARD_BIG
    }

    fn description(&self) -> &'static str {
        "Move to previous WORD"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for WordBackwardBig {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Backward, WordBoundary::BigWord, false)
    }
}

// =============================================================================
// WORD End (E)
// =============================================================================

/// Move cursor to end of current/next WORD (whitespace-delimited).
#[derive(Debug, Clone, Copy, Default)]
pub struct WordEndBig;

impl Command for WordEndBig {
    fn id(&self) -> CommandId {
        ids::WORD_END_BIG
    }

    fn description(&self) -> &'static str {
        "Move to end of WORD"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for WordEndBig {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Forward, WordBoundary::BigWord, true)
    }
}

// =============================================================================
// Word End Backward (ge)
// =============================================================================

/// Move cursor to end of previous word.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordEndBackward;

impl Command for WordEndBackward {
    fn id(&self) -> CommandId {
        ids::WORD_END_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Move to end of previous word"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of words",
        )]
    }
}

impl CommandHandler for WordEndBackward {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Backward, WordBoundary::Word, true)
    }
}

// =============================================================================
// WORD End Backward (gE)
// =============================================================================

/// Move cursor to end of previous WORD (whitespace-delimited).
#[derive(Debug, Clone, Copy, Default)]
pub struct WordEndBackwardBig;

impl Command for WordEndBackwardBig {
    fn id(&self) -> CommandId {
        ids::WORD_END_BACKWARD_BIG
    }

    fn description(&self) -> &'static str {
        "Move to end of previous WORD"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of WORDs",
        )]
    }
}

impl CommandHandler for WordEndBackwardBig {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_word_motion(runtime, args, Direction::Backward, WordBoundary::BigWord, true)
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all word motion commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(WordForward),
        Box::new(WordBackward),
        Box::new(WordEnd),
        Box::new(WordForwardBig),
        Box::new(WordBackwardBig),
        Box::new(WordEndBig),
        Box::new(WordEndBackward),
        Box::new(WordEndBackwardBig),
    ]
}

// =============================================================================
// Tests
// =============================================================================
