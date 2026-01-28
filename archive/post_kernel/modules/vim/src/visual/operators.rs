//! Visual mode operator commands.
//!
//! Provides commands that operate on the visual selection:
//! - `d` - Delete selection
//! - `y` - Yank (copy) selection
//! - `c` - Change selection (delete + insert mode)
//! - `>` - Indent selection
//! - `<` - Dedent selection

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        BufferApi, SessionRuntime, TransitionContext,
        api::{ModeApi, RegisterApi, RegisterContent, Selection, SelectionMode},
    },
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::{ids, modes::VimMode};

/// Calculate the expanded range for a selection.
///
/// This converts an API Selection into start/end positions suitable for
/// text extraction and deletion, taking selection mode into account:
/// - Character mode: Include character at end position
/// - Line mode: Expand to full lines including trailing newline
/// - Block mode: Treat as character range (full block support deferred)
///
/// Returns (start, end, is_linewise).
fn expand_selection_range(
    selection: &Selection,
    end_line_len: Option<usize>,
    total_lines: usize,
) -> (Position, Position, bool) {
    let start = selection.start;
    let end = selection.end;

    match selection.mode {
        SelectionMode::Character => {
            // Include the character at end position
            (start, Position::new(end.line, end.column + 1), false)
        }
        SelectionMode::Line => {
            // Expand to full lines, including the trailing newline
            let start = Position::new(start.line, 0);
            // For non-last lines, extend to start of next line (includes newline)
            // For last line, end at line length
            let end_line_len = end_line_len.unwrap_or(0);
            let end = if end.line + 1 < total_lines {
                Position::new(end.line + 1, 0)
            } else {
                Position::new(end.line, end_line_len)
            };
            (start, end, true)
        }
        SelectionMode::Block => {
            // Block mode: for now, treat as character range
            // Full block support is deferred
            (start, Position::new(end.line, end.column + 1), false)
        }
    }
}

/// Delete selection (d in visual mode).
///
/// Deletes the selected text and stores it in the register.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteSelection;

impl Command for DeleteSelection {
    fn id(&self) -> CommandId {
        ids::DELETE_SELECTION
    }

    fn description(&self) -> &'static str {
        "Delete visual selection"
    }
}

