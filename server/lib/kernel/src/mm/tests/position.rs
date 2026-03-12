use super::*;

// === Position Tests ===

#[test]
fn position_new() {
    let pos = Position::new(3, 7);
    assert_eq!(pos.line, 3);
    assert_eq!(pos.column, 7);
}

#[test]
fn position_line_start() {
    let pos = Position::line_start(5);
    assert_eq!(pos.line, 5);
    assert_eq!(pos.column, 0);
}

#[test]
fn position_line_start_zero() {
    let pos = Position::line_start(0);
    assert_eq!(pos, Position::origin());
}

#[test]
fn position_default() {
    let pos = Position::default();
    assert_eq!(pos.line, 0);
    assert_eq!(pos.column, 0);
    assert_eq!(pos, Position::origin());
}

#[test]
fn position_eq() {
    assert_eq!(Position::new(1, 2), Position::new(1, 2));
    assert_ne!(Position::new(1, 2), Position::new(1, 3));
    assert_ne!(Position::new(1, 2), Position::new(2, 2));
}

#[test]
fn position_ord_same_line() {
    assert!(Position::new(0, 0) < Position::new(0, 1));
    assert!(Position::new(0, 5) < Position::new(0, 10));
    assert!(Position::new(0, 10) > Position::new(0, 5));
}

#[test]
fn position_ord_different_lines() {
    assert!(Position::new(0, 100) < Position::new(1, 0));
    assert!(Position::new(1, 0) < Position::new(2, 0));
    assert!(Position::new(3, 0) > Position::new(2, 99));
}

#[test]
fn position_ord_equal() {
    let a = Position::new(5, 5);
    let b = Position::new(5, 5);
    assert!(a <= b);
    assert!(a >= b);
    assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
}

#[test]
fn position_partial_ord_consistent_with_ord() {
    let a = Position::new(1, 5);
    let b = Position::new(2, 3);
    assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
    assert_eq!(a.partial_cmp(&b), Some(a.cmp(&b)));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn position_display() {
    let pos = Position::new(0, 0);
    assert_eq!(format!("{pos}"), "1:1");

    let pos = Position::new(9, 19);
    assert_eq!(format!("{pos}"), "10:20");
}

#[test]
fn position_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Position::new(0, 0));
    set.insert(Position::new(1, 0));
    set.insert(Position::new(0, 0)); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn position_clone_and_copy() {
    let a = Position::new(3, 4);
    let b = a; // copy
    assert_eq!(a, b);
}

// === Cursor Tests ===

#[test]
fn cursor_new() {
    let cursor = Cursor::new(Position::new(5, 10));
    assert_eq!(cursor.position, Position::new(5, 10));
    assert!(cursor.anchor.is_none());
    assert!(cursor.preferred_column.is_none());
}

#[test]
fn cursor_origin() {
    let cursor = Cursor::origin();
    assert_eq!(cursor.position, Position::origin());
    assert!(!cursor.has_selection());
}

#[test]
fn cursor_default() {
    let cursor = Cursor::default();
    assert_eq!(cursor.position, Position::origin());
    assert!(cursor.anchor.is_none());
    assert!(cursor.preferred_column.is_none());
}

#[test]
fn cursor_start_and_clear_selection() {
    let mut cursor = Cursor::new(Position::new(2, 5));
    assert!(!cursor.has_selection());
    assert!(cursor.selection_bounds().is_none());

    cursor.start_selection();
    assert!(cursor.has_selection());
    assert_eq!(cursor.anchor, Some(Position::new(2, 5)));

    cursor.clear_selection();
    assert!(!cursor.has_selection());
    assert!(cursor.anchor.is_none());
}

#[test]
fn cursor_selection_bounds_forward() {
    let mut cursor = Cursor::new(Position::new(0, 0));
    cursor.start_selection();
    cursor.position = Position::new(2, 10);

    let (start, end) = cursor.selection_bounds().unwrap();
    assert_eq!(start, Position::new(0, 0));
    assert_eq!(end, Position::new(2, 10));
}

#[test]
fn cursor_selection_bounds_backward() {
    let mut cursor = Cursor::new(Position::new(5, 15));
    cursor.start_selection();
    cursor.position = Position::new(1, 3);

    let (start, end) = cursor.selection_bounds().unwrap();
    assert_eq!(start, Position::new(1, 3));
    assert_eq!(end, Position::new(5, 15));
}

#[test]
fn cursor_selection_bounds_same_position() {
    let mut cursor = Cursor::new(Position::new(3, 3));
    cursor.start_selection();
    // cursor.position stays at anchor

    let (start, end) = cursor.selection_bounds().unwrap();
    assert_eq!(start, Position::new(3, 3));
    assert_eq!(end, Position::new(3, 3));
}

#[test]
fn cursor_selection_bounds_no_selection() {
    let cursor = Cursor::new(Position::new(0, 0));
    assert!(cursor.selection_bounds().is_none());
}

#[test]
fn cursor_preferred_column_lifecycle() {
    let mut cursor = Cursor::new(Position::new(0, 15));
    assert!(cursor.preferred_column.is_none());
    assert_eq!(cursor.effective_column(), 15);

    cursor.update_preferred_column();
    assert_eq!(cursor.preferred_column, Some(15));
    assert_eq!(cursor.effective_column(), 15);

    // Move to shorter line
    cursor.position = Position::new(1, 5);
    // preferred_column still 15
    assert_eq!(cursor.effective_column(), 15);

    cursor.clear_preferred_column();
    assert!(cursor.preferred_column.is_none());
    assert_eq!(cursor.effective_column(), 5);
}

#[test]
fn cursor_effective_column_without_preferred() {
    let cursor = Cursor::new(Position::new(0, 42));
    assert_eq!(cursor.effective_column(), 42);
}

#[test]
fn cursor_eq() {
    let a = Cursor::new(Position::new(1, 2));
    let b = Cursor::new(Position::new(1, 2));
    let c = Cursor::new(Position::new(1, 3));
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn cursor_clone() {
    let mut cursor = Cursor::new(Position::new(1, 2));
    cursor.start_selection();
    cursor.update_preferred_column();

    let cloned = cursor;
    assert_eq!(cursor.position, cloned.position);
    assert_eq!(cursor.anchor, cloned.anchor);
    assert_eq!(cursor.preferred_column, cloned.preferred_column);
}
