//! Delete operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use {
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{Edit, RegisterContent},
};

use super::{Operator, OperatorContext, OperatorError, Range, registers};

/// Delete operator - cuts text to register.
///
/// Behavior:
/// - Deletes text in the given range
/// - Stores deleted text in the unnamed register (or specified register)
/// - Linewise if the motion was linewise
///
/// # Example
///
/// ```ignore
/// let delete = DeleteOperator;
/// delete.execute(&mut ctx, range)?;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DeleteOperator;

impl Operator for DeleteOperator {
    fn id(&self) -> &'static str {
        "delete"
    }

    #[allow(clippy::too_many_lines, clippy::option_if_let_else)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        // Get the buffer via kernel's buffer manager
        let buffer_arc = ctx
            .kernel
            .buffers
            .get(ctx.buffer_id)
            .ok_or(OperatorError::BufferNotFound(ctx.buffer_id))?;

        let mut buffer = buffer_arc.write();

        // Get text before deleting
        let start = range.start;
        let end = range.end;

        // Build deleted text from lines
        // - register_text: for register storage (linewise = "line\n")
        // - deleted_text: for undo (exact bytes deleted)
        let mut register_text = String::new();
        let mut deleted_text = String::new();
        let lines = buffer.lines();

        if range.is_linewise {
            // Linewise deletion: delete entire lines from start.line to end.line (inclusive)
            // Ignore column values - always delete full lines
            let line_count = lines.len();

            // Clamp end.line to last valid line to handle counts exceeding buffer
            let clamped_end = end.line.min(line_count.saturating_sub(1));

            // Build register_text as "line\n" for each line (for paste to work correctly)
            for line_idx in start.line..=clamped_end {
                if let Some(line) = lines.get(line_idx) {
                    register_text.push_str(line);
                    register_text.push('\n');
                }
            }

            // For linewise, adjust the actual deletion range to cover full lines.
            // Three cases based on Vim semantics:
            //
            // Case 1: Deleting non-last lines (e.g., dd on line 0 of 3-line buffer)
            //   - Delete from start of first line through the newline
            //   - Range: (start.line, 0) to (clamped_end + 1, 0)
            //   - Removes: "line\n" leaving subsequent lines
            //   - deleted_text: "line\n"
            //
            // Case 2: Deleting last line(s) but not all (e.g., dd on line 1 of 2-line buffer)
            //   - Include the PRECEDING newline from previous line
            //   - Range: (start.line - 1, prev_len) to (clamped_end, last_len)
            //   - Removes: "\nline" leaving previous lines intact
            //   - deleted_text: "\nline" (for correct undo)
            //
            // Case 3: Deleting the only line (single-line buffer)
            //   - Delete entire content
            //   - Range: (0, 0) to (0, content_len)
            //   - Results in empty buffer
            //   - deleted_text: "line"
            let delete_start;
            let delete_end;

            if clamped_end + 1 < line_count {
                // Case 1: Deleting non-last lines - delete through newline to next line
                delete_start = reovim_kernel::api::v1::Position::new(start.line, 0);
                delete_end = reovim_kernel::api::v1::Position::new(clamped_end + 1, 0);
                // deleted_text matches what we're deleting: "line\n"
                deleted_text.clone_from(&register_text);
            } else if start.line > 0 {
                // Case 2: Deleting last line(s) but not all
                // Include the preceding newline (from end of previous line)
                // Use .chars().count() for UTF-8 safety
                let prev_line_len = lines.get(start.line - 1).map_or(0, |l| l.chars().count());
                delete_start = reovim_kernel::api::v1::Position::new(start.line - 1, prev_line_len);
                // clamped_end is always valid: end.line.min(line_count - 1) < lines.len()
                let last_line = &lines[clamped_end];
                delete_end =
                    reovim_kernel::api::v1::Position::new(clamped_end, last_line.chars().count());
                // Build deleted_text as "\nline" (preceding newline + content, no trailing newline)
                // This matches what we're actually deleting for correct undo
                for line_idx in start.line..=clamped_end {
                    deleted_text.push('\n');
                    if let Some(line) = lines.get(line_idx) {
                        deleted_text.push_str(line);
                    }
                }
            } else {
                // Case 3: Deleting all lines (start.line == 0 and clamped_end is last line)
                delete_start = reovim_kernel::api::v1::Position::new(0, 0);
                // clamped_end is always valid: end.line.min(line_count - 1) < lines.len()
                let last_line = &lines[clamped_end];
                delete_end =
                    reovim_kernel::api::v1::Position::new(clamped_end, last_line.chars().count());
                // deleted_text is just the content (no newlines - single line)
                if let Some(line) = lines.get(clamped_end) {
                    deleted_text.push_str(line);
                }
            }

            // Track which case for cursor positioning
            let is_deleting_last_line = (clamped_end + 1 >= line_count) && start.line > 0;

            // Store in register as linewise (handles +/* via ClipboardProvider)
            let content = RegisterContent::linewise(register_text);
            registers::store_and_sync(ctx.kernel, ctx.registers, ctx.register, &content);
            registers::push_to_history(ctx.clipboard_history, &content);

            // Use cursor position from context (passed from caller who has window access)
            let cursor_before = ctx.cursor_position;

            // Delete entire lines
            buffer.delete_range(delete_start, delete_end);

            // Cursor positioning after linewise delete follows Vim behavior:
            //
            // Case 1 (delete non-last lines): Cursor at column 0 of the line that
            //   takes the place of the deleted lines (i.e., the first remaining line).
            //
            // Case 2 (delete last lines but not all): Cursor at the last valid column
            //   of the new last line, since there's no line below to move to.
            //
            // Case 3 (delete all lines): Buffer is empty, cursor at (0, 0).
            let line_count = buffer.line_count();
            let final_line = start.line.min(line_count.saturating_sub(1));
            // Note: Buffer always maintains at least one line (even if empty),
            // so line_count is always >= 1 after delete_range.
            let final_col = if is_deleting_last_line {
                // Case 2: Cursor at last valid column of the new last line
                let line_len = buffer.line_len(final_line).unwrap_or(0);
                if line_len == 0 {
                    0
                } else {
                    line_len.saturating_sub(1)
                }
            } else {
                // Case 1: Cursor at column 0
                0
            };
            // Cursor position after delete — used for both undo tracking and
            // communicating desired cursor back to execute_operator (#552)
            let cursor_after = reovim_kernel::api::v1::Position::new(final_line, final_col);
            ctx.cursor_after = Some(cursor_after);

            // Record edit for undo
            if let Some(undo_registry) = ctx.kernel.services.get::<UndoProviderRegistry>()
                && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
            {
                let edit = Edit::Delete {
                    position: delete_start,
                    text: deleted_text.clone(),
                };
                undo_provider.record(ctx.buffer_id, vec![edit], cursor_before, cursor_after);
            }
        } else {
            // Characterwise deletion
            if start.line == end.line {
                // Single line deletion
                // start.line is valid: buffer exists and lines were just obtained from it
                let line = &lines[start.line];
                let start_col = start.column.min(line.len());
                let end_col = end.column.min(line.len());
                if start_col < end_col {
                    deleted_text.push_str(&line[start_col..end_col]);
                }
            } else {
                // Multi-line deletion
                // All indices in start.line..=end.line are valid: lines were obtained
                // from the same buffer snapshot and end.line <= last valid line
                for (line_idx, line) in lines.iter().enumerate().take(end.line + 1).skip(start.line)
                {
                    if line_idx == start.line {
                        let start_col = start.column.min(line.len());
                        deleted_text.push_str(&line[start_col..]);
                        deleted_text.push('\n');
                    } else if line_idx == end.line {
                        let end_col = end.column.min(line.len());
                        deleted_text.push_str(&line[..end_col]);
                    } else {
                        deleted_text.push_str(line);
                        deleted_text.push('\n');
                    }
                }
            }

            // Store in register as characterwise (handles +/* via ClipboardProvider)
            let content = RegisterContent::characterwise(deleted_text.clone());
            registers::store_and_sync(ctx.kernel, ctx.registers, ctx.register, &content);
            registers::push_to_history(ctx.clipboard_history, &content);

            // Use cursor position from context (passed from caller who has window access)
            let cursor_before = ctx.cursor_position;

            // Delete the text from buffer
            buffer.delete_range(start, end);

            // Cursor after characterwise delete is at the start position (#552)
            let cursor_after = start;
            ctx.cursor_after = Some(cursor_after);

            // Record edit for undo
            if let Some(undo_registry) = ctx.kernel.services.get::<UndoProviderRegistry>()
                && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
            {
                let edit = Edit::Delete {
                    position: start,
                    text: deleted_text,
                };
                undo_provider.record(ctx.buffer_id, vec![edit], cursor_before, cursor_after);
            }
        }

        drop(buffer);
        Ok(())
    }

    fn is_linewise(&self) -> bool {
        false // Default; actual linewise-ness is determined by motion
    }

    fn is_text_modifying(&self) -> bool {
        true
    }
}
