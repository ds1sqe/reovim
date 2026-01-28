//! Yank operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use reovim_kernel::api::v1::RegisterContent;

use super::{Operator, OperatorContext, OperatorError, Range, registers};

/// Yank operator - copies text to register.
///
/// Behavior:
/// - Copies text in the given range to register
/// - Does NOT modify the buffer
/// - Linewise if the motion was linewise
///
/// # Example
///
/// ```ignore
/// let yank = YankOperator;
/// yank.execute(&mut ctx, range)?;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct YankOperator;

impl Operator for YankOperator {
    fn id(&self) -> &'static str {
        "yank"
    }

    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        // Get the buffer via kernel's buffer manager
        let buffer_arc = ctx
            .kernel
            .buffers
            .get(ctx.buffer_id)
            .ok_or(OperatorError::BufferNotFound(ctx.buffer_id))?;

        let buffer = buffer_arc.read();

        // Build yanked text from lines
        let start = range.start;
        let end = range.end;
        let mut yanked_text = String::new();
        let lines = buffer.lines();

        if range.is_linewise {
            // Linewise yank: copy entire lines from start.line to end.line (inclusive)
            // Ignore column values - always yank full lines
            // Clamp end.line to last valid line to handle counts exceeding buffer
            let line_count = lines.len();
            let clamped_end = end.line.min(line_count.saturating_sub(1));

            for line_idx in start.line..=clamped_end {
                if let Some(line) = lines.get(line_idx) {
                    yanked_text.push_str(line);
                    yanked_text.push('\n');
                }
            }
        } else if start.line == end.line {
            // Single line characterwise yank
            if let Some(line) = lines.get(start.line) {
                let start_col = start.column.min(line.len());
                let end_col = end.column.min(line.len());
                if start_col < end_col {
                    yanked_text.push_str(&line[start_col..end_col]);
                }
            }
        } else {
            // Multi-line characterwise yank
            for line_idx in start.line..=end.line {
                if let Some(line) = lines.get(line_idx) {
                    if line_idx == start.line {
                        let start_col = start.column.min(line.len());
                        yanked_text.push_str(&line[start_col..]);
                        yanked_text.push('\n');
                    } else if line_idx == end.line {
                        let end_col = end.column.min(line.len());
                        yanked_text.push_str(&line[..end_col]);
                    } else {
                        yanked_text.push_str(line);
                        yanked_text.push('\n');
                    }
                }
            }
        }

        // Release the buffer lock before register operations
        drop(buffer);

        // Store in register - linewise if the range was linewise
        let content = if range.is_linewise {
            RegisterContent::linewise(yanked_text)
        } else {
            RegisterContent::characterwise(yanked_text)
        };

        // Store to the specified register (handles +, *, a-z, etc.)
        // Uses ClipboardProvider for +/* registers, RegisterBank for others
        registers::store_to_register(ctx.kernel, ctx.register, &content);

        // Push to history for numbered registers (0-9)
        // This happens on every yank regardless of target register
        registers::push_to_history(ctx.kernel, &content);

        Ok(())
    }

    fn is_linewise(&self) -> bool {
        false // Default; actual linewise-ness is determined by motion
    }

    fn is_text_modifying(&self) -> bool {
        false // Yank does NOT modify text
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
    fn test_yank_operator_id() {
        let yank = YankOperator;
        assert_eq!(yank.id(), "yank");
    }

    #[test]
    fn test_yank_is_not_text_modifying() {
        let yank = YankOperator;
        assert!(!yank.is_text_modifying());
    }

    #[test]
    fn test_yank_is_not_linewise_by_default() {
        let yank = YankOperator;
        assert!(!yank.is_linewise());
    }
}
