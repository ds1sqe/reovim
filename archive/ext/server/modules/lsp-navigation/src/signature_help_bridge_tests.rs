use super::*;

#[test]
fn bridge_kind() {
    assert_eq!(SignatureHelpBridge.kind(), "signature-help");
}

#[test]
fn bridge_scope() {
    assert_eq!(SignatureHelpBridge.scope(), ExtensionScope::Client);
}

#[test]
fn snapshot_no_state_returns_none() {
    let map = ExtensionMap::new();
    assert!(SignatureHelpBridge.snapshot(&map).is_none());
}

#[test]
fn snapshot_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<SignatureHelpState>();

    let snap = SignatureHelpBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], false);
    assert!(snap.get("label").is_none());
}

#[test]
fn snapshot_active() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<SignatureHelpState>();
    state.show("fn foo(x: i32)".to_owned(), 1, 5, 12);

    let snap = SignatureHelpBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    assert_eq!(snap["label"], "fn foo(x: i32)");
    assert_eq!(snap["origin"]["BufferPosition"]["buffer_id"], 1);
    assert_eq!(snap["origin"]["BufferPosition"]["line"], 5);
    assert_eq!(snap["origin"]["BufferPosition"]["col"], 12);
}

#[test]
fn is_active_no_state() {
    let map = ExtensionMap::new();
    assert!(!SignatureHelpBridge.is_active(&map));
}

#[test]
fn is_active_inactive_state() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<SignatureHelpState>();
    assert!(!SignatureHelpBridge.is_active(&map));
}

#[test]
fn is_active_active_state() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<SignatureHelpState>();
    state.active = true;
    assert!(SignatureHelpBridge.is_active(&map));
}

#[test]
fn on_mode_changed_dismisses_active() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<SignatureHelpState>();
    state.show("fn foo()".to_owned(), 1, 0, 0);
    assert!(state.active);

    SignatureHelpBridge.on_mode_changed("test:normal", "test:insert", &mut map);

    let state = map.get::<SignatureHelpState>().unwrap();
    assert!(!state.active);
    assert!(state.label.is_empty());
}

#[test]
fn on_mode_changed_noop_when_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<SignatureHelpState>();

    SignatureHelpBridge.on_mode_changed("test:normal", "test:insert", &mut map);

    let state = map.get::<SignatureHelpState>().unwrap();
    assert!(!state.active);
}

#[test]
fn on_mode_changed_noop_no_state() {
    let mut map = ExtensionMap::new();
    SignatureHelpBridge.on_mode_changed("test:normal", "test:insert", &mut map);
}
