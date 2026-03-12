use super::*;

#[test]
fn create_defaults() {
    let state = HoverState::create();
    assert!(!state.active);
    assert!(state.content.is_empty());
    assert_eq!(state.content_type, HoverContentType::PlainText);
    assert_eq!(state.origin_buffer_id, 0);
    assert_eq!(state.origin_line, 0);
    assert_eq!(state.origin_col, 0);
}

#[test]
fn state_debug() {
    let state = HoverState::create();
    let debug = format!("{state:?}");
    assert!(debug.contains("HoverState"));
}

#[test]
fn show_sets_all_fields() {
    let mut state = HoverState::create();
    state.show("fn foo() -> bool".to_owned(), HoverContentType::Markdown, 42, 5, 12);
    assert!(state.active);
    assert_eq!(state.content, "fn foo() -> bool");
    assert_eq!(state.content_type, HoverContentType::Markdown);
    assert_eq!(state.origin_buffer_id, 42);
    assert_eq!(state.origin_line, 5);
    assert_eq!(state.origin_col, 12);
}

#[test]
fn show_overwrites_previous() {
    let mut state = HoverState::create();
    state.show("first".to_owned(), HoverContentType::PlainText, 1, 0, 0);
    state.show("second".to_owned(), HoverContentType::Markdown, 2, 10, 5);
    assert_eq!(state.content, "second");
    assert_eq!(state.content_type, HoverContentType::Markdown);
    assert_eq!(state.origin_buffer_id, 2);
}

#[test]
fn dismiss_clears_state() {
    let mut state = HoverState::create();
    state.show("hover text".to_owned(), HoverContentType::PlainText, 1, 5, 12);
    state.dismiss();
    assert!(!state.active);
    assert!(state.content.is_empty());
}

#[test]
fn dismiss_when_inactive_is_noop() {
    let mut state = HoverState::create();
    state.dismiss();
    assert!(!state.active);
}

#[test]
fn content_type_debug() {
    let ct = HoverContentType::Markdown;
    let debug = format!("{ct:?}");
    assert!(debug.contains("Markdown"));
}

#[test]
fn content_type_clone() {
    let ct = HoverContentType::PlainText;
    #[allow(clippy::clone_on_copy)]
    let cloned = ct.clone();
    assert_eq!(ct, cloned);
}

#[test]
fn content_type_copy() {
    let ct = HoverContentType::Markdown;
    let copied = ct;
    assert_eq!(ct, copied);
}

#[test]
fn content_type_eq() {
    assert_eq!(HoverContentType::PlainText, HoverContentType::PlainText);
    assert_ne!(HoverContentType::PlainText, HoverContentType::Markdown);
}
