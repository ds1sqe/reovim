use super::*;

#[test]
fn create_defaults() {
    let state = SignatureHelpState::create();
    assert!(!state.active);
    assert!(state.label.is_empty());
    assert_eq!(state.origin_buffer_id, 0);
    assert_eq!(state.origin_line, 0);
    assert_eq!(state.origin_col, 0);
}

#[test]
fn state_debug() {
    let state = SignatureHelpState::create();
    let debug = format!("{state:?}");
    assert!(debug.contains("SignatureHelpState"));
}

#[test]
fn show_sets_all_fields() {
    let mut state = SignatureHelpState::create();
    state.show("fn foo(x: i32) -> bool".to_owned(), 42, 5, 12);
    assert!(state.active);
    assert_eq!(state.label, "fn foo(x: i32) -> bool");
    assert_eq!(state.origin_buffer_id, 42);
    assert_eq!(state.origin_line, 5);
    assert_eq!(state.origin_col, 12);
}

#[test]
fn show_overwrites_previous() {
    let mut state = SignatureHelpState::create();
    state.show("first".to_owned(), 1, 0, 0);
    state.show("second".to_owned(), 2, 10, 5);
    assert_eq!(state.label, "second");
    assert_eq!(state.origin_buffer_id, 2);
}

#[test]
fn dismiss_clears_state() {
    let mut state = SignatureHelpState::create();
    state.show("fn foo()".to_owned(), 1, 5, 12);
    state.dismiss();
    assert!(!state.active);
    assert!(state.label.is_empty());
}

#[test]
fn dismiss_when_inactive_is_noop() {
    let mut state = SignatureHelpState::create();
    state.dismiss();
    assert!(!state.active);
}
