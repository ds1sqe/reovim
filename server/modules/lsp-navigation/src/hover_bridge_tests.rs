use {
    super::*, crate::hover_state::HoverSnapshot, reovim_driver_session::CursorSnapshot,
    std::sync::Arc,
};

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
// on_cursor_moved() tests (#662)
// ========================================================================

#[test]
fn cursor_moved_dismisses_when_position_changed() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<HoverState>();
    state.show("hover".to_owned(), HoverContentType::PlainText, 1, 5, 10);

    HoverBridge.on_cursor_moved(6, 10, &mut map);

    let state = map.get::<HoverState>().unwrap();
    assert!(!state.active);
    assert!(state.content.is_empty());
}

#[test]
fn cursor_moved_keeps_popup_at_origin() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<HoverState>();
    state.show("hover".to_owned(), HoverContentType::PlainText, 1, 5, 10);

    HoverBridge.on_cursor_moved(5, 10, &mut map);

    let state = map.get::<HoverState>().unwrap();
    assert!(state.active);
}

#[test]
fn cursor_moved_noop_when_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<HoverState>();

    HoverBridge.on_cursor_moved(100, 200, &mut map);

    let state = map.get::<HoverState>().unwrap();
    assert!(!state.active);
}

#[test]
fn cursor_moved_noop_no_state() {
    let mut map = ExtensionMap::new();
    HoverBridge.on_cursor_moved(0, 0, &mut map);
}

#[test]
fn cursor_moved_same_line_different_col_dismisses() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<HoverState>();
    state.show("hover".to_owned(), HoverContentType::PlainText, 1, 5, 10);

    HoverBridge.on_cursor_moved(5, 15, &mut map);

    let state = map.get::<HoverState>().unwrap();
    assert!(!state.active);
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

    // Set cursor at the same position as the hover origin.
    let snap = client.get_or_insert::<CursorSnapshot>();
    snap.line = 10;
    snap.col = 5;
    snap.buffer_id = 42;

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

#[test]
fn tick_discards_stale_snapshot_when_cursor_moved() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    // Cursor has moved away from hover origin.
    let snap = client.get_or_insert::<CursorSnapshot>();
    snap.line = 20;
    snap.col = 0;
    snap.buffer_id = 42;

    let cache = services.get_or_create::<HoverCache>();
    cache.shared().store(Arc::new(Some(HoverSnapshot {
        content: "fn foo()".to_owned(),
        content_type: HoverContentType::Markdown,
        buffer_id: 42,
        line: 10,
        col: 5,
    })));

    // tick should discard stale snapshot and return false
    assert!(!HoverBridge.tick(&mut client, &mut shared, &services));

    // HoverState should NOT be populated
    assert!(client.get::<HoverState>().is_none());

    // Cache should be consumed (snapshot was taken and discarded)
    assert!(cache.take().is_none());
}

#[test]
fn tick_delivers_when_no_cursor_snapshot() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    // No CursorSnapshot in extensions (first tick before any cursor move).
    let cache = services.get_or_create::<HoverCache>();
    cache.shared().store(Arc::new(Some(HoverSnapshot {
        content: "hello".to_owned(),
        content_type: HoverContentType::PlainText,
        buffer_id: 1,
        line: 0,
        col: 0,
    })));

    // Should deliver — no cursor snapshot means no guard to check.
    assert!(HoverBridge.tick(&mut client, &mut shared, &services));
    assert!(client.get::<HoverState>().unwrap().active);
}
