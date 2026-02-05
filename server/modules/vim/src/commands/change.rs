//! Change commands.
//!
//! Provides change commands that delete text and enter insert mode:
//! - `ChangeLine` (cc)
//! - `ChangeToEndOfLine` (C)
//!
//! # Epic #372 - Mode Ownership
//!
//! These commands use `VimMode::INSERT_ID` to transition to insert mode after
//! deleting, which is why they belong in the vim module.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
        BufferApi, RegisterApi, SessionRuntime, TransitionContext, api::ModeApi,
    },
    reovim_kernel::api::v1::{CommandId, Position, RegisterContent},
};

/// Helper to get cursor position from the active window.
fn get_cursor_position(runtime: &SessionRuntime<'_>) -> Option<Position> {
    let window = runtime.windows().active()?;
    Some(Position::new(window.cursor.line, window.cursor.column))
}

/// Helper to set cursor position on the active window.
fn set_cursor_position(runtime: &mut SessionRuntime<'_>, pos: Position) {
    if let Some(window) = runtime.windows_mut().active_mut() {
        window.cursor = pos.into();
    }
}

use crate::{ids, modes::VimMode};

/// Change current line (cc).
///
/// Clears the content of the current line(s) and enters insert mode.
/// Unlike `dd`, this keeps the line(s) but empties their content.
/// The deleted text is stored in the register as linewise.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeLine;

impl Command for ChangeLine {
    fn id(&self) -> CommandId {
        ids::CHANGE_LINE
    }

    fn description(&self) -> &'static str {
        "Change current line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of lines to change"),
            ArgSpec::optional("register", ArgKind::Register, "Target register"),
        ]
    }
}

impl CommandHandler for ChangeLine {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);
        let start_line = get_cursor_position(runtime).map_or(0, |p| p.line);
        let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);

        if line_count == 0 {
            // Empty buffer - just enter insert mode
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
            return CommandResult::Success;
        }

        // Calculate lines to change
        let lines_to_change = count.min(line_count.saturating_sub(start_line));
        if lines_to_change == 0 {
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
            return CommandResult::Success;
        }

        // Collect text to delete for register (include newlines between lines)
        let mut deleted_text = String::new();
        for i in 0..lines_to_change {
            let line_idx = start_line + i;
            if let Some(line) = runtime.buffer_line(buffer_id, line_idx) {
                deleted_text.push_str(&line);
            }
            if i < lines_to_change - 1 {
                deleted_text.push('\n');
            }
        }
        deleted_text.push('\n'); // Linewise content ends with newline

        // Store in register via RegisterApi
        let content = RegisterContent::linewise(deleted_text);
        let register = args.register();
        runtime.set_register(register, content);

        // For cc: if changing multiple lines, delete all but first, then clear first
        // Single line: just clear the content

        if lines_to_change == 1 {
            // Clear the single line content
            let line_len = runtime.buffer_line_len(buffer_id, start_line).unwrap_or(0);
            if line_len > 0 {
                let delete_start = Position::new(start_line, 0);
                let delete_end = Position::new(start_line, line_len);
                runtime.delete_range(buffer_id, delete_start, delete_end);
                set_cursor_position(runtime, Position::new(start_line, 0));
            }
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
            return CommandResult::Success;
        }

        // Multiple lines: delete all content from first line to end of last changed line
        // Then keep one empty line at start_line
        let last_changed_line = start_line + lines_to_change - 1;
        let last_line_len = runtime
            .buffer_line_len(buffer_id, last_changed_line)
            .unwrap_or(0);
        let end_line = start_line + lines_to_change;

        let (delete_start, delete_end) = if end_line >= line_count {
            // Changing to end of buffer - delete from start of first line to end of last line
            (Position::new(start_line, 0), Position::new(last_changed_line, last_line_len))
        } else {
            // Normal case: delete from start of first line to start of line after changed range
            (Position::new(start_line, 0), Position::new(end_line, 0))
        };

        runtime.delete_range(buffer_id, delete_start, delete_end);
        set_cursor_position(runtime, Position::new(start_line, 0));
        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Change to end of line (C).
///
/// Deletes from cursor to end of line and enters insert mode.
/// The deleted text is stored in the register as characterwise.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeToEndOfLine;

impl Command for ChangeToEndOfLine {
    fn id(&self) -> CommandId {
        ids::CHANGE_TO_EOL
    }

    fn description(&self) -> &'static str {
        "Change to end of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "register",
            ArgKind::Register,
            "Target register",
        )]
    }
}

impl CommandHandler for ChangeToEndOfLine {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let pos = get_cursor_position(runtime).unwrap_or_else(|| Position::new(0, 0));
        let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);

        // Nothing to delete if at or past end of line - just enter insert mode
        if pos.column >= line_len {
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
            return CommandResult::Success;
        }

        // Get text to delete for register
        let deleted_text = runtime
            .buffer_line(buffer_id, pos.line)
            .map(|line| line[pos.column..].to_string())
            .unwrap_or_default();

        // Store in register via RegisterApi
        let content = RegisterContent::characterwise(deleted_text);
        let register = args.register();
        runtime.set_register(register, content);

        // Delete from cursor to end of line (not including newline)
        let delete_end = Position::new(pos.line, line_len);
        runtime.delete_range(buffer_id, pos, delete_end);

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}
