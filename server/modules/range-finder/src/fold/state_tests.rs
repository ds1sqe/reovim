use {super::*, reovim_driver_syntax::FoldKind};

fn make_ranges() -> Vec<FoldRange> {
    vec![
        FoldRange::new(2, 10, FoldKind::Class, "impl Foo {"),
        FoldRange::new(3, 6, FoldKind::Function, "fn bar() {"),
        FoldRange::new(7, 9, FoldKind::Function, "fn baz() {"),
    ]
}

// ========================================================================
// FoldState tests
// ========================================================================

#[test]
fn test_fold_state_default_empty() {
    let state = FoldState::new();
    assert!(state.ranges().is_empty());
    assert!(!state.has_collapsed());
}

#[test]
fn test_set_ranges_clears_collapsed() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());
    state.close(0);
    assert!(state.has_collapsed());

    // Setting new ranges clears collapsed
    state.set_ranges(make_ranges());
    assert!(!state.has_collapsed());
}

#[test]
fn test_set_ranges_sorts_by_start_line() {
    let mut state = FoldState::new();
    // Give ranges out of order
    let ranges = vec![
        FoldRange::new(7, 9, FoldKind::Function, "fn baz() {"),
        FoldRange::new(2, 10, FoldKind::Class, "impl Foo {"),
        FoldRange::new(3, 6, FoldKind::Function, "fn bar() {"),
    ];
    state.set_ranges(ranges);

    // Should be sorted by start_line
    assert_eq!(state.ranges()[0].start_line, 2);
    assert_eq!(state.ranges()[1].start_line, 3);
    assert_eq!(state.ranges()[2].start_line, 7);
}

#[test]
fn test_fold_at_line_exact_start() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Line 3 is the start of fn bar (index 1)
    let idx = state.fold_at_line(3).unwrap();
    assert_eq!(idx, 1);
}

#[test]
fn test_fold_at_line_inside() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Line 5 is inside fn bar (index 1), also inside impl Foo (index 0)
    // Should return innermost (fn bar, smaller span)
    let idx = state.fold_at_line(5).unwrap();
    assert_eq!(idx, 1);
}

#[test]
fn test_fold_at_line_outside() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Line 0 is outside all folds
    assert!(state.fold_at_line(0).is_none());
    // Line 11 is outside all folds
    assert!(state.fold_at_line(11).is_none());
}

#[test]
fn test_fold_at_line_innermost_nested() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Line 8 is inside fn baz (index 2) and impl Foo (index 0)
    // fn baz spans 7-9 (3 lines), impl Foo spans 2-10 (9 lines)
    let idx = state.fold_at_line(8).unwrap();
    assert_eq!(idx, 2); // innermost
}

#[test]
fn test_fold_at_line_wider_after_narrower() {
    let mut state = FoldState::new();
    // Two folds with the same start_line: narrow first, wide second.
    // When sorted by start_line (stable sort), narrow comes first.
    let ranges = vec![
        FoldRange::new(2, 4, FoldKind::Function, "fn narrow() {"),
        FoldRange::new(2, 10, FoldKind::Class, "impl Wide {"),
    ];
    state.set_ranges(ranges);

    // Line 3 is in both folds. Narrow checked first (span=2, best_span=2),
    // then wide (span=8, 8 < 2 is false → branch not taken).
    let idx = state.fold_at_line(3).unwrap();
    assert_eq!(idx, 0); // narrow fold wins
}

#[test]
fn test_get_fold_marker_mismatched_collapsed() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Collapse both fn bar (index 1, start=3) and fn baz (index 2, start=7)
    state.close(1);
    state.close(2);

    // Query line 3: matches fold at index 1, but index 2 has start_line=7 ≠ 3 (false branch)
    let (hidden, preview) = state.get_fold_marker(3).unwrap();
    assert_eq!(hidden, 3);
    assert_eq!(preview, "fn bar() {");

    // Query line 7: matches fold at index 2, but index 1 has start_line=3 ≠ 7 (false branch)
    let (hidden, preview) = state.get_fold_marker(7).unwrap();
    assert_eq!(hidden, 2);
    assert_eq!(preview, "fn baz() {");
}

#[test]
fn test_toggle_collapses() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    assert!(!state.is_collapsed(0));
    state.toggle(0);
    assert!(state.is_collapsed(0));
}

#[test]
fn test_toggle_expands() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    state.close(0);
    assert!(state.is_collapsed(0));
    state.toggle(0);
    assert!(!state.is_collapsed(0));
}

#[test]
fn test_toggle_out_of_bounds() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Should not panic
    state.toggle(999);
    assert!(!state.has_collapsed());
}

#[test]
fn test_open_already_open() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Already open, open is no-op
    state.open(0);
    assert!(!state.is_collapsed(0));
}

#[test]
fn test_open_collapsed() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    state.close(0);
    assert!(state.is_collapsed(0));
    state.open(0);
    assert!(!state.is_collapsed(0));
}

