//! Line motion commands.
//!
//! Implements vim line motions: `0`, `$`, `^`, `gg`, `G`.
//!
//! These commands wire to the kernel's `MotionEngine::calculate()` which
//! already implements all the motion logic.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{ChangeTracker, SessionRuntime},
    reovim_kernel::api::v1::{
        CommandId, Cursor, JumpEntry, LinePosition, Motion, MotionEngine, Position,
    },
};

use crate::ids;

// =============================================================================
// Helper functions
// =============================================================================

/// Execute a line position motion and update cursor position.
///
/// In operator-pending mode, returns an `OperatorRange` instead of moving the cursor.
/// Line position motions (`0`, `$`, `^`) are characterwise.
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_line_position(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    position: LinePosition,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let old_pos = Position::new(window.cursor.line, window.cursor.column);

    // Calculate motion using with_buffer_read callback
    let motion = Motion::LinePosition(position);
    let motion_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let cursor = Cursor::new(old_pos);
        MotionEngine::calculate(buffer, &cursor, motion, 1)
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

/// Execute a jump line motion and update cursor position.
///
/// In operator-pending mode, returns an `OperatorRange` instead of moving the cursor.
/// Document motions (`gg`, `G`) are linewise.
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_jump_line(
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
    target_line: Option<usize>,
) -> CommandResult {
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get cursor from per-client Window (#471)
    let Some(window) = runtime.windows().active() else {
        return CommandResult::error("No active window");
    };
    let old_pos = Position::new(window.cursor.line, window.cursor.column);

    // Calculate motion using with_buffer_read callback
    let motion = Motion::JumpLine(target_line);
    let motion_result = runtime.with_buffer_read(buffer_id, |buffer| {
        let cursor = Cursor::new(old_pos);
        MotionEngine::calculate(buffer, &cursor, motion, 1)
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

    // Push current position to jump list before major jump (#654)
    runtime
        .jumplist_mut()
        .push(JumpEntry::new(buffer_id, old_pos));

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
// Line Start (0)
// =============================================================================

/// Move cursor to start of line (column 0).
#[derive(Debug, Clone, Copy, Default)]
pub struct LineStart;

impl Command for LineStart {
    fn id(&self) -> CommandId {
        ids::LINE_START
    }

    fn description(&self) -> &'static str {
        "Move to start of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for LineStart {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_line_position(runtime, args, LinePosition::Start)
    }
}

// =============================================================================
// Line End ($)
// =============================================================================

/// Move cursor to end of line.
#[derive(Debug, Clone, Copy, Default)]
pub struct LineEnd;

impl Command for LineEnd {
    fn id(&self) -> CommandId {
        ids::LINE_END
    }

    fn description(&self) -> &'static str {
        "Move to end of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for LineEnd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_line_position(runtime, args, LinePosition::End)
    }
}

// =============================================================================
// First Non-Blank (^)
// =============================================================================

/// Move cursor to first non-blank character on line.
#[derive(Debug, Clone, Copy, Default)]
pub struct FirstNonBlank;

impl Command for FirstNonBlank {
    fn id(&self) -> CommandId {
        ids::FIRST_NON_BLANK
    }

    fn description(&self) -> &'static str {
        "Move to first non-blank character"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for FirstNonBlank {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_line_position(runtime, args, LinePosition::FirstNonBlank)
    }
}

// =============================================================================
// Document Start (gg)
// =============================================================================

/// Move cursor to start of document (first line).
#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentStart;

impl Command for DocumentStart {
    fn id(&self) -> CommandId {
        ids::DOCUMENT_START
    }

    fn description(&self) -> &'static str {
        "Move to start of document"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Line number to jump to",
        )]
    }
}

impl CommandHandler for DocumentStart {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // gg without count goes to line 0
        // With count, go to that line (1-indexed in vim, convert to 0-indexed)
        let target_line = args.count().map(|c| c.saturating_sub(1));
        execute_jump_line(runtime, args, target_line.or(Some(0)))
    }
}

// =============================================================================
// Document End (G)
// =============================================================================

/// Move cursor to end of document (last line) or specific line with count.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentEnd;

impl Command for DocumentEnd {
    fn id(&self) -> CommandId {
        ids::DOCUMENT_END
    }

    fn description(&self) -> &'static str {
        "Move to end of document or line N"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Line number to jump to",
        )]
    }
}

impl CommandHandler for DocumentEnd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // G without count goes to last line (None)
        // With count, go to that line (1-indexed in vim, convert to 0-indexed)
        let target_line = args.count().map(|c| c.saturating_sub(1));
        execute_jump_line(runtime, args, target_line)
    }
}

// =============================================================================
// Whole Line Motion (for operator doubling: dd, yy, cc)
// =============================================================================

/// Whole line motion for operator doubling (dd, yy, cc).
///
/// In operator-pending mode, this returns the current line as a linewise range.
/// This enables vim's pattern where pressing the operator key twice operates
/// on the current line (e.g., 'd' enters operator-pending, then 'd' again
/// provides the "whole line" motion).
///
/// In normal mode, this is a no-op since there's no operator to apply.
#[derive(Debug, Clone, Copy, Default)]
pub struct WholeLine;

impl Command for WholeLine {
    fn id(&self) -> CommandId {
        ids::WHOLE_LINE
    }

    fn description(&self) -> &'static str {
        "Whole line motion (for operator doubling)"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines",
        )]
    }
}

impl CommandHandler for WholeLine {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // WholeLine is used for operator doubling (dd, yy, cc).
        // Per #388: motions just return Success. The vim resolver already
        // knows dd/yy/cc are linewise and handles this in is_line_operator().
        CommandResult::Success
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all line motion commands as boxed trait objects.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(LineStart),
        Box::new(LineEnd),
        Box::new(FirstNonBlank),
        Box::new(DocumentStart),
        Box::new(DocumentEnd),
        Box::new(WholeLine),
    ]
}

// =============================================================================
// Tests
// =============================================================================
