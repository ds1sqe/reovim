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
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, Position, RegisterContent},
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let line_len = buffer.line_len(pos.line).unwrap_or(0);

        // Can't delete on empty line or at end of line
        if line_len == 0 || pos.column >= line_len {
            return CommandResult::Success; // No-op
        }

        // Delete up to end of line
        let chars_to_delete = count.min(line_len - pos.column);
        if chars_to_delete > 0 {
            let _cursor_before = buffer.position();
            let _edit = buffer.delete(chars_to_delete);
            let _cursor_after = buffer.position();
            // TODO(#394): Return edit action via different mechanism (escape hatch until API supports this)
        }
        drop(buffer);

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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let pos = buffer.position();

        // Can't delete before column 0
        if pos.column == 0 {
            // In insert mode, join with previous line
            if pos.line > 0 {
                let prev_line_len = buffer.line_len(pos.line - 1).unwrap_or(0);
                let new_pos = Position::new(pos.line - 1, prev_line_len);
                buffer.set_position(new_pos);
                let _cursor_before = buffer.position();
                let _edit = buffer.delete(1); // Delete the newline
                let _cursor_after = buffer.position();
                // TODO(#394): Return edit action via different mechanism (escape hatch until API supports this)
            }
            drop(buffer);
            return CommandResult::Success;
        }

        let chars_to_delete = count.min(pos.column);
        let new_col = pos.column - chars_to_delete;
        let delete_pos = Position::new(pos.line, new_col);

        buffer.set_position(delete_pos);
        let _cursor_before = buffer.position();
        let _edit = buffer.delete(chars_to_delete);
        let _cursor_after = buffer.position();
        drop(buffer);

        // TODO(#394): Return edit action via different mechanism (escape hatch until API supports this)
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let start_line = buffer.position().line;
        let line_count = buffer.line_count();

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
            if let Some(line) = buffer.line(line_idx) {
                deleted_text.push_str(line);
                deleted_text.push('\n');
            }
        }

        // Store in register (use specified or unnamed)
        let content = RegisterContent::linewise(deleted_text);
        let register = args.register();
        runtime
            .kernel()
            .registers
            .write()
            .set_by_name(register, content);

        // Delete range: from start of first line to start of line after deleted range
        let start = Position::new(start_line, 0);

        // Calculate total characters to delete (including newlines)
        let mut chars_to_delete = 0;
        for i in 0..lines_to_delete {
            let line_idx = start_line + i;
            if line_idx < line_count {
                let line_len = buffer.line_len(line_idx).unwrap_or(0);
                chars_to_delete += line_len;
                // Add 1 for newline unless it's the last line
                if line_idx + 1 < line_count {
                    chars_to_delete += 1;
                }
            }
        }

        // Handle deleting last line(s) - need to also delete preceding newline
        let end_line = start_line + lines_to_delete;
        let edit = if end_line >= line_count && start_line > 0 {
            // We're deleting to end of buffer, so delete preceding newline too
            let new_start =
                Position::new(start_line - 1, buffer.line_len(start_line - 1).unwrap_or(0));
            buffer.set_position(new_start);
            buffer.delete(chars_to_delete + 1) // +1 for preceding newline
        } else {
            buffer.set_position(start);
            buffer.delete(chars_to_delete)
        };
        let cursor_before = start; // Before the delete, cursor was at start of deleted range

        // Move cursor to first non-blank of remaining line
        let new_line_count = buffer.line_count();
        let new_line = start_line.min(new_line_count.saturating_sub(1));
        let first_non_blank = buffer
            .line(new_line)
            .map_or(0, |line| line.chars().position(|c| !c.is_whitespace()).unwrap_or(0));
        buffer.set_position(Position::new(new_line, first_non_blank));
        let _cursor_after = buffer.position();
        drop(buffer);

        // TODO(#394): Return edit action via different mechanism (escape hatch until API supports this)
        let _ = (cursor_before, edit); // Suppress unused warnings
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let line_len = buffer.line_len(pos.line).unwrap_or(0);

        // Nothing to delete if at or past end of line
        if pos.column >= line_len {
            return CommandResult::Success;
        }

        // Get text to delete for register
        let deleted_text = buffer
            .line(pos.line)
            .map(|line| line[pos.column..].to_string())
            .unwrap_or_default();

        // Store in register (use specified or unnamed)
        let content = RegisterContent::characterwise(deleted_text);
        let register = args.register();
        runtime
            .kernel()
            .registers
            .write()
            .set_by_name(register, content);

        // Delete from cursor to end of line (not including newline)
        let chars_to_delete = line_len - pos.column;
        let _cursor_before = buffer.position();
        let _edit = buffer.delete(chars_to_delete);
        let _cursor_after = buffer.position();
        drop(buffer);

        // TODO(#394): Return edit action via different mechanism (escape hatch until API supports this)
        CommandResult::Success
    }
}
