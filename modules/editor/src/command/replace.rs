//! Replace commands.
//!
//! Provides replace and repeat commands:
//! - `ReplaceCharStart` (r)
//! - `RepeatDot` (.)
//! - `JoinLines` (J)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{CommandId, Edit, KernelContext, Position},
};

use reovim_kernel::api::v1::ModuleId;

// Command module ID for editor commands.
const EDITOR_MODULE: ModuleId = ModuleId::new("editor");

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
        CommandId::new(EDITOR_MODULE, "replace-char-start")
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
    fn execute(&self, _ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let count = args.count().unwrap_or(1);
        CommandResult::waiting_for_replace_char(count)
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
        CommandId::new(EDITOR_MODULE, "repeat-dot")
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        // The runner handles the actual repeat logic.
        // We just signal the intent to repeat.
        CommandResult::RepeatAction
    }
}

/// Join current line with next line (J).
#[derive(Debug, Clone, Copy, Default)]
pub struct JoinLines;

impl Command for JoinLines {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "join-lines")
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let mut buffer = buffer_arc.write();
        let current_line = buffer.position().line;
        let mut line_count = buffer.line_count();
        let cursor_before = buffer.position();
        let mut edits: Vec<Edit> = Vec::new();

        for _ in 0..count {
            // Can't join if on last line
            if current_line + 1 >= line_count {
                break;
            }

            // Move to end of current line
            let line_len = buffer.line_len(current_line).unwrap_or(0);
            buffer.set_position(Position::new(current_line, line_len));

            // Delete newline (joins the lines)
            let edit = buffer.delete(1);
            edits.push(edit);

            // Delete leading whitespace of what was the next line
            let new_line_content = buffer.line(current_line).unwrap_or("");
            let after_join = &new_line_content[line_len..];
            let leading_ws = after_join.chars().take_while(|c| c.is_whitespace()).count();

            if leading_ws > 0 {
                let edit = buffer.delete(leading_ws);
                edits.push(edit);
            }

            // Insert single space between joined content (Vim behavior)
            // Only if there's content after the join point
            if buffer.line_len(current_line).unwrap_or(0) > line_len {
                let edit = buffer.insert(" ");
                edits.push(edit);
            }

            // Update line count for next iteration
            line_count = buffer.line_count();
        }

        let cursor_after = buffer.position();
        drop(buffer);

        if edits.is_empty() {
            return CommandResult::Success;
        }

        CommandResult::edit_actions(buffer_id, edits, cursor_before, cursor_after)
    }
}
