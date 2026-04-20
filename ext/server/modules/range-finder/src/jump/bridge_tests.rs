use {super::*, crate::jump::search::Direction, reovim_driver_text_session::TextInputSink};

#[test]
fn test_jump_bridge_kind() {
    assert_eq!(JumpBridge.kind(), "range-finder-jump");
}

#[test]
fn test_jump_bridge_scope() {
    assert_eq!(JumpBridge.scope(), ExtensionScope::Client);
}

#[test]
fn test_jump_bridge_snapshot_empty_map() {
    let map = ExtensionMap::new();
    assert!(JumpBridge.snapshot(&map).is_none());
}

#[test]
fn test_jump_bridge_snapshot_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<JumpSessionState>();

    let snap = JumpBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], false);
}

#[test]
fn test_jump_bridge_snapshot_active() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<JumpSessionState>();
    state.start(vec!["he he he".into()], 0, 100, Direction::Both, 0);
    state.insert_char('h');
    state.insert_char('e');

    let snap = JumpBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    let matches = snap["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0]["label"], "s");
}

#[test]
fn test_jump_bridge_is_active_empty() {
    let map = ExtensionMap::new();
    assert!(!JumpBridge.is_active(&map));
}

#[test]
fn test_jump_bridge_is_active_true() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<JumpSessionState>();
    state.start(vec!["hello".into()], 0, 0, Direction::Both, 0);
    assert!(JumpBridge.is_active(&map));
}

#[test]
fn test_jump_bridge_is_active_false() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<JumpSessionState>();
    assert!(!JumpBridge.is_active(&map));
}

#[test]
fn test_jump_bridge_snapshot_active_waiting_first() {
    // Active but not yet showing labels (WaitingFirstChar).
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<JumpSessionState>();
    state.start(vec!["hello".into()], 0, 0, Direction::Both, 0);

    let snap = JumpBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    // No matches yet, empty array.
    let matches = snap["matches"].as_array().unwrap();
    assert!(matches.is_empty());
}