impl CommandHandler for DeleteSelection {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection using API
        let Some(selection) = runtime.selection(buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line info for expanding selection
        let end_line_len = runtime.buffer_line_len(buffer_id, selection.end.line);
        let total_lines = runtime.buffer_line_count(buffer_id).unwrap_or(1);

        // Expand selection to deletion range
        let (start, end, is_linewise) =
            expand_selection_range(&selection, end_line_len, total_lines);
        let cursor_pos = start;

        // Extract text for register
        if let Some(text) = runtime.buffer_text_range(buffer_id, start, end) {
            let content = if is_linewise {
                RegisterContent::linewise(&text)
            } else {
                RegisterContent::characterwise(&text)
            };
            runtime.set_register(args.register(), content);
        }

        // Delete the range
        runtime.delete_range(buffer_id, start, end);

        // Clear selection and set cursor
        runtime.set_selection(buffer_id, None);
        runtime.set_buffer_position(buffer_id, cursor_pos);

        // Mode transition to Normal
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Yank selection (y in visual mode).
///
/// Copies the selected text to the register without deleting it.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct YankSelection;

impl Command for YankSelection {
    fn id(&self) -> CommandId {
        ids::YANK_SELECTION
    }

    fn description(&self) -> &'static str {
        "Yank visual selection"
    }
}

impl CommandHandler for YankSelection {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection using API
        let Some(selection) = runtime.selection(buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line info for expanding selection
        let end_line_len = runtime.buffer_line_len(buffer_id, selection.end.line);
        let total_lines = runtime.buffer_line_count(buffer_id).unwrap_or(1);

        // Expand selection to yank range
        let (start, end, is_linewise) =
            expand_selection_range(&selection, end_line_len, total_lines);

        // Extract text for register
        if let Some(text) = runtime.buffer_text_range(buffer_id, start, end) {
            let content = if is_linewise {
                RegisterContent::linewise(&text)
            } else {
                RegisterContent::characterwise(&text)
            };
            runtime.set_register(args.register(), content);
        }

        // Clear selection (yank doesn't delete text or move cursor)
        runtime.set_selection(buffer_id, None);

        // Mode transition to Normal
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Change selection (c in visual mode).
///
/// Deletes the selected text and enters Insert mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeSelection;

impl Command for ChangeSelection {
    fn id(&self) -> CommandId {
        ids::CHANGE_SELECTION
    }

    fn description(&self) -> &'static str {
        "Change visual selection (delete and enter insert mode)"
    }
}

impl CommandHandler for ChangeSelection {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection using API
        let Some(selection) = runtime.selection(buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line info for expanding selection
        let end_line_len = runtime.buffer_line_len(buffer_id, selection.end.line);
        let total_lines = runtime.buffer_line_count(buffer_id).unwrap_or(1);

        // Expand selection to deletion range
        let (start, end, is_linewise) =
            expand_selection_range(&selection, end_line_len, total_lines);
        let cursor_pos = start;

        // Extract text for register (change stores deleted text like delete)
        if let Some(text) = runtime.buffer_text_range(buffer_id, start, end) {
            let content = if is_linewise {
                RegisterContent::linewise(&text)
            } else {
                RegisterContent::characterwise(&text)
            };
            runtime.set_register(args.register(), content);
        }

        // Delete the range
        runtime.delete_range(buffer_id, start, end);

        // Clear selection and set cursor
        runtime.set_selection(buffer_id, None);
        runtime.set_buffer_position(buffer_id, cursor_pos);

        // Mode transition to Insert (change = delete + insert mode)
        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Indent selection (> in visual mode).
///
/// Increases indentation of selected lines.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct IndentSelection;

impl Command for IndentSelection {
    fn id(&self) -> CommandId {
        ids::INDENT_SELECTION
    }

    fn description(&self) -> &'static str {
        "Indent visual selection"
    }
}

impl CommandHandler for IndentSelection {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection to determine line range
        let Some(selection) = runtime.selection(buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line range from normalized selection
        let start_line = selection.start.line;
        let end_line = selection.end.line;

        // Indent each line (add tab/spaces at start)
        // Using 4 spaces as default indent
        let indent = "    ";
        for line_idx in start_line..=end_line {
            runtime.insert_text(buffer_id, Position::new(line_idx, 0), indent);
        }

        // Clear selection
        runtime.set_selection(buffer_id, None);

        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Dedent selection (< in visual mode).
///
/// Decreases indentation of selected lines.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct DedentSelection;

impl Command for DedentSelection {
    fn id(&self) -> CommandId {
        ids::DEDENT_SELECTION
    }

    fn description(&self) -> &'static str {
        "Dedent visual selection"
    }
}

impl CommandHandler for DedentSelection {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection to determine line range
        let Some(selection) = runtime.selection(buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line range from normalized selection
        let start_line = selection.start.line;
        let end_line = selection.end.line;

        // Dedent each line (remove leading whitespace, up to 4 chars or one tab)
        for line_idx in start_line..=end_line {
            if let Some(line) = runtime.buffer_line(buffer_id, line_idx) {
                let mut chars_to_remove = 0;
                for (i, c) in line.chars().enumerate() {
                    if c == '\t' {
                        chars_to_remove = i + 1;
                        break;
                    } else if c == ' ' && i < 4 {
                        chars_to_remove = i + 1;
                    } else {
                        break;
                    }
                }
                if chars_to_remove > 0 {
                    let start = Position::new(line_idx, 0);
                    let end = Position::new(line_idx, chars_to_remove);
                    runtime.delete_range(buffer_id, start, end);
                }
            }
        }

        // Clear selection
        runtime.set_selection(buffer_id, None);

        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}
