//! Delete commands.
//!
//! Provides delete commands:
//! - `DeleteChar` (x)
//! - `DeleteCharBefore` (X)
//! - `DeleteLine` (dd)
//! - `DeleteToEndOfLine` (D)
//!
//! Note: Change commands (cc, C) are in `reovim_module_vim::commands::change`
//! because they require `VimMode` constants for mode transitions.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{BufferApi, SessionRuntime},
    reovim_kernel::api::v1::CommandId,
    reovim_types_text::{Position, RegisterContent},
};

use crate::ids;

/// Delete character under cursor (x).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteChar;

impl Command for DeleteChar {
    fn id(&self) -> CommandId {
        ids::DELETE_CHAR
    }

    fn description(&self) -> &'static str {
        "Delete character under cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to delete",
        )]
    }
}

impl CommandHandler for DeleteChar {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);

        // Can't delete on empty line or at end of line
        if line_len == 0 || pos.column >= line_len {
            return CommandResult::Success; // No-op
        }

        // Delete up to end of line
        let chars_to_delete = count.min(line_len - pos.column);
        if chars_to_delete > 0 {
            let end = Position::new(pos.line, pos.column + chars_to_delete);
            runtime.delete_range(buffer_id, pos, end);
        }

        CommandResult::Success
    }
}

/// Delete character before cursor (X).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteCharBefore;

impl Command for DeleteCharBefore {
    fn id(&self) -> CommandId {
        ids::DELETE_CHAR_BEFORE
    }

    fn description(&self) -> &'static str {
        "Delete character before cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to delete",
        )]
    }
}

impl CommandHandler for DeleteCharBefore {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Can't delete before column 0
        if pos.column == 0 {
            // In insert mode, join with previous line
            if pos.line > 0 {
                let prev_line_len = runtime
                    .buffer_line_len(buffer_id, pos.line - 1)
                    .unwrap_or(0);
                let new_pos = Position::new(pos.line - 1, prev_line_len);
                // Update cursor via per-client Window (#471)
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.cursor = new_pos.into();
                }
                // Delete the newline character (from end of prev line to start of current line)
                let end = Position::new(pos.line, 0);
                runtime.delete_range(buffer_id, new_pos, end);
            }
            return CommandResult::Success;
        }

        let chars_to_delete = count.min(pos.column);
        let new_col = pos.column - chars_to_delete;
        let delete_pos = Position::new(pos.line, new_col);

        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = delete_pos.into();
        }
        runtime.delete_range(buffer_id, delete_pos, pos);

        CommandResult::Success
    }
}

/// Delete current line (dd).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteLine;

impl Command for DeleteLine {
    fn id(&self) -> CommandId {
        ids::DELETE_LINE
    }

    fn description(&self) -> &'static str {
        "Delete current line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of lines to delete"),
            ArgSpec::optional("register", ArgKind::Register, "Target register"),
        ]
    }
}

impl CommandHandler for DeleteLine {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);

        // Get cursor from per-client Window (#471)
        let start_line = runtime.windows().active().map_or(0, |w| w.cursor.line);
        let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);

        if line_count == 0 {
            return CommandResult::Success;
        }

        // Calculate lines to delete
        let lines_to_delete = count.min(line_count.saturating_sub(start_line));
        if lines_to_delete == 0 {
            return CommandResult::Success;
        }

        // Collect deleted text for register
        let mut deleted_text = String::new();
        for i in 0..lines_to_delete {
            let line_idx = start_line + i;
            if let Some(line) = runtime.buffer_line(buffer_id, line_idx) {
                deleted_text.push_str(&line);
                deleted_text.push('\n');
            }
        }

        // Store in register with clipboard sync (#515)
        let content = RegisterContent::linewise(deleted_text);
        let register = args.register();
        runtime.store_register_with_sync(register, content);

        // Calculate delete range
        let end_line = start_line + lines_to_delete;
        let last_deleted_line = end_line - 1;
        let last_deleted_line_len = runtime
            .buffer_line_len(buffer_id, last_deleted_line)
            .unwrap_or(0);

        let (delete_start, delete_end) = if end_line >= line_count && start_line > 0 {
            // Deleting to end of buffer AND not the first line - include preceding newline
            let prev_line_len = runtime
                .buffer_line_len(buffer_id, start_line - 1)
                .unwrap_or(0);
            (
                Position::new(start_line - 1, prev_line_len),
                Position::new(last_deleted_line, last_deleted_line_len),
            )
        } else if end_line < line_count {
            // Not deleting to end of buffer - include newline after last deleted line
            (Position::new(start_line, 0), Position::new(end_line, 0))
        } else {
            // Deleting to end of buffer from the first line - delete just the content
            (
                Position::new(start_line, 0),
                Position::new(last_deleted_line, last_deleted_line_len),
            )
        };

        // Perform the delete
        runtime.delete_range(buffer_id, delete_start, delete_end);

        // Move cursor to first non-blank of remaining line
        let new_line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);
        let new_line = start_line.min(new_line_count.saturating_sub(1));
        let first_non_blank = runtime
            .buffer_line(buffer_id, new_line)
            .map_or(0, |line| line.chars().position(|c| !c.is_whitespace()).unwrap_or(0));
        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = Position::new(new_line, first_non_blank).into();
        }

        CommandResult::Success
    }
}

/// Delete to end of line (D).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteToEndOfLine;

impl Command for DeleteToEndOfLine {
    fn id(&self) -> CommandId {
        ids::DELETE_TO_EOL
    }

    fn description(&self) -> &'static str {
        "Delete to end of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "register",
            ArgKind::Register,
            "Target register",
        )]
    }
}

impl CommandHandler for DeleteToEndOfLine {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);

        // Nothing to delete if at or past end of line
        if pos.column >= line_len {
            return CommandResult::Success;
        }

        // Get text to delete for register
        let deleted_text = runtime
            .buffer_line(buffer_id, pos.line)
            .map(|line| line[pos.column..].to_string())
            .unwrap_or_default();

        // Store in register with clipboard sync (#515)
        let content = RegisterContent::characterwise(deleted_text);
        let register = args.register();
        runtime.store_register_with_sync(register, content);

        // Delete from cursor to end of line (not including newline)
        let end = Position::new(pos.line, line_len);
        runtime.delete_range(buffer_id, pos, end);

        CommandResult::Success
    }
}
