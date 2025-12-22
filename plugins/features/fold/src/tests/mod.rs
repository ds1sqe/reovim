//! Tests for the fold plugin

use crate::state::{FoldKind, FoldRange, FoldState};

#[test]
fn test_fold_range() {
    let range = FoldRange::new(5, 10, FoldKind::Function, "fn foo()".to_string());

    assert!(range.is_foldable());
    assert_eq!(range.line_count(), 6);
    assert!(!range.contains_line(5)); // Start line not contained
    assert!(range.contains_line(6));
    assert!(range.contains_line(10));
    assert!(!range.contains_line(11));
}

#[test]
fn test_fold_state_toggle() {
    let mut state = FoldState::new();
    state.set_ranges(vec![
        FoldRange::new(0, 5, FoldKind::Function, "fn one()".to_string()),
        FoldRange::new(10, 15, FoldKind::Function, "fn two()".to_string()),
    ]);

    // Initially not collapsed
    assert!(!state.is_collapsed(0));
    assert!(!state.is_line_hidden(3));

    // Toggle to collapse
    assert!(state.toggle(0));
    assert!(state.is_collapsed(0));
    assert!(state.is_line_hidden(3));

    // Toggle to expand
    assert!(state.toggle(0));
    assert!(!state.is_collapsed(0));
    assert!(!state.is_line_hidden(3));
}

#[test]
fn test_fold_marker() {
    let mut state = FoldState::new();
    state.set_ranges(vec![FoldRange::new(
        0,
        5,
        FoldKind::Function,
        "fn foo()".to_string(),
    )]);

    // Not collapsed - no marker
    assert!(state.get_fold_marker(0).is_none());

    // Collapse
    state.close(0);

    // Should have marker
    let marker = state.get_fold_marker(0);
    assert!(marker.is_some());
    let (count, preview) = marker.unwrap();
    assert_eq!(count, 5); // 5 hidden lines (1-5)
    assert_eq!(preview, "fn foo()");
}

#[test]
fn test_open_close_all() {
    let mut state = FoldState::new();
    state.set_ranges(vec![
        FoldRange::new(0, 5, FoldKind::Function, "fn one()".to_string()),
        FoldRange::new(10, 15, FoldKind::Function, "fn two()".to_string()),
    ]);

    state.close_all();
    assert!(state.is_collapsed(0));
    assert!(state.is_collapsed(10));

    state.open_all();
    assert!(!state.is_collapsed(0));
    assert!(!state.is_collapsed(10));
}

#[test]
fn test_display_line_mapping() {
    let mut state = FoldState::new();
    state.set_ranges(vec![FoldRange::new(
        2,
        5,
        FoldKind::Function,
        "fn test()".to_string(),
    )]);

    // Close the fold
    state.close(2);

    // Lines 0, 1, 2 are visible (2 is fold header)
    // Lines 3, 4, 5 are hidden
    // Line 6+ visible again

    assert!(!state.is_line_hidden(0));
    assert!(!state.is_line_hidden(1));
    assert!(!state.is_line_hidden(2)); // Fold header is visible
    assert!(state.is_line_hidden(3));
    assert!(state.is_line_hidden(4));
    assert!(state.is_line_hidden(5));
    assert!(!state.is_line_hidden(6));
}

#[test]
fn test_visible_line_count() {
    let mut state = FoldState::new();
    state.set_ranges(vec![FoldRange::new(
        2,
        5,
        FoldKind::Function,
        "fn test()".to_string(),
    )]);

    // 10 total lines, no folds collapsed
    assert_eq!(state.visible_line_count(10), 10);

    // Close fold (hides lines 3, 4, 5)
    state.close(2);
    assert_eq!(state.visible_line_count(10), 7);
}
