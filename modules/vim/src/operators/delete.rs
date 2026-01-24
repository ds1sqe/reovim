//! Delete operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use reovim_kernel::api::v1::RegisterContent;

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
        let mut deleted_text = String::new();
        let lines = buffer.lines();

        if range.is_linewise {
            // Linewise deletion: delete entire lines from start.line to end.line (inclusive)
            // Ignore column values - always delete full lines
            let line_count = lines.len();

            // Clamp end.line to last valid line to handle counts exceeding buffer
            let clamped_end = end.line.min(line_count.saturating_sub(1));

            for line_idx in start.line..=clamped_end {
                if let Some(line) = lines.get(line_idx) {
                    deleted_text.push_str(line);
                    deleted_text.push('\n');
                }
            }

            // For linewise, adjust the actual deletion range to cover full lines
            let delete_start = reovim_kernel::api::v1::Position::new(start.line, 0);

            // Calculate end position:
            // - If next line exists, point to start of next line (deletes through clamped_end's newline)
            // - If clamped_end is last line, point to end of its content (no trailing newline)
            let delete_end = if clamped_end + 1 < line_count {
                // Next line exists - point to its start
                reovim_kernel::api::v1::Position::new(clamped_end + 1, 0)
            } else if let Some(last_line) = lines.get(clamped_end) {
                // End line is last line - point to end of its content
                reovim_kernel::api::v1::Position::new(clamped_end, last_line.chars().count())
            } else {
                // Fallback (shouldn't happen in normal operation)
                reovim_kernel::api::v1::Position::new(clamped_end, 0)
            };

            // Store in register as linewise (handles +/* via ClipboardProvider)
            let content = RegisterContent::linewise(deleted_text);
            registers::store_to_register(ctx.kernel, ctx.register, &content);
            registers::push_to_history(ctx.kernel, &content);

            // Delete entire lines
            buffer.delete_range(delete_start, delete_end);
        } else {
            // Characterwise deletion
            if start.line == end.line {
                // Single line deletion
                if let Some(line) = lines.get(start.line) {
                    let start_col = start.column.min(line.len());
                    let end_col = end.column.min(line.len());
                    if start_col < end_col {
                        deleted_text.push_str(&line[start_col..end_col]);
                    }
                }
            } else {
                // Multi-line deletion
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
            }

            // Store in register as characterwise (handles +/* via ClipboardProvider)
            let content = RegisterContent::characterwise(deleted_text);
            registers::store_to_register(ctx.kernel, ctx.register, &content);
            registers::push_to_history(ctx.kernel, &content);

            // Delete the text from buffer
            buffer.delete_range(start, end);
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
                _: &mut KernelContext,
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
    fn test_delete_operator_id() {
        let delete = DeleteOperator;
        assert_eq!(delete.id(), "delete");
    }

    #[test]
    fn test_delete_is_text_modifying() {
        let delete = DeleteOperator;
        assert!(delete.is_text_modifying());
    }

    #[test]
    fn test_delete_is_not_linewise_by_default() {
        let delete = DeleteOperator;
        assert!(!delete.is_linewise());
    }
}
