use super::*;

#[test]
fn test_syntax_edit_new() {
    let edit = SyntaxEdit::new(10, 20, 15, 2, 5, 3, 0, 2, 10);

    assert_eq!(edit.start_byte, 10);
    assert_eq!(edit.old_end_byte, 20);
    assert_eq!(edit.new_end_byte, 15);
    assert_eq!(edit.start_row, 2);
    assert_eq!(edit.start_col, 5);
    assert_eq!(edit.old_end_row, 3);
    assert_eq!(edit.old_end_col, 0);
    assert_eq!(edit.new_end_row, 2);
    assert_eq!(edit.new_end_col, 10);
}

#[test]
fn test_syntax_edit_insert() {
    let edit = SyntaxEdit::insert(10, 2, 5, 15, 2, 10);

    assert_eq!(edit.start_byte, 10);
    assert_eq!(edit.old_end_byte, 10); // No old content
    assert_eq!(edit.new_end_byte, 15);
    assert_eq!(edit.start_row, 2);
    assert_eq!(edit.start_col, 5);
    assert_eq!(edit.old_end_row, 2);
    assert_eq!(edit.old_end_col, 5);
    assert_eq!(edit.new_end_row, 2);
    assert_eq!(edit.new_end_col, 10);

    assert!(edit.is_insert());
    assert!(!edit.is_delete());
    assert!(!edit.is_replace());
}

#[test]
fn test_syntax_edit_delete() {
    let edit = SyntaxEdit::delete(10, 2, 5, 20, 3, 0);

    assert_eq!(edit.start_byte, 10);
    assert_eq!(edit.old_end_byte, 20);
    assert_eq!(edit.new_end_byte, 10); // No new content
    assert_eq!(edit.start_row, 2);
    assert_eq!(edit.start_col, 5);
    assert_eq!(edit.old_end_row, 3);
    assert_eq!(edit.old_end_col, 0);
    assert_eq!(edit.new_end_row, 2);
    assert_eq!(edit.new_end_col, 5);

    assert!(edit.is_delete());
    assert!(!edit.is_insert());
    assert!(!edit.is_replace());
}

#[test]
fn test_syntax_edit_replace() {
    let edit = SyntaxEdit::new(10, 20, 15, 2, 5, 3, 0, 2, 10);

    assert!(edit.is_replace());
    assert!(!edit.is_insert());
    assert!(!edit.is_delete());
}

#[test]
fn test_syntax_edit_affected_range() {
    // Insertion: range is start..new_end
    let insert = SyntaxEdit::insert(10, 0, 0, 15, 0, 5);
    assert_eq!(insert.affected_range(), 10..15);

    // Deletion: range is start..old_end
    let delete = SyntaxEdit::delete(10, 0, 0, 20, 1, 0);
    assert_eq!(delete.affected_range(), 10..20);

    // Replace with shrink: range is start..old_end
    let shrink = SyntaxEdit::new(10, 30, 20, 0, 0, 0, 0, 0, 0);
    assert_eq!(shrink.affected_range(), 10..30);

    // Replace with growth: range is start..new_end
    let grow = SyntaxEdit::new(10, 20, 30, 0, 0, 0, 0, 0, 0);
    assert_eq!(grow.affected_range(), 10..30);
}

#[test]
fn test_syntax_edit_bytes_removed() {
    let edit = SyntaxEdit::new(10, 25, 15, 0, 0, 0, 0, 0, 0);
    assert_eq!(edit.bytes_removed(), 15); // 25 - 10

    let insert = SyntaxEdit::insert(10, 0, 0, 20, 0, 0);
    assert_eq!(insert.bytes_removed(), 0);
}

#[test]
fn test_syntax_edit_bytes_inserted() {
    let edit = SyntaxEdit::new(10, 25, 18, 0, 0, 0, 0, 0, 0);
    assert_eq!(edit.bytes_inserted(), 8); // 18 - 10

    let delete = SyntaxEdit::delete(10, 0, 0, 20, 0, 0);
    assert_eq!(delete.bytes_inserted(), 0);
}

#[test]
fn test_syntax_edit_byte_delta() {
    // Shrink: 10 bytes removed, 5 inserted = -5 delta
    let shrink = SyntaxEdit::new(10, 20, 15, 0, 0, 0, 0, 0, 0);
    assert_eq!(shrink.byte_delta(), -5);

    // Grow: 5 bytes removed, 10 inserted = +5 delta
    let grow = SyntaxEdit::new(10, 15, 20, 0, 0, 0, 0, 0, 0);
    assert_eq!(grow.byte_delta(), 5);

    // Pure insert
    let insert = SyntaxEdit::insert(10, 0, 0, 15, 0, 0);
    assert_eq!(insert.byte_delta(), 5);

    // Pure delete
    let delete = SyntaxEdit::delete(10, 0, 0, 20, 0, 0);
    assert_eq!(delete.byte_delta(), -10);
}
