use super::*;

fn make_snapshot(label: &str, kind: CompletionKind) -> CompletionItemSnapshot {
    CompletionItemSnapshot {
        label: label.to_owned(),
        insert_text: label.to_owned(),
        is_snippet: false,
        kind_abbrev: kind.abbreviation().to_owned(),
        kind_icon: kind.icon().to_owned(),
        kind,
        detail: None,
        source_id: "test".to_owned(),
    }
}

fn make_driver_item(label: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_owned(),
        insert_text: label.to_owned(),
        kind: CompletionKind::Function,
        detail: Some("fn()".to_owned()),
        documentation: None,
        source_id: "lsp",
        is_snippet: false,
        sort_priority: 100,
    }
}

#[test]
fn state_create_defaults() {
    let state = CompletionState::create();
    assert!(!state.active);
    assert!(state.items.is_empty());
    assert_eq!(state.selected, 0);
    assert!(state.prefix.is_empty());
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn state_debug() {
    let state = CompletionState::create();
    let debug = format!("{state:?}");
    assert!(debug.contains("CompletionState"));
}

#[test]
fn open_sets_items_and_prefix() {
    let mut state = CompletionState::create();
    let items = vec![
        make_snapshot("foo", CompletionKind::Function),
        make_snapshot("bar", CompletionKind::Variable),
    ];
    state.open(items, "f");
    assert!(state.active);
    assert_eq!(state.items.len(), 2);
    assert_eq!(state.selected, 0);
    assert_eq!(state.prefix, "f");
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn open_resets_previous_state() {
    let mut state = CompletionState::create();
    state.open(vec![make_snapshot("a", CompletionKind::Text)], "prefix");
    state.selected = 0;
    state.scroll_offset = 5;

    // Open again with new items.
    state.open(
        vec![
            make_snapshot("x", CompletionKind::Text),
            make_snapshot("y", CompletionKind::Text),
        ],
        "new",
    );
    assert_eq!(state.items.len(), 2);
    assert_eq!(state.selected, 0);
    assert_eq!(state.scroll_offset, 0);
    assert_eq!(state.prefix, "new");
}

#[test]
fn close_clears_state() {
    let mut state = CompletionState::create();
    state.open(vec![make_snapshot("foo", CompletionKind::Function)], "f");
    state.close();
    assert!(!state.active);
    assert!(state.items.is_empty());
    assert_eq!(state.selected, 0);
    assert_eq!(state.scroll_offset, 0);
    assert!(state.prefix.is_empty());
}

#[test]
fn next_wraps_around() {
    let mut state = CompletionState::create();
    state.open(
        vec![
            make_snapshot("a", CompletionKind::Text),
            make_snapshot("b", CompletionKind::Text),
            make_snapshot("c", CompletionKind::Text),
        ],
        "",
    );
    assert_eq!(state.selected, 0);
    state.next();
    assert_eq!(state.selected, 1);
    state.next();
    assert_eq!(state.selected, 2);
    state.next();
    assert_eq!(state.selected, 0); // Wrapped
}

#[test]
fn prev_wraps_around() {
    let mut state = CompletionState::create();
    state.open(
        vec![
            make_snapshot("a", CompletionKind::Text),
            make_snapshot("b", CompletionKind::Text),
            make_snapshot("c", CompletionKind::Text),
        ],
        "",
    );
    assert_eq!(state.selected, 0);
    state.prev();
    assert_eq!(state.selected, 2); // Wrapped to end
    state.prev();
    assert_eq!(state.selected, 1);
}

#[test]
fn next_on_empty_is_noop() {
    let mut state = CompletionState::create();
    state.next();
    assert_eq!(state.selected, 0);
}

#[test]
fn prev_on_empty_is_noop() {
    let mut state = CompletionState::create();
    state.prev();
    assert_eq!(state.selected, 0);
}

#[test]
fn selected_item_when_active() {
    let mut state = CompletionState::create();
    state.open(
        vec![
            make_snapshot("first", CompletionKind::Function),
            make_snapshot("second", CompletionKind::Variable),
        ],
        "",
    );
    let item = state.selected_item().unwrap();
    assert_eq!(item.label, "first");

    state.next();
    let item = state.selected_item().unwrap();
    assert_eq!(item.label, "second");
}

#[test]
fn selected_item_when_inactive() {
    let state = CompletionState::create();
    assert!(state.selected_item().is_none());
}

#[test]
fn selected_item_empty_items() {
    let mut state = CompletionState::create();
    state.active = true;
    assert!(state.selected_item().is_none());
}

#[test]
fn snapshot_from_item() {
    let item = make_driver_item("my_func");
    let snap = CompletionItemSnapshot::from_item(&item);
    assert_eq!(snap.label, "my_func");
    assert_eq!(snap.insert_text, "my_func");
    assert!(!snap.is_snippet);
    assert_eq!(snap.kind_abbrev, "fn");
    assert_eq!(snap.kind_icon, CompletionKind::Function.icon());
    assert_eq!(snap.kind, CompletionKind::Function);
    assert_eq!(snap.detail.as_deref(), Some("fn()"));
    assert_eq!(snap.source_id, "lsp");
}

#[test]
fn snapshot_from_item_no_detail() {
    let item = CompletionItem {
        label: "keyword".to_owned(),
        insert_text: "keyword".to_owned(),
        kind: CompletionKind::Keyword,
        detail: None,
        documentation: None,
        source_id: "buffer",
        is_snippet: false,
        sort_priority: 50,
    };
    let snap = CompletionItemSnapshot::from_item(&item);
    assert_eq!(snap.label, "keyword");
    assert_eq!(snap.kind_abbrev, "kw");
    assert!(snap.detail.is_none());
    assert_eq!(snap.source_id, "buffer");
}

#[test]
fn snapshot_clone() {
    let snap = make_snapshot("test", CompletionKind::Method);
    #[allow(clippy::redundant_clone)]
    let cloned = snap.clone();
    assert_eq!(cloned.label, "test");
    assert_eq!(cloned.kind, CompletionKind::Method);
}

#[test]
fn snapshot_debug() {
    let snap = make_snapshot("x", CompletionKind::Text);
    let debug = format!("{snap:?}");
    assert!(debug.contains("CompletionItemSnapshot"));
}
