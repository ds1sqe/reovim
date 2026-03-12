use super::*;

use reovim_kernel::api::v1::Position;

#[test]
fn test_default_state() {
    let state = OperatorPendingState::default();
    assert!(!state.has_textobj_range());
}

#[test]
fn test_set_and_take_textobj_range() {
    let mut state = OperatorPendingState::new();

    // Set the range
    let range = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
    state.set_textobj_range(range);
    assert!(state.has_textobj_range());

    // Take consumes the range
    let taken = state.take_textobj_range();
    assert!(taken.is_some());
    assert_eq!(taken.unwrap().start, Position::new(0, 0));
    assert_eq!(taken.unwrap().end, Position::new(0, 5));

    // Now it's gone
    assert!(!state.has_textobj_range());
    assert!(state.take_textobj_range().is_none());
}

#[test]
fn test_clear() {
    let mut state = OperatorPendingState::new();

    // Set the range
    let range = TextObjRange::linewise(Position::new(1, 0), Position::new(3, 0));
    state.set_textobj_range(range);
    assert!(state.has_textobj_range());

    // Clear removes it
    state.clear();
    assert!(!state.has_textobj_range());
}

#[test]
fn test_take_returns_none_when_empty() {
    let mut state = OperatorPendingState::new();
    assert!(state.take_textobj_range().is_none());
}

#[test]
fn test_session_extension_create() {
    let state = OperatorPendingState::create();
    assert!(!state.has_textobj_range());
}

#[test]
fn test_set_overwrites_previous() {
    let mut state = OperatorPendingState::new();

    let range1 = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
    state.set_textobj_range(range1);

    let range2 = TextObjRange::linewise(Position::new(1, 0), Position::new(3, 0));
    state.set_textobj_range(range2);

    // Should get the second range
    let taken = state.take_textobj_range().unwrap();
    assert!(taken.is_linewise);
    assert_eq!(taken.start, Position::new(1, 0));
}

#[test]
fn test_clear_on_empty_is_noop() {
    let mut state = OperatorPendingState::new();
    state.clear(); // Should not panic
    assert!(!state.has_textobj_range());
}

#[test]
fn test_double_take_returns_none() {
    let mut state = OperatorPendingState::new();
    let range = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
    state.set_textobj_range(range);

    assert!(state.take_textobj_range().is_some());
    assert!(state.take_textobj_range().is_none()); // Second take returns None
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_debug() {
    let state = OperatorPendingState::new();
    let debug = format!("{state:?}");
    assert!(debug.contains("OperatorPendingState"));
}

#[test]
fn test_default_vs_new() {
    let default = OperatorPendingState::default();
    let new = OperatorPendingState::new();
    assert!(!default.has_textobj_range());
    assert!(!new.has_textobj_range());
}
