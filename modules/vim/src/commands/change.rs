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
    reovim_kernel::api::v1::{
        CommandId, KernelContext, Position, RegisterContent, events::ModeChanged,
    },
};

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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let start_line = buffer.position().line;
        let line_count = buffer.line_count();

        if line_count == 0 {
            // Empty buffer - just enter insert mode
            drop(buffer);
            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));
            return CommandResult::Success;
        }

        // Calculate lines to change
        let lines_to_change = count.min(line_count.saturating_sub(start_line));
        if lines_to_change == 0 {
            drop(buffer);
            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));
            return CommandResult::Success;
        }

        // Collect text to delete for register (include newlines between lines)
        let mut deleted_text = String::new();
        for i in 0..lines_to_change {
            let line_idx = start_line + i;
            if let Some(line) = buffer.line(line_idx) {
                deleted_text.push_str(line);
            }
            if i < lines_to_change - 1 {
                deleted_text.push('\n');
            }
        }
        deleted_text.push('\n'); // Linewise content ends with newline

        // Store in register (use specified or unnamed)
        let content = RegisterContent::linewise(deleted_text);
        let register = args.register();
        ctx.registers.write().set_by_name(register, content);

        // For cc: if changing multiple lines, delete all but first, then clear first
        // Single line: just clear the content
        let cursor_before = buffer.position();

        if lines_to_change == 1 {
            // Clear the single line content
            let line_len = buffer.line_len(start_line).unwrap_or(0);
            if line_len > 0 {
                buffer.set_position(Position::new(start_line, 0));
                let _edit = buffer.delete(line_len);
                buffer.set_position(Position::new(start_line, 0));
                let _cursor_after = buffer.position();
                drop(buffer);

                ctx.event_bus
                    .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));

                // TODO: Return edit action via different mechanism when SessionContext is available
                let _ = (buffer_id, cursor_before); // Suppress unused warnings
                return CommandResult::Success;
            }
            // Line is already empty
            drop(buffer);
            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));
            return CommandResult::Success;
        }

        // Multiple lines: delete lines 2..N entirely, then clear line 1
        // First, calculate total chars to delete from lines 2..N (including newlines)
        let mut chars_to_delete_from_rest = 0;
        for i in 1..lines_to_change {
            let line_idx = start_line + i;
            if line_idx < line_count {
                let line_len = buffer.line_len(line_idx).unwrap_or(0);
                chars_to_delete_from_rest += line_len;
                // Add 1 for newline
                if line_idx + 1 < line_count {
                    chars_to_delete_from_rest += 1;
                }
            }
        }

        // Delete from end of first line (newline) to end of last changed line
        let first_line_len = buffer.line_len(start_line).unwrap_or(0);
        let total_delete = first_line_len + 1 + chars_to_delete_from_rest; // +1 for newline after first line

        // Handle edge case: if deleting to end of buffer
        let end_line = start_line + lines_to_change;
        if end_line >= line_count {
            // We're changing to end of buffer - delete including last line's content
            // but keep one empty line
            buffer.set_position(Position::new(start_line, 0));
            let chars = first_line_len + chars_to_delete_from_rest + (lines_to_change - 1); // newlines between
            let content_len = buffer.content().len();
            let _edit = buffer.delete(chars.min(content_len));
            buffer.set_position(Position::new(start_line, 0));
            let _cursor_after = buffer.position();
            drop(buffer);

            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));

            // TODO: Return edit action via different mechanism when SessionContext is available
            let _ = (buffer_id, cursor_before); // Suppress unused warnings
            return CommandResult::Success;
        }

        // Normal case: delete all content from lines and their separating newlines
        // Keep one line at start_line, cleared
        buffer.set_position(Position::new(start_line, 0));
        let _edit = buffer.delete(total_delete);
        buffer.set_position(Position::new(start_line, 0));
        let _cursor_after = buffer.position();
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));

        // TODO: Return edit action via different mechanism when SessionContext is available
        let _ = (buffer_id, cursor_before); // Suppress unused warnings
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let pos = buffer.position();
        let line_len = buffer.line_len(pos.line).unwrap_or(0);

        // Nothing to delete if at or past end of line - just enter insert mode
        if pos.column >= line_len {
            drop(buffer);
            ctx.event_bus
                .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));
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
        ctx.registers.write().set_by_name(register, content);

        // Delete from cursor to end of line (not including newline)
        let chars_to_delete = line_len - pos.column;
        let _cursor_before = buffer.position();
        let _edit = buffer.delete(chars_to_delete);
        let _cursor_after = buffer.position();
        drop(buffer);

        ctx.event_bus
            .emit(ModeChanged::with_mode_id("normal", VimMode::INSERT_ID));

        // TODO: Return edit action via different mechanism when SessionContext is available
        CommandResult::Success
    }
}
