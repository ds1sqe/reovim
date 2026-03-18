use {super::*, crate::hover_state::HoverSnapshot, std::sync::Arc};

#[test]
fn bridge_kind() {
    assert_eq!(HoverBridge.kind(), "hover");
}

#[test]
fn bridge_scope() {
    assert_eq!(HoverBridge.scope(), ExtensionScope::Client);
}

#[test]
fn snapshot_no_state_returns_none() {
    let map = ExtensionMap::new();
    assert!(HoverBridge.snapshot(&map).is_none());
}

#[test]
fn snapshot_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<HoverState>();

    let snap = HoverBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], false);
    assert!(snap.get("content").is_none());
}

#[test]
fn snapshot_active_plaintext() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<HoverState>();
    state.show("fn foo() -> bool".to_owned(), HoverContentType::PlainText, 1, 5, 12);

    let snap = HoverBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    assert_eq!(snap["content"], "fn foo() -> bool");
    assert_eq!(snap["contentType"], "plaintext");
    assert_eq!(snap["origin"]["BufferPosition"]["buffer_id"], 1);
    assert_eq!(snap["origin"]["BufferPosition"]["line"], 5);
    assert_eq!(snap["origin"]["BufferPosition"]["col"], 12);
}

#[test]
fn snapshot_active_markdown() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<HoverState>();
    state.show("**bold**".to_owned(), HoverContentType::Markdown, 2, 10, 0);

    let snap = HoverBridge.snapshot(&map).unwrap();
    assert_eq!(snap["contentType"], "markdown");
    assert_eq!(snap["content"], "**bold**");
    assert_eq!(snap["origin"]["BufferPosition"]["buffer_id"], 2);
}

#[test]
fn is_active_no_state() {
    let map = ExtensionMap::new();
    assert!(!HoverBridge.is_active(&map));
}

#[test]
fn is_active_inactive_state() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<HoverState>();
    assert!(!HoverBridge.is_active(&map));
}

#[test]
fn is_active_active_state() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<HoverState>();
    state.active = true;
    assert!(HoverBridge.is_active(&map));
}

#[test]
fn on_mode_changed_dismisses_active() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<HoverState>();
    state.show("hover".to_owned(), HoverContentType::PlainText, 1, 0, 0);
    assert!(state.active);

    HoverBridge.on_mode_changed("test:normal", "test:insert", &mut map);

    let state = map.get::<HoverState>().unwrap();
    assert!(!state.active);
    assert!(state.content.is_empty());
}

#[test]
fn on_mode_changed_noop_when_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<HoverState>();

    HoverBridge.on_mode_changed("test:normal", "test:insert", &mut map);

    let state = map.get::<HoverState>().unwrap();
    assert!(!state.active);
}

#[test]
fn on_mode_changed_noop_no_state() {
    let mut map = ExtensionMap::new();
    HoverBridge.on_mode_changed("test:normal", "test:insert", &mut map);
}

// ========================================================================
// tick() tests (#662)
// ========================================================================

#[test]
fn tick_no_cache_returns_false() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    assert!(!HoverBridge.tick(&mut client, &mut shared, &services));
}

#[test]
fn tick_empty_cache_returns_false() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    let _ = services.get_or_create::<HoverCache>();

    assert!(!HoverBridge.tick(&mut client, &mut shared, &services));
}

#[test]
fn tick_with_data_populates_hover_state() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let cache = services.get_or_create::<HoverCache>();
    cache.shared().store(Arc::new(Some(HoverSnapshot {
        content: "fn foo()".to_owned(),
        content_type: HoverContentType::Markdown,
        buffer_id: 42,
        line: 10,
        col: 5,
    })));

    // tick should consume the cache and return true
    assert!(HoverBridge.tick(&mut client, &mut shared, &services));

    // HoverState should now be populated
    let state = client.get::<HoverState>().unwrap();
    assert!(state.active);
    assert_eq!(state.content, "fn foo()");
    assert_eq!(state.content_type, HoverContentType::Markdown);
    assert_eq!(state.origin_buffer_id, 42);
    assert_eq!(state.origin_line, 10);
    assert_eq!(state.origin_col, 5);

    // Second tick should return false (cache consumed)
    assert!(!HoverBridge.tick(&mut client, &mut shared, &services));
}
