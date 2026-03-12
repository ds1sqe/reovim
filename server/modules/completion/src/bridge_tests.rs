use reovim_driver_completion::CompletionKind;

use {super::*, crate::state::CompletionItemSnapshot};

fn make_snapshot(label: &str, kind: CompletionKind) -> CompletionItemSnapshot {
    CompletionItemSnapshot {
        label: label.to_owned(),
        insert_text: label.to_owned(),
        is_snippet: false,
        kind_abbrev: kind.abbreviation().to_owned(),
        kind,
        detail: None,
        source_id: "test".to_owned(),
    }
}

#[test]
fn bridge_kind() {
    assert_eq!(CompletionBridge.kind(), "completion");
}

#[test]
fn bridge_scope() {
    assert_eq!(CompletionBridge.scope(), ExtensionScope::Client);
}

#[test]
fn snapshot_no_state_returns_none() {
    let map = ExtensionMap::new();
    assert!(CompletionBridge.snapshot(&map).is_none());
}

#[test]
fn snapshot_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<CompletionState>();

    let snap = CompletionBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], false);
    // Inactive snapshot should be minimal.
    assert!(snap.get("items").is_none());
}

#[test]
fn snapshot_active_empty() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<CompletionState>();
    state.active = true;
    state.prefix = "fo".to_owned();

    let snap = CompletionBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    assert_eq!(snap["prefix"], "fo");
    assert_eq!(snap["selected"], 0);
    assert_eq!(snap["scrollOffset"], 0);
    assert!(snap["items"].as_array().unwrap().is_empty());
}

#[test]
fn snapshot_active_with_items() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<CompletionState>();
    state.active = true;
    state.prefix = "pr".to_owned();
    state.selected = 1;
    state.items = vec![
        CompletionItemSnapshot {
            label: "println".to_owned(),
            insert_text: "println".to_owned(),
            is_snippet: false,
            kind_abbrev: "fn".to_owned(),
            kind: CompletionKind::Function,
            detail: Some("macro".to_owned()),
            source_id: "lsp".to_owned(),
        },
        make_snapshot("print", CompletionKind::Function),
    ];

    let snap = CompletionBridge.snapshot(&map).unwrap();
    assert_eq!(snap["selected"], 1);
    assert_eq!(snap["prefix"], "pr");

    let items = snap["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["label"], "println");
    assert_eq!(items[0]["kindAbbrev"], "fn");
    assert_eq!(items[0]["sourceId"], "lsp");
    assert_eq!(items[0]["detail"], "macro");
    assert_eq!(items[1]["label"], "print");
    assert!(items[1].get("detail").is_none());
}

// ========================================================================
// on_mode_changed tests (#521)
// ========================================================================

#[test]
fn on_mode_changed_dismisses_active_popup() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<CompletionState>();
    state.open(vec![make_snapshot("foo", CompletionKind::Function)], "f");
    assert!(state.active);

    CompletionBridge.on_mode_changed("test:insert", "test:normal", &mut map);

    let state = map.get::<CompletionState>().unwrap();
    assert!(!state.active);
    assert!(state.items.is_empty());
}

#[test]
fn on_mode_changed_noop_when_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<CompletionState>();

    // Not active — should not panic.
    CompletionBridge.on_mode_changed("test:insert", "test:normal", &mut map);

    let state = map.get::<CompletionState>().unwrap();
    assert!(!state.active);
}

#[test]
fn on_mode_changed_noop_no_state() {
    let mut map = ExtensionMap::new();
    // No CompletionState at all — should not panic.
    CompletionBridge.on_mode_changed("test:insert", "test:normal", &mut map);
}

// ========================================================================
// is_active tests
// ========================================================================

#[test]
fn is_active_no_state() {
    let map = ExtensionMap::new();
    assert!(!CompletionBridge.is_active(&map));
}

#[test]
fn is_active_inactive_state() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<CompletionState>();
    assert!(!CompletionBridge.is_active(&map));
}

#[test]
fn is_active_active_state() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<CompletionState>();
    state.active = true;
    assert!(CompletionBridge.is_active(&map));
}
