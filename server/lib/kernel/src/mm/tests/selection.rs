use super::*;

#[test]
fn test_selection_mode_default() {
    let mode = SelectionMode::default();
    assert!(mode.is_character());
    assert!(!mode.is_block());
    assert!(!mode.is_line());
}

#[test]
fn test_selection_mode_checks() {
    assert!(SelectionMode::Character.is_character());
    assert!(SelectionMode::Block.is_block());
    assert!(SelectionMode::Line.is_line());
}

#[test]
fn test_selection_new() {
    let sel = Selection::new();
    assert!(!sel.is_active());
    assert_eq!(sel.anchor, Position::origin());
    assert_eq!(sel.mode, SelectionMode::Character);
}

#[test]
fn test_selection_start() {
    let mut sel = Selection::new();
    sel.start(Position::new(5, 10), SelectionMode::Block);

    assert!(sel.is_active());
    assert_eq!(sel.anchor, Position::new(5, 10));
    assert_eq!(sel.mode, SelectionMode::Block);
}

#[test]
fn test_selection_clear() {
    let mut sel = Selection::new();
    sel.start(Position::new(5, 10), SelectionMode::Character);
    assert!(sel.is_active());

    sel.clear();
    assert!(!sel.is_active());
    // Anchor is preserved
    assert_eq!(sel.anchor, Position::new(5, 10));
}

#[test]
fn test_selection_bounds_forward() {
    let mut sel = Selection::new();
    sel.start(Position::new(1, 5), SelectionMode::Character);

    let cursor = Position::new(3, 10);
    let (start, end) = sel.bounds(cursor).unwrap();

    assert_eq!(start, Position::new(1, 5));
    assert_eq!(end, Position::new(3, 10));
}

#[test]
fn test_selection_bounds_backward() {
    let mut sel = Selection::new();
    sel.start(Position::new(5, 15), SelectionMode::Character);

    let cursor = Position::new(2, 3);
    let (start, end) = sel.bounds(cursor).unwrap();

    // Should be normalized
    assert_eq!(start, Position::new(2, 3));
    assert_eq!(end, Position::new(5, 15));
}

#[test]
fn test_selection_bounds_when_inactive() {
    let sel = Selection::new();
    assert!(sel.bounds(Position::new(0, 0)).is_none());
}

#[test]
fn test_selection_block_bounds() {
    let mut sel = Selection::new();
    sel.start(Position::new(5, 20), SelectionMode::Block);

    let cursor = Position::new(2, 5);
    let (top_left, bottom_right) = sel.block_bounds(cursor).unwrap();

    assert_eq!(top_left, Position::new(2, 5));
    assert_eq!(bottom_right, Position::new(5, 20));
}

#[test]
fn test_selection_block_bounds_non_block_mode() {
    let mut sel = Selection::new();
    sel.start(Position::new(5, 20), SelectionMode::Character);

    let cursor = Position::new(2, 5);
    assert!(sel.block_bounds(cursor).is_none());
}

#[test]
fn test_selection_line_bounds() {
    let mut sel = Selection::new();
    sel.start(Position::new(10, 5), SelectionMode::Line);

    let cursor = Position::new(5, 0);
    let (start_line, end_line) = sel.line_bounds(cursor).unwrap();

    assert_eq!(start_line, 5);
    assert_eq!(end_line, 10);
}

#[test]
fn test_selection_mode_switching() {
    let mut sel = Selection::new();
    sel.start(Position::new(0, 0), SelectionMode::Character);
    assert!(sel.mode().is_character());

    sel.set_mode(SelectionMode::Block);
    assert!(sel.mode().is_block());

    sel.set_mode(SelectionMode::Line);
    assert!(sel.mode().is_line());
}

#[test]
fn test_selection_contains_character() {
    let mut sel = Selection::new();
    sel.start(Position::new(1, 0), SelectionMode::Character);
    let cursor = Position::new(3, 10);

    assert!(sel.contains(Position::new(2, 5), cursor));
    assert!(sel.contains(Position::new(1, 0), cursor)); // Start
    assert!(sel.contains(Position::new(3, 10), cursor)); // End
    assert!(!sel.contains(Position::new(0, 0), cursor)); // Before
    assert!(!sel.contains(Position::new(4, 0), cursor)); // After
}

#[test]
fn test_selection_contains_block() {
    let mut sel = Selection::new();
    sel.start(Position::new(1, 5), SelectionMode::Block);
    let cursor = Position::new(3, 15);

    assert!(sel.contains(Position::new(2, 10), cursor)); // Inside
    assert!(sel.contains(Position::new(1, 5), cursor)); // Corner
    assert!(!sel.contains(Position::new(2, 3), cursor)); // Left of block
    assert!(!sel.contains(Position::new(2, 20), cursor)); // Right of block
}

#[test]
fn test_selection_contains_line() {
    let mut sel = Selection::new();
    sel.start(Position::new(2, 0), SelectionMode::Line);
    let cursor = Position::new(5, 10);

    assert!(sel.contains(Position::new(3, 0), cursor)); // Inside
    assert!(sel.contains(Position::new(3, 100), cursor)); // Inside, any column
    assert!(!sel.contains(Position::new(1, 0), cursor)); // Before
    assert!(!sel.contains(Position::new(6, 0), cursor)); // After
}

#[test]
fn test_selection_line_count() {
    let mut sel = Selection::new();
    sel.start(Position::new(3, 0), SelectionMode::Character);

    assert_eq!(sel.line_count(Position::new(3, 10)), 1); // Same line
    assert_eq!(sel.line_count(Position::new(5, 0)), 3); // Lines 3, 4, 5
    assert_eq!(sel.line_count(Position::new(1, 0)), 3); // Lines 1, 2, 3 (backward)
}

#[test]
fn test_selection_line_count_inactive() {
    let sel = Selection::new();
    assert_eq!(sel.line_count(Position::new(5, 0)), 0);
}

#[test]
fn test_selection_convenience_methods() {
    let mut sel = Selection::new();

    sel.start_char(Position::new(0, 0));
    assert!(sel.mode().is_character());

    sel.start_line(Position::new(1, 0));
    assert!(sel.mode().is_line());

    sel.start_block(Position::new(2, 0));
    assert!(sel.mode().is_block());
}

// === Coverage: inactive selection returns None/false (L241, L264) ===

#[test]
fn test_line_bounds_inactive() {
    let sel = Selection::new(); // inactive by default
    assert_eq!(sel.line_bounds(Position::new(0, 0)), None);
}

#[test]
fn test_contains_inactive() {
    let sel = Selection::new();
    assert!(!sel.contains(Position::new(0, 0), Position::new(0, 5)));
}

// === Coverage: contains with all selection modes (L272, L282, L289) ===

#[test]
fn test_contains_character_mode() {
    let mut sel = Selection::new();
    sel.start_char(Position::new(0, 2));
    assert!(sel.contains(Position::new(0, 3), Position::new(0, 5)));
    assert!(!sel.contains(Position::new(0, 0), Position::new(0, 5)));
}

#[test]
fn test_contains_block_mode() {
    let mut sel = Selection::new();
    sel.start_block(Position::new(0, 2));
    assert!(sel.contains(Position::new(1, 3), Position::new(2, 5)));
    assert!(!sel.contains(Position::new(0, 0), Position::new(2, 5)));
}

#[test]
fn test_contains_line_mode() {
    let mut sel = Selection::new();
    sel.start_line(Position::new(1, 0));
    assert!(sel.contains(Position::new(2, 0), Position::new(3, 0)));
    assert!(!sel.contains(Position::new(0, 0), Position::new(3, 0)));
}
