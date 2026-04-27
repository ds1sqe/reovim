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
    assert_eq!(state.origin_cursor, [0; 8]);
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
    assert_eq!(state.origin_cursor, [0; 8]);
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
fn set_origin_cursor_stores_bytes() {
    let mut state = HoverState::create();
    let cursor = reovim_driver_text_session::CursorSnapshot::from_bytes([1, 2, 3, 4, 5, 6, 7, 8]);
    state.set_origin_cursor(&cursor);
    assert_eq!(state.origin_cursor, [1, 2, 3, 4, 5, 6, 7, 8]);
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

// ========================================================================
// HoverSnapshot tests (#662)
// ========================================================================

#[test]
fn snapshot_debug() {
    let snap = HoverSnapshot {
        content: "hello".to_owned(),
        content_type: HoverContentType::Markdown,
        buffer_id: 1,
        line: 5,
        col: 3,
    };
    let debug = format!("{snap:?}");
    assert!(debug.contains("HoverSnapshot"));
    assert!(debug.contains("hello"));
}

#[test]
fn snapshot_clone() {
    let snap = HoverSnapshot {
        content: "fn foo()".to_owned(),
        content_type: HoverContentType::PlainText,
        buffer_id: 42,
        line: 10,
        col: 0,
    };
    #[allow(clippy::redundant_clone)]
    let cloned = snap.clone();
    assert_eq!(cloned.content, "fn foo()");
    assert_eq!(cloned.buffer_id, 42);
}

// ========================================================================
// HoverCache tests (#662)
// ========================================================================

#[test]
fn cache_default_returns_none() {
    let cache = HoverCache::default();
    assert!(cache.take().is_none());
}

#[test]
fn cache_store_and_take() {
    let cache = HoverCache::default();
    let shared = cache.shared();

    // Store a snapshot via the shared Arc
    shared.store(Arc::new(Some(HoverSnapshot {
        content: "hover info".to_owned(),
        content_type: HoverContentType::Markdown,
        buffer_id: 1,
        line: 5,
        col: 3,
    })));

    // Take should retrieve it
    let snap = cache.take();
    assert!(snap.is_some());
    let snap = snap.unwrap();
    assert_eq!(snap.content, "hover info");
    assert_eq!(snap.buffer_id, 1);
    assert_eq!(snap.line, 5);
    assert_eq!(snap.col, 3);

    // Second take should be None
    assert!(cache.take().is_none());
}

#[test]
fn cache_debug() {
    let cache = HoverCache::default();
    let debug = format!("{cache:?}");
    assert!(debug.contains("HoverCache"));
}
