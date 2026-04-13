//! Yank operator.
//!
//! Reference: lib/core/src/command/builtin/operator.rs (concept-extraction, not migration)

use reovim_domain_text::RegisterContent;

use super::{Operator, OperatorContext, OperatorError, Range, char_col_to_byte, registers};

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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        // Get the buffer via kernel's buffer manager
        let buffer_arc = ctx.text_buffer()?;

        let buffer = buffer_arc.read();

        // Build yanked text from lines
        let start = range.start;
        let end = range.end;
        let mut yanked_text = String::new();

        if range.is_linewise {
            // Linewise yank: copy entire lines from start.line to end.line (inclusive)
            // Ignore column values - always yank full lines
            // Clamp end.line to last valid line to handle counts exceeding buffer
            let line_count = buffer.line_count();
            let clamped_end = end.line.min(line_count.saturating_sub(1));

            for line_idx in start.line..=clamped_end {
                if let Some(line) = buffer.line(line_idx) {
                    yanked_text.push_str(&line);
                    yanked_text.push('\n');
                }
            }
        } else if start.line == end.line {
            // Single line characterwise yank
            if let Some(line) = buffer.line(start.line) {
                let char_len = line.chars().count();
                let start_col = start.column.min(char_len);
                let end_col = end.column.min(char_len);
                if start_col < end_col {
                    let start_byte = char_col_to_byte(&line, start_col);
                    let end_byte = char_col_to_byte(&line, end_col);
                    yanked_text.push_str(&line[start_byte..end_byte]);
                }
            }
        } else {
            // Multi-line characterwise yank
            for line_idx in start.line..=end.line {
                if let Some(line) = buffer.line(line_idx) {
                    if line_idx == start.line {
                        let char_len = line.chars().count();
                        let start_col = start.column.min(char_len);
                        let start_byte = char_col_to_byte(&line, start_col);
                        yanked_text.push_str(&line[start_byte..]);
                        yanked_text.push('\n');
                    } else if line_idx == end.line {
                        let char_len = line.chars().count();
                        let end_col = end.column.min(char_len);
                        let end_byte = char_col_to_byte(&line, end_col);
                        yanked_text.push_str(&line[..end_byte]);
                    } else {
                        yanked_text.push_str(&line);
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
        // Uses ClipboardProvider for +/* registers, per-client RegisterBank for others (#515)
        registers::store_and_sync(ctx.kernel, ctx.registers, ctx.register, &content);

        // Push to per-client history for numbered registers (0-9) (#515)
        // This happens on every yank regardless of target register
        registers::push_to_history(ctx.clipboard_history, &content);

        Ok(())
    }

    fn is_linewise(&self) -> bool {
        false // Default; actual linewise-ness is determined by motion
    }

    fn is_content_modifying(&self) -> bool {
        false // Yank does NOT modify text
    }
}
