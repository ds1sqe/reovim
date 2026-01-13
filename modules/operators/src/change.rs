//! Change operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use reovim_kernel::api::v1::{Operator, OperatorContext, OperatorError, Range, RegisterContent};

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

        // Delete the text from buffer
        buffer.delete_range(start, end);
        drop(buffer);

        // Store in register
        // For now, assume characterwise; linewise detection would come from motion context
        let content = RegisterContent::characterwise(deleted_text);
        ctx.kernel.registers.write().set(content);

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
    use super::*;

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
