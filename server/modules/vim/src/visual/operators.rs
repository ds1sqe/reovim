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
        api::{ChangeTracker, ModeApi, RegisterContent, Selection, SelectionMode},
    },
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::{ids, modes::VimMode};

/// Calculate the expanded range for a selection.
///
/// This converts an API Selection into start/end positions suitable for
/// text extraction and deletion, taking selection mode into account:
/// - Character mode: End is already exclusive, use as-is
/// - Line mode: Expand to full lines including trailing newline
/// - Block mode: End is already exclusive, use as-is
///
/// Phase 8 (#465): Selection.end is EXCLUSIVE (like Rust ranges).
/// The selection (0,0) to (0,5) means columns 0..5 = "hello" (5 chars).
///
/// Returns `(start, end, is_linewise)`.
fn expand_selection_range(
    selection: &Selection,
    end_line_len: Option<usize>,
    total_lines: usize,
) -> (Position, Position, bool) {
    let start = selection.start;
    let end = selection.end;

    match selection.mode {
        SelectionMode::Line => {
            // Expand to full lines, including the trailing newline
            let start = Position::new(start.line, 0);
            // For line mode, end.line is already the exclusive end line
            // For non-last lines, extend to start of end line (includes previous line's newline)
            // For last line, end at line length
            let end_line_len = end_line_len.unwrap_or(0);
            let end = if end.line < total_lines {
                Position::new(end.line, 0)
            } else {
                // End is past buffer, cap at last line's length
                Position::new(end.line - 1, end_line_len)
            };
            (start, end, true)
        }
        // Character and Block modes: End is already exclusive - use as-is
        SelectionMode::Character | SelectionMode::Block => (start, end, false),
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line info for expanding selection
        let end_line_len = runtime.buffer_line_len(buffer_id, selection.end.line);
        let total_lines = runtime.buffer_line_count(buffer_id).unwrap_or(1);

        // Expand selection to deletion range
        let (start, end, is_linewise) =
            expand_selection_range(&selection, end_line_len, total_lines);
        let cursor_pos = start;

        // Extract text for register with clipboard sync (#515)
        if let Some(text) = runtime.buffer_text_range(buffer_id, start, end) {
            let content = if is_linewise {
                RegisterContent::linewise(&text)
            } else {
                RegisterContent::characterwise(&text)
            };
            runtime.store_register_with_sync(args.register(), content);
        }

        // Delete the range
        runtime.delete_range(buffer_id, start, end);

        // Clear selection and set cursor
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
            window.cursor = cursor_pos.into();
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line info for expanding selection
        let end_line_len = runtime.buffer_line_len(buffer_id, selection.end.line);
        let total_lines = runtime.buffer_line_count(buffer_id).unwrap_or(1);

        // Expand selection to yank range
        let (start, end, is_linewise) =
            expand_selection_range(&selection, end_line_len, total_lines);

        // Extract text for register with clipboard sync (#515)
        if let Some(text) = runtime.buffer_text_range(buffer_id, start, end) {
            let content = if is_linewise {
                RegisterContent::linewise(&text)
            } else {
                RegisterContent::characterwise(&text)
            };
            runtime.store_register_with_sync(args.register(), content);
        }

        // Clear selection (yank doesn't delete text or move cursor)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line info for expanding selection
        let end_line_len = runtime.buffer_line_len(buffer_id, selection.end.line);
        let total_lines = runtime.buffer_line_count(buffer_id).unwrap_or(1);

        // Expand selection to deletion range
        let (start, end, is_linewise) =
            expand_selection_range(&selection, end_line_len, total_lines);
        let cursor_pos = start;

        // Extract text for register with clipboard sync (#515)
        if let Some(text) = runtime.buffer_text_range(buffer_id, start, end) {
            let content = if is_linewise {
                RegisterContent::linewise(&text)
            } else {
                RegisterContent::characterwise(&text)
            };
            runtime.store_register_with_sync(args.register(), content);
        }

        // Delete the range
        runtime.delete_range(buffer_id, start, end);

        // Clear selection and set cursor
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
            window.cursor = cursor_pos.into();
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line range from normalized selection
        // Phase 8 (#465): Selection.end is EXCLUSIVE (like Rust ranges)
        let start_line = selection.start.line;
        let end_line = selection.end.line; // exclusive

        // Indent each line (add tab/spaces at start)
        // Using 4 spaces as default indent
        let indent = "    ";
        for line_idx in start_line..end_line {
            runtime.insert_text(buffer_id, Position::new(line_idx, 0), indent);
        }

        // Clear selection
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line range from normalized selection
        // Phase 8 (#465): Selection.end is EXCLUSIVE (like Rust ranges)
        let start_line = selection.start.line;
        let end_line = selection.end.line; // exclusive

        // Dedent each line (remove leading whitespace, up to 4 chars or one tab)
        for line_idx in start_line..end_line {
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
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
#[path = "tests/operators.rs"]
mod tests;
