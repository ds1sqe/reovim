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
    reovim_driver_session::{SessionRuntime, TransitionContext, api::ModeApi},
    reovim_kernel::api::v1::{BufferId, CommandId, KernelContext, Position, SelectionMode},
};

use crate::operators::{DeleteOperator, Operator, OperatorContext, Range, YankOperator};

use crate::{ids, modes::VimMode};

/// Helper function to get selection range from buffer.
///
/// Returns (range, `cursor_position`) if selection is active, None otherwise.
pub(super) fn get_selection_range(
    ctx: &KernelContext,
    buffer_id: BufferId,
) -> Option<(Range, Position)> {
    let buffer_arc = ctx.buffers.get(buffer_id)?;
    let buffer = buffer_arc.read();

    let selection = buffer.selection();
    if !selection.is_active() {
        return None;
    }

    let cursor = buffer.position();
    let anchor = selection.anchor;
    let mode = selection.mode();

    // Normalize: start should be before end
    let (start, end) = if anchor < cursor {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    };

    // Get line info for Line mode (must be done before dropping buffer)
    let end_line_len = buffer.line_len(end.line).unwrap_or(0);
    let total_lines = buffer.line_count();
    drop(buffer);

    // Create range based on selection mode
    let range = match mode {
        SelectionMode::Character => {
            // Include the character at end position
            Range::new(start, Position::new(end.line, end.column + 1))
        }
        SelectionMode::Line => {
            // Expand to full lines, including the trailing newline
            let start = Position::new(start.line, 0);
            // For non-last lines, extend to start of next line (includes newline)
            // For last line, end at line length
            let end = if end.line + 1 < total_lines {
                Position::new(end.line + 1, 0)
            } else {
                Position::new(end.line, end_line_len)
            };
            Range::linewise(start, end)
        }
        SelectionMode::Block => {
            // Block mode: for now, treat as character range
            // Full block support is deferred
            Range::new(start, Position::new(end.line, end.column + 1))
        }
    };

    Some((range, start))
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

        let kernel = runtime.kernel();

        let Some((range, cursor_pos)) = get_selection_range(kernel, buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Execute delete operator
        let delete_op = DeleteOperator;
        let mut op_ctx = OperatorContext {
            kernel,
            buffer_id,
            register: args.register(),
            count: 1,
        };

        if let Err(e) = delete_op.execute(&mut op_ctx, range) {
            return CommandResult::error(&format!("Delete failed: {e}"));
        }

        // Clear selection and set cursor
        if let Some(buffer_arc) = kernel.buffers.get(buffer_id) {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().clear();
            buffer.set_position(cursor_pos);
        }

        // Mode transition to Normal with target ModeId
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

        let kernel = runtime.kernel();

        let Some((range, _cursor_pos)) = get_selection_range(kernel, buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Execute yank operator
        let yank_op = YankOperator;
        let mut op_ctx = OperatorContext {
            kernel,
            buffer_id,
            register: args.register(),
            count: 1,
        };

        if let Err(e) = yank_op.execute(&mut op_ctx, range) {
            return CommandResult::error(&format!("Yank failed: {e}"));
        }

        // Clear selection (yank doesn't move cursor in Vim, but returns to normal)
        if let Some(buffer_arc) = kernel.buffers.get(buffer_id) {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().clear();
        }

        // Mode transition to Normal with target ModeId
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

        let kernel = runtime.kernel();

        let Some((range, cursor_pos)) = get_selection_range(kernel, buffer_id) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Execute delete operator (change = delete + insert mode)
        let delete_op = DeleteOperator;
        let mut op_ctx = OperatorContext {
            kernel,
            buffer_id,
            register: args.register(),
            count: 1,
        };

        if let Err(e) = delete_op.execute(&mut op_ctx, range) {
            return CommandResult::error(&format!("Change failed: {e}"));
        }

        // Clear selection and set cursor
        if let Some(buffer_arc) = kernel.buffers.get(buffer_id) {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().clear();
            buffer.set_position(cursor_pos);
        }

        // Mode transition to Insert with target ModeId
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

        let kernel = runtime.kernel();

        let Some(buffer_arc) = kernel.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let selection = buffer.selection();

        if !selection.is_active() {
            return CommandResult::Success; // No selection - no-op
        }

        let cursor = buffer.position();
        let anchor = selection.anchor;

        // Get line range
        let start_line = anchor.line.min(cursor.line);
        let end_line = anchor.line.max(cursor.line);

        // Indent each line (add tab/spaces at start)
        // Using 4 spaces as default indent
        let indent = "    ";
        for line_idx in start_line..=end_line {
            buffer.insert_at(Position::new(line_idx, 0), indent);
        }

        // Clear selection
        buffer.selection_mut().clear();

        // Mode transition to Normal is handled by event loop
        drop(buffer);
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

        let kernel = runtime.kernel();

        let Some(buffer_arc) = kernel.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let mut buffer = buffer_arc.write();
        let selection = buffer.selection();

        if !selection.is_active() {
            return CommandResult::Success; // No selection - no-op
        }

        let cursor = buffer.position();
        let anchor = selection.anchor;

        // Get line range
        let start_line = anchor.line.min(cursor.line);
        let end_line = anchor.line.max(cursor.line);

        // Dedent each line (remove leading whitespace, up to 4 chars or one tab)
        for line_idx in start_line..=end_line {
            if let Some(line) = buffer.lines().get(line_idx) {
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
                    buffer.delete_at(Position::new(line_idx, 0), chars_to_remove);
                }
            }
        }

        // Clear selection
        buffer.selection_mut().clear();

        // Mode transition to Normal is handled by event loop
        drop(buffer);
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}
