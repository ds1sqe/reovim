//! Case transformation operators.
//!
//! Implements three case operators that work with motions and text objects:
//! - `LowercaseOperator` (gu) - converts text to lowercase
//! - `UppercaseOperator` (gU) - converts text to uppercase
//! - `ToggleCaseOperator` (g~) - toggles case of each character
//!
//! Unlike delete/yank/change, case operators do not modify registers.

use {
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_types_text::{Edit, Position},
};

use super::{Operator, OperatorContext, OperatorError, Range, char_col_to_byte};

// =============================================================================
// Case Transformation Helper
// =============================================================================

/// Apply a case transformation to text in a range.
///
/// Reads text from the buffer, applies the transformation function,
/// and replaces the original text if it changed.
#[allow(clippy::too_many_lines)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn apply_case_transform(
    ctx: &mut OperatorContext<'_>,
    range: Range,
    transform: fn(&str) -> String,
) -> Result<(), OperatorError> {
    let buffer_arc = ctx.text_buffer()?;

    let mut buffer = buffer_arc.write();

    let start = range.start;
    let end = range.end;

    // Build original text from the range
    let mut original = String::new();

    if range.is_linewise {
        let line_count = buffer.line_count();
        let clamped_end = end.line.min(line_count.saturating_sub(1));

        for line_idx in start.line..=clamped_end {
            if line_idx > start.line {
                original.push('\n');
            }
            if let Some(line) = buffer.line(line_idx) {
                original.push_str(&line);
            }
        }
    } else if start.line == end.line {
        // Single line
        if let Some(line) = buffer.line(start.line) {
            let char_len = line.chars().count();
            let start_col = start.column.min(char_len);
            let end_col = end.column.min(char_len);
            if start_col < end_col {
                let start_byte = char_col_to_byte(&line, start_col);
                let end_byte = char_col_to_byte(&line, end_col);
                original.push_str(&line[start_byte..end_byte]);
            }
        }
    } else {
        // Multi-line characterwise
        for line_idx in start.line..=end.line {
            if let Some(line) = buffer.line(line_idx) {
                if line_idx == start.line {
                    let char_len = line.chars().count();
                    let start_col = start.column.min(char_len);
                    let start_byte = char_col_to_byte(&line, start_col);
                    original.push_str(&line[start_byte..]);
                    original.push('\n');
                } else if line_idx == end.line {
                    let char_len = line.chars().count();
                    let end_col = end.column.min(char_len);
                    let end_byte = char_col_to_byte(&line, end_col);
                    original.push_str(&line[..end_byte]);
                } else {
                    original.push_str(&line);
                    original.push('\n');
                }
            }
        }
    }

    let transformed = transform(&original);

    // Only modify buffer if something actually changed
    if transformed == original {
        // Even if nothing changed, set cursor_after for consistency
        ctx.cursor_after = Some(start);
        return Ok(());
    }

    let cursor_before = ctx.cursor_position;

    if range.is_linewise {
        let line_count = buffer.line_count();
        let clamped_end = end.line.min(line_count.saturating_sub(1));

        // Delete from start of first line to end of last line
        let delete_start = Position::new(start.line, 0);
        let last_line_char_len = buffer.line(clamped_end).map_or(0, |l| l.chars().count());
        let delete_end = Position::new(clamped_end, last_line_char_len);

        buffer.delete_range(delete_start, delete_end);
        buffer.insert_at(delete_start, &transformed);

        ctx.cursor_after = Some(Position::new(start.line, 0));
    } else {
        buffer.delete_range(start, end);
        buffer.insert_at(start, &transformed);

        ctx.cursor_after = Some(start);
    }

    // Record undo
    if let Some(undo_registry) = ctx.kernel.services.get::<UndoProviderRegistry>()
        && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
    {
        // Record as delete-then-insert pair
        let delete_edit = Edit::Delete {
            position: if range.is_linewise {
                Position::new(start.line, 0)
            } else {
                start
            },
            text: original,
        };
        let insert_edit = Edit::Insert {
            position: if range.is_linewise {
                Position::new(start.line, 0)
            } else {
                start
            },
            text: transformed,
        };
        let cursor_after = ctx.cursor_after.unwrap_or(start);
        undo_provider.record(
            ctx.buffer_id,
            vec![delete_edit, insert_edit],
            cursor_before,
            cursor_after,
        );
    }

    drop(buffer);
    Ok(())
}

// =============================================================================
// Lowercase Operator (gu)
// =============================================================================

/// Lowercase operator - converts text to lowercase.
#[derive(Debug, Clone, Copy)]
pub struct LowercaseOperator;

impl Operator for LowercaseOperator {
    fn id(&self) -> &'static str {
        "lowercase"
    }

    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        apply_case_transform(ctx, range, str::to_lowercase)
    }

    fn is_text_modifying(&self) -> bool {
        true
    }
}

// =============================================================================
// Uppercase Operator (gU)
// =============================================================================

/// Uppercase operator - converts text to uppercase.
#[derive(Debug, Clone, Copy)]
pub struct UppercaseOperator;

impl Operator for UppercaseOperator {
    fn id(&self) -> &'static str {
        "uppercase"
    }

    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        apply_case_transform(ctx, range, str::to_uppercase)
    }

    fn is_text_modifying(&self) -> bool {
        true
    }
}

// =============================================================================
// Toggle Case Operator (g~)
// =============================================================================

/// Toggle case operator - swaps uppercase/lowercase.
#[derive(Debug, Clone, Copy)]
pub struct ToggleCaseOperator;

impl Operator for ToggleCaseOperator {
    fn id(&self) -> &'static str {
        "toggle-case"
    }

    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError> {
        apply_case_transform(ctx, range, toggle_case)
    }

    fn is_text_modifying(&self) -> bool {
        true
    }
}

/// Toggle the case of each character in a string.
fn toggle_case(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_uppercase() {
                c.to_lowercase().next().unwrap_or(c)
            } else if c.is_lowercase() {
                c.to_uppercase().next().unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}
