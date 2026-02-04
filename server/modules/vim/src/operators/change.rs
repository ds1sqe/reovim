//! Change operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use {
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{Edit, RegisterContent},
};

use super::{Operator, OperatorContext, OperatorError, Range, registers};

/// Change operator - cuts text and signals insert mode.
///
/// Behavior:
/// - Deletes text in the given range
/// - Stores deleted text in the unnamed register (or specified register)
/// - Signals that insert mode should be entered (via return value or event)
///
/// Note: The actual mode change is handled by the caller (runner/display driver).
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
    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        // Get the buffer via kernel's buffer manager
        let buffer_arc = ctx
            .kernel
            .buffers
            .get(ctx.buffer_id)
            .ok_or(OperatorError::BufferNotFound(ctx.buffer_id))?;

        let mut buffer = buffer_arc.write();

        // Capture cursor before changes for undo
        let cursor_before = buffer.position();

        // Get text before deleting
        let start = range.start;
        let end = range.end;

        // Build deleted text from lines
        let mut deleted_text = String::new();
        let lines = buffer.lines();
        let line_count = lines.len();

        // Track what was actually deleted for undo
        let delete_pos;

        if range.is_linewise {
            // Linewise change: delete content of lines from start.line to end.line (inclusive)
            // Unlike delete, change keeps ONE line for insertion (Vim behavior)
            // Clamp end.line to last valid line to handle counts exceeding buffer
            let clamped_end = end.line.min(line_count.saturating_sub(1));

            for line_idx in start.line..=clamped_end {
                if let Some(line) = lines.get(line_idx) {
                    deleted_text.push_str(line);
                    deleted_text.push('\n');
                }
            }

            // For linewise change (cc):
            // - Delete content of all affected lines
            // - Keep ONE empty line at start.line for insertion
            // This means: delete from (start.line, 0) to (clamped_end, end_of_content),
            // then if multiple lines, delete the extra newlines to leave just one line
            let delete_start = reovim_kernel::api::v1::Position::new(start.line, 0);
            let delete_end = if clamped_end + 1 < line_count {
                // Not the last line - delete content but preserve start.line's newline
                // So delete from (start.line, 0) to (clamped_end + 1, 0), then we're on next line
                // Actually for cc on middle line, we want to replace lines with one empty line
                // Delete everything from start to clamped_end (including their newlines except last)
                // Let's delete to end of clamped_end, then the newline stays
                if let Some(end_line_content) = lines.get(clamped_end) {
                    // Delete all lines but keep start.line as empty (with its newline)
                    // Delete from start.line to end of clamped_end content, plus all intermediate newlines
                    reovim_kernel::api::v1::Position::new(
                        clamped_end,
                        end_line_content.chars().count(),
                    )
                } else {
                    reovim_kernel::api::v1::Position::new(clamped_end, 0)
                }
            } else if let Some(last_line) = lines.get(clamped_end) {
                // End line is last line - delete to end of content (keep line structure)
                reovim_kernel::api::v1::Position::new(clamped_end, last_line.chars().count())
            } else {
                // Fallback
                reovim_kernel::api::v1::Position::new(clamped_end, 0)
            };

            delete_pos = delete_start;
            buffer.delete_range(delete_start, delete_end);
        } else if start.line == end.line {
            // Single line characterwise change
            if let Some(line) = lines.get(start.line) {
                let start_col = start.column.min(line.len());
                let end_col = end.column.min(line.len());
                if start_col < end_col {
                    deleted_text.push_str(&line[start_col..end_col]);
                }
            }
            delete_pos = start;
            buffer.delete_range(start, end);
        } else {
            // Multi-line characterwise change
            for line_idx in start.line..=end.line {
                if let Some(line) = lines.get(line_idx) {
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
            delete_pos = start;
            buffer.delete_range(start, end);
        }

        // Capture cursor after changes
        let cursor_after = buffer.position();

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

        registers::store_to_register(ctx.kernel, ctx.register, &content);
        registers::push_to_history(ctx.kernel, &content);

        // Note: Insert mode transition is handled by the caller
        // The runner/display driver should check operator id and enter insert mode

        Ok(())
    }

    fn is_linewise(&self) -> bool {
        false // Default; actual linewise-ness is determined by motion
    }

    fn is_text_modifying(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{CommandContext, CommandHandler, CommandResult},
        reovim_driver_session::{ClientId, Session, SessionRuntime, api::CommandExecutor},
        reovim_kernel::api::v1::{CommandId, KernelContext, ModeId, ModuleId},
    };

    #[allow(dead_code)]
    fn run_command<C: CommandHandler>(
        cmd: &C,
        ctx: &KernelContext,
        args: &CommandContext,
    ) -> CommandResult {
        struct StubExecutor;
        impl CommandExecutor for StubExecutor {
            fn execute(
                &self,
                _: &CommandId,
                _: &CommandContext,
                _: &KernelContext,
            ) -> Option<CommandResult> {
                Some(CommandResult::Success)
            }
        }
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode);
        let executor = StubExecutor;
        let mut runtime = SessionRuntime::new(&mut session, ctx, &executor);
        cmd.execute(&mut runtime, args)
    }

    #[test]
    fn test_change_operator_id() {
        let change = ChangeOperator;
        assert_eq!(change.id(), "change");
    }

    #[test]
    fn test_change_is_text_modifying() {
        let change = ChangeOperator;
        assert!(change.is_text_modifying());
    }

    #[test]
    fn test_change_is_not_linewise_by_default() {
        let change = ChangeOperator;
        assert!(!change.is_linewise());
    }
}
