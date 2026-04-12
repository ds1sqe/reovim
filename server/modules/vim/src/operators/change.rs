//! Change operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use {
    reovim_domain_text::{Edit, Position, RegisterContent},
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
};

use super::{Operator, OperatorContext, OperatorError, Range, char_col_to_byte, registers};

/// Change operator - cuts text and signals insert mode.
///
/// Behavior:
/// - Deletes text in the given range
/// - Stores deleted text in the unnamed register (or specified register)
/// - Signals that insert mode should be entered (via return value or event)
///
/// Note: The actual mode change is handled by the caller (server/display driver).
/// The operator just deletes the text.
///
/// # Example
///
/// ```ignore
/// let change = ChangeOperator;
/// change.execute(&mut ctx, range)?;
/// // Caller should now enter insert mode
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ChangeOperator;

impl Operator for ChangeOperator {
    fn id(&self) -> &'static str {
        "change"
    }

    #[allow(clippy::option_if_let_else)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        // Get the buffer via kernel's buffer manager
        let buffer_arc = ctx.text_buffer()?;

        let mut buffer = buffer_arc.write();

        // Use cursor position from context (passed from caller who has window access)
        let cursor_before = ctx.cursor_position;

        // Get text before deleting
        let start = range.start;
        let end = range.end;

        // Build deleted text from lines
        let mut deleted_text = String::new();
        let line_count = buffer.line_count();

        // Track what was actually deleted for undo
        let delete_pos;

        if range.is_linewise {
            // Linewise change: delete content of lines from start.line to end.line (inclusive)
            // Unlike delete, change keeps ONE line for insertion (Vim behavior)
            // Clamp end.line to last valid line to handle counts exceeding buffer
            let clamped_end = end.line.min(line_count.saturating_sub(1));

            for line_idx in start.line..=clamped_end {
                if let Some(line) = buffer.line(line_idx) {
                    deleted_text.push_str(&line);
                    deleted_text.push('\n');
                }
            }

            // For linewise change (cc):
            // - Delete content of all affected lines
            // - Keep ONE empty line at start.line for insertion
            // This means: delete from (start.line, 0) to (clamped_end, end_of_content),
            // then if multiple lines, delete the extra newlines to leave just one line
            let delete_start = Position::new(start.line, 0);
            let delete_end = if clamped_end + 1 < line_count {
                // Not the last line - delete content but preserve start.line's newline
                // Delete all lines but keep start.line as empty (with its newline)
                let end_line_char_len = buffer.line(clamped_end).map_or(0, |l| l.chars().count());
                Position::new(clamped_end, end_line_char_len)
            } else {
                // End line is last line - delete to end of content (keep line structure)
                let last_line_char_len = buffer.line(clamped_end).map_or(0, |l| l.chars().count());
                Position::new(clamped_end, last_line_char_len)
            };

            delete_pos = delete_start;
            buffer.delete_range(delete_start, delete_end);
        } else if start.line == end.line {
            // Single line characterwise change
            if let Some(line) = buffer.line(start.line) {
                let char_len = line.chars().count();
                let start_col = start.column.min(char_len);
                let end_col = end.column.min(char_len);
                if start_col < end_col {
                    let start_byte = char_col_to_byte(&line, start_col);
                    let end_byte = char_col_to_byte(&line, end_col);
                    deleted_text.push_str(&line[start_byte..end_byte]);
                }
            }
            delete_pos = start;
            buffer.delete_range(start, end);
        } else {
            // Multi-line characterwise change
            for line_idx in start.line..=end.line {
                if let Some(line) = buffer.line(line_idx) {
                    if line_idx == start.line {
                        let char_len = line.chars().count();
                        let start_col = start.column.min(char_len);
                        let start_byte = char_col_to_byte(&line, start_col);
                        deleted_text.push_str(&line[start_byte..]);
                        deleted_text.push('\n');
                    } else if line_idx == end.line {
                        let char_len = line.chars().count();
                        let end_col = end.column.min(char_len);
                        let end_byte = char_col_to_byte(&line, end_col);
                        deleted_text.push_str(&line[..end_byte]);
                    } else {
                        deleted_text.push_str(&line);
                        deleted_text.push('\n');
                    }
                }
            }
            delete_pos = start;
            buffer.delete_range(start, end);
        }

        // Cursor after change is at delete_pos (where insertion will happen) (#552)
        let cursor_after = delete_pos;
        ctx.cursor_after = Some(cursor_after);

        drop(buffer);

        // Record edit for undo
        if !deleted_text.is_empty()
            && let Some(undo_registry) = ctx.kernel.services.get::<UndoProviderRegistry>()
            && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
        {
            let edit = Edit::Delete {
                position: delete_pos,
                text: deleted_text.clone(),
            };
            undo_provider.record(ctx.buffer_id, vec![edit], cursor_before, cursor_after);
        }

        // Store in register - linewise if the range was linewise (handles +/* via ClipboardProvider)
        let content = if range.is_linewise {
            RegisterContent::linewise(deleted_text)
        } else {
            RegisterContent::characterwise(deleted_text)
        };

        registers::store_and_sync(ctx.kernel, ctx.registers, ctx.register, &content);
        registers::push_to_history(ctx.clipboard_history, &content);

        // Note: Insert mode transition is handled by the caller
        // The server/display driver should check operator id and enter insert mode

        Ok(())
    }

    fn is_linewise(&self) -> bool {
        false // Default; actual linewise-ness is determined by motion
    }

    fn is_text_modifying(&self) -> bool {
        true
    }
}
