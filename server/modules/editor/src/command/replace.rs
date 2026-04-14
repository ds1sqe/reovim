//! Replace commands.
//!
//! Provides replace and repeat commands:
//! - `ReplaceCharStart` (r) - signals waiting for char
//! - `ReplaceChar` - performs the actual replacement
//! - `RepeatDot` (.)
//! - `JoinLines` (J)

use {
    reovim_domain_text::Position,
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_text_session::{SessionRuntime, api::BufferApi},
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

/// Start replace char operation (r).
///
/// This command signals that the next character typed should replace
/// the character(s) under the cursor. Returns `WaitingForChar` with
/// the `ReplaceChar` operation type.
///
/// Unlike `R` (replace mode), `r` is a single-character replacement:
/// - `rx` replaces the char under cursor with 'x'
/// - `3rx` replaces the next 3 chars with 'x'
#[derive(Debug, Clone, Copy, Default)]
pub struct ReplaceCharStart;

impl Command for ReplaceCharStart {
    fn id(&self) -> CommandId {
        ids::REPLACE_CHAR_START
    }

    fn description(&self) -> &'static str {
        "Replace character under cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to replace",
        )]
    }
}

impl CommandHandler for ReplaceCharStart {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will signal waiting for replace character with count
        let _count = args.count().unwrap_or(1);
        CommandResult::Success
    }
}

/// Replace character under cursor (r{char}).
///
/// The resolver intercepts `r`, waits for the next character, then
/// dispatches this command with `replace_char` in the context metadata.
///
/// Behavior:
/// - `rx` replaces the char under cursor with 'x'
/// - `3rx` replaces the next 3 chars with 'x'
/// - At end of line: replaces only available chars up to line end
#[derive(Debug, Clone, Copy, Default)]
pub struct ReplaceChar;

impl Command for ReplaceChar {
    fn id(&self) -> CommandId {
        ids::REPLACE_CHAR
    }

    fn description(&self) -> &'static str {
        "Replace character(s) under cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::required("replace_char", ArgKind::Char, "Replacement character"),
            ArgSpec::optional("count", ArgKind::Count, "Number of characters to replace"),
        ]
    }
}

impl CommandHandler for ReplaceChar {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(replacement) = args.char("replace_char") else {
            return CommandResult::error("No replacement character");
        };

        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let cursor = Position::new(window.cursor.line, window.cursor.column);

        let count = args.count().unwrap_or(1);

        let line_len = runtime.buffer_line_len(buffer_id, cursor.line).unwrap_or(0);
        if line_len == 0 {
            return CommandResult::Success;
        }

        // Clamp count to remaining chars on the line
        let available = line_len.saturating_sub(cursor.column);
        let actual_count = count.min(available);
        if actual_count == 0 {
            return CommandResult::Success;
        }

        // Delete `actual_count` chars at cursor
        let delete_end = Position::new(cursor.line, cursor.column + actual_count);
        runtime.delete_range(buffer_id, cursor, delete_end);

        // Insert `actual_count` copies of replacement char
        let replacement_text: String = std::iter::repeat_n(replacement, actual_count).collect();
        runtime.insert_text(buffer_id, cursor, &replacement_text);

        // Cursor stays at original position (Vim behavior: cursor doesn't move on `r`)
        CommandResult::Success
    }
}

/// Repeat the last repeatable command (.).
///
/// This command returns `RepeatAction` which signals the runner to
/// replay the last repeatable command from `repeat_state`. Repeatable
/// commands include text-modifying operations like insert, delete, change.
///
/// # Vim Behavior
///
/// - `.` repeats the last change command
/// - `3.` repeats the last change 3 times
/// - Insert mode text is recorded and replayed
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatDot;

impl Command for RepeatDot {
    fn id(&self) -> CommandId {
        ids::REPEAT_DOT
    }

    fn description(&self) -> &'static str {
        "Repeat last change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of times to repeat",
        )]
    }
}

impl CommandHandler for RepeatDot {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will signal intent to repeat last change
        CommandResult::Success
    }
}

/// Join current line with next line (J).
#[derive(Debug, Clone, Copy, Default)]
pub struct JoinLines;

impl Command for JoinLines {
    fn id(&self) -> CommandId {
        ids::JOIN_LINES
    }

    fn description(&self) -> &'static str {
        "Join current line with next line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines to join",
        )]
    }
}

impl CommandHandler for JoinLines {
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
        let current_line = pos.line;

        let Some(mut line_count) = runtime.buffer_line_count(buffer_id) else {
            return CommandResult::error("Failed to get line count");
        };

        let mut joined_any = false;

        for _ in 0..count {
            // Can't join if on last line
            if current_line + 1 >= line_count {
                break;
            }

            // Get line length via BufferApi
            let line_len = runtime
                .buffer_line_len(buffer_id, current_line)
                .unwrap_or(0);

            // Delete newline (joins the lines) - delete from end of current line to start of next
            let newline_start = Position::new(current_line, line_len);
            let newline_end = Position::new(current_line + 1, 0);
            runtime.delete_range(buffer_id, newline_start, newline_end);

            // Get the joined line content to find leading whitespace
            let joined_line = runtime
                .buffer_line(buffer_id, current_line)
                .unwrap_or_default();
            let after_join = if joined_line.len() > line_len {
                &joined_line[line_len..]
            } else {
                ""
            };
            let leading_ws = after_join.chars().take_while(|c| c.is_whitespace()).count();

            // Delete leading whitespace of what was the next line
            if leading_ws > 0 {
                let ws_start = Position::new(current_line, line_len);
                let ws_end = Position::new(current_line, line_len + leading_ws);
                runtime.delete_range(buffer_id, ws_start, ws_end);
            }

            // Insert single space between joined content (Vim behavior)
            // Only if there's content after the join point
            let new_line_len = runtime
                .buffer_line_len(buffer_id, current_line)
                .unwrap_or(0);
            if new_line_len > line_len {
                let insert_pos = Position::new(current_line, line_len);
                runtime.insert_text(buffer_id, insert_pos, " ");
            }

            // Update line count for next iteration
            line_count = runtime.buffer_line_count(buffer_id).unwrap_or(line_count);
            joined_any = true;
        }

        if !joined_any {
            return CommandResult::Success;
        }

        // Position cursor at join point
        let final_line_len = runtime
            .buffer_line_len(buffer_id, current_line)
            .unwrap_or(0);
        let cursor_pos =
            Position::new(current_line, pos.column.min(final_line_len.saturating_sub(1)));
        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = cursor_pos.into();
        }

        CommandResult::Success
    }
}
