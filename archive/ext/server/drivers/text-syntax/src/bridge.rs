use crate::SyntaxEdit;

/// Compute the end position (row, col) after inserting text starting at (`start_row`, `start_col`).
///
/// Handles multi-line text by counting newlines and tracking the final line's column.
#[must_use]
pub fn compute_end_position(start_row: u32, start_col: u32, text: &str) -> (u32, u32) {
    let mut row = start_row;
    let mut col = start_col;
    for ch in text.chars() {
        if ch == '\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (row, col)
}

/// Convert a `TextBufferModified` event into a `SyntaxEdit` for incremental
/// tree-sitter parsing (#740 Plan 09).
///
/// Bridges the text-domain event crate (`reovim-domain-text-events`) and the
/// syntax driver crate (`reovim-driver-syntax`). Uses `TextEdit` position and
/// byte offsets from the event directly.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // TextPosition uses usize; SyntaxEdit uses u32
pub fn text_event_to_syntax_edit(
    event: &reovim_domain_text_events::TextBufferModified,
) -> SyntaxEdit {
    use reovim_domain_text_events::TextEdit;

    match &event.edit {
        TextEdit::Insert { position, text } => {
            let (new_end_row, new_end_col) =
                compute_end_position(position.line as u32, position.column as u32, text);
            SyntaxEdit::insert(
                event.start_byte,
                position.line as u32,
                position.column as u32,
                event.new_end_byte,
                new_end_row,
                new_end_col,
            )
        }
        TextEdit::Delete { position, text } => {
            let (old_end_row, old_end_col) =
                compute_end_position(position.line as u32, position.column as u32, text);
            SyntaxEdit::delete(
                event.start_byte,
                position.line as u32,
                position.column as u32,
                event.old_end_byte,
                old_end_row,
                old_end_col,
            )
        }
    }
}