#[test]
fn test_close_already_closed() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    state.close(0);
    assert!(state.is_collapsed(0));
    // Close again is no-op
    state.close(0);
    assert!(state.is_collapsed(0));
}

#[test]
fn test_close_open() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    assert!(!state.is_collapsed(0));
    state.close(0);
    assert!(state.is_collapsed(0));
}

#[test]
fn test_close_out_of_bounds() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Should not panic or add invalid index
    state.close(999);
    assert!(!state.has_collapsed());
}

#[test]
fn test_open_all() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    state.close_all();
    assert!(state.has_collapsed());
    state.open_all();
    assert!(!state.has_collapsed());
}

#[test]
fn test_close_all() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    state.close_all();
    assert!(state.is_collapsed(0));
    assert!(state.is_collapsed(1));
    assert!(state.is_collapsed(2));
}

#[test]
fn test_is_collapsed() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    assert!(!state.is_collapsed(0));
    state.close(0);
    assert!(state.is_collapsed(0));
    assert!(!state.is_collapsed(1));
}

#[test]
fn test_is_line_hidden_inside_collapsed() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Collapse fn bar (index 1, lines 3-6)
    state.close(1);

    // Lines 4, 5, 6 are hidden (inside collapsed fold, not start line)
    assert!(state.is_line_hidden(4));
    assert!(state.is_line_hidden(5));
    assert!(state.is_line_hidden(6));
}

#[test]
fn test_is_line_hidden_outside() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Collapse fn bar (index 1, lines 3-6)
    state.close(1);

    // Line 2 is outside
    assert!(!state.is_line_hidden(2));
    // Line 7 is outside fn bar
    assert!(!state.is_line_hidden(7));
}

#[test]
fn test_is_line_hidden_fold_start() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Collapse fn bar (index 1, lines 3-6)
    state.close(1);

    // Line 3 is start of fold - visible as fold marker
    assert!(!state.is_line_hidden(3));
}

#[test]
fn test_get_fold_marker_collapsed() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Collapse fn bar (index 1, lines 3-6)
    state.close(1);

    let (hidden, preview) = state.get_fold_marker(3).unwrap();
    assert_eq!(hidden, 3); // lines 4, 5, 6 hidden
    assert_eq!(preview, "fn bar() {");
}

#[test]
fn test_get_fold_marker_open() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // fn bar is open, no marker
    assert!(state.get_fold_marker(3).is_none());
}

#[test]
fn test_get_fold_marker_no_fold() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Line 0 has no fold
    assert!(state.get_fold_marker(0).is_none());
}

#[test]
fn test_nested_fold_independent_collapse() {
    let mut state = FoldState::new();
    state.set_ranges(make_ranges());

    // Collapse inner fold fn bar (index 1, lines 3-6)
    // Keep outer fold impl Foo (index 0, lines 2-10) open
    state.close(1);

    // Lines 4-6 hidden (inside collapsed fn bar)
    assert!(state.is_line_hidden(4));
    assert!(state.is_line_hidden(5));
    assert!(state.is_line_hidden(6));

    // Lines 7-10 visible (inside open impl Foo, outside fn bar)
    assert!(!state.is_line_hidden(7));
    assert!(!state.is_line_hidden(8));
    assert!(!state.is_line_hidden(10));
}

// ========================================================================
// FoldSessionState tests
// ========================================================================

fn buffer_id(n: usize) -> BufferId {
    BufferId::from_raw(n)
}

#[test]
fn test_session_extension_create() {
    let state = FoldSessionState::create();
    assert!(!state.has_collapsed_folds());
}

#[test]
fn test_per_buffer_isolation() {
    let mut state = FoldSessionState::new();
    let buf1 = buffer_id(1);
    let buf2 = buffer_id(2);

    let fold1 = state.get_or_insert(buf1);
    fold1.set_ranges(make_ranges());
    fold1.close(0);

    let fold2 = state.get_or_insert(buf2);
    fold2.set_ranges(vec![FoldRange::new(0, 5, FoldKind::Function, "fn main() {")]);

    // Buffer 1 has collapsed folds
    assert!(state.get(buf1).unwrap().has_collapsed());
    // Buffer 2 has none
    assert!(!state.get(buf2).unwrap().has_collapsed());
}

#[test]
fn test_has_collapsed_folds() {
    let mut state = FoldSessionState::new();
    let buf = buffer_id(1);

    assert!(!state.has_collapsed_folds());

    let fold = state.get_or_insert(buf);
    fold.set_ranges(make_ranges());
    fold.close(0);

    assert!(state.has_collapsed_folds());
}

#[test]
fn test_buffers_iterator() {
    let mut state = FoldSessionState::new();

    state.get_or_insert(buffer_id(1)).set_ranges(make_ranges());
    state.get_or_insert(buffer_id(2));

    assert_eq!(state.buffers().count(), 2);
}

#[test]
fn test_get_nonexistent_buffer() {
    let state = FoldSessionState::new();
    assert!(state.get(buffer_id(999)).is_none());
}
