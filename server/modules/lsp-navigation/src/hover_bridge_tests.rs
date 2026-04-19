use {
    super::*, crate::hover_state::HoverSnapshot, reovim_driver_text_session::TextCursorShadow,
    std::sync::Arc,
};

fn cursor_snapshot(
    buffer_id: u64,
    line: u32,
    col: u32,
) -> reovim_driver_text_session::CursorSnapshot {
    let mut map = ExtensionMap::new();
    set_cursor_shadow(&mut map, buffer_id, line, col);
    map.get::<TextCursorShadow>().unwrap().cursor_snapshot()
}

fn set_cursor_shadow(map: &mut ExtensionMap, buffer_id: u64, line: u32, col: u32) {
    let shadow = map.get_or_insert::<TextCursorShadow>();
    shadow.update(
        reovim_kernel::api::v1::BufferId::from_raw(buffer_id as usize),
        line as usize,
        col as usize,
    );
}

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
    state.set_origin_cursor(&cursor_snapshot(1, 5, 10));

    HoverBridge.on_cursor_moved(&cursor_snapshot(1, 6, 10), &mut map);

    let state = map.get::<HoverState>().unwrap();
    assert!(!state.active);
    assert!(state.content.is_empty());
}

#[test]
fn cursor_moved_keeps_popup_at_origin() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<HoverState>();
    state.show("hover".to_owned(), HoverContentType::PlainText, 1, 5, 10);
    state.set_origin_cursor(&cursor_snapshot(1, 5, 10));

    HoverBridge.on_cursor_moved(&cursor_snapshot(1, 5, 10), &mut map);

    let state = map.get::<HoverState>().unwrap();
    assert!(state.active);
}

#[test]
fn cursor_moved_noop_when_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<HoverState>();

    HoverBridge.on_cursor_moved(&cursor_snapshot(1, 100, 200), &mut map);

    let state = map.get::<HoverState>().unwrap();
    assert!(!state.active);
}

#[test]
fn cursor_moved_noop_no_state() {
    let mut map = ExtensionMap::new();
    HoverBridge.on_cursor_moved(&reovim_driver_text_session::CursorSnapshot::SENTINEL, &mut map);
}

#[test]
fn cursor_moved_same_line_different_col_dismisses() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<HoverState>();
    state.show("hover".to_owned(), HoverContentType::PlainText, 1, 5, 10);
    state.set_origin_cursor(&cursor_snapshot(1, 5, 10));

    HoverBridge.on_cursor_moved(&cursor_snapshot(1, 5, 15), &mut map);

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
    set_cursor_shadow(&mut client, 42, 10, 5);

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
    set_cursor_shadow(&mut client, 42, 20, 0);

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

    // No text cursor shadow in extensions (first tick before any cursor move).
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

// ========================================================================
// Regression: hover requires two K presses to show (#693)
// ========================================================================
//
// Root cause: HoverCommand::execute() spawns an async task that stores
// the LSP result in HoverCache and IMMEDIATELY stops the tick scheduler
// (commands.rs, fire-and-forget block). If the tick task hasn't fired
// between the store and the stop, the cached result is never consumed.
//
// Timeline of the race:
//   T=0ms    K pressed → tick started, LSP request sent
//   T=0ms    First tick fires (immediate) → cache empty → no-op
//   T=80ms   LSP responds → async task stores in cache
//   T=80ms   Async task calls handle.stop() → TICK TASK ABORTED
//   T=100ms  Tick would have fired and consumed cache → DEAD
//
// On second K press, a new tick starts and immediately picks up the
// stale first-press result, making it appear as "works on second press."

/// Regression: cache is consumed by the tick on first press.
///
/// Before the fix, the async task stopped the tick immediately after
/// storing the result, racing the tick consumer. Now the tick stays
/// alive after `store()`, so the next tick fires and delivers normally.
///
/// This test verifies the happy path: cache populated → tick delivers.
#[test]
fn regression_cache_consumed_by_tick_on_first_press() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    set_cursor_shadow(&mut client, 42, 10, 5);

    let cache = services.get_or_create::<HoverCache>();

    // Async task stores result — tick is still running (not stopped).
    cache.shared().store(Arc::new(Some(HoverSnapshot {
        content: "first press result".to_owned(),
        content_type: HoverContentType::Markdown,
        buffer_id: 42,
        line: 10,
        col: 5,
    })));

    // Before tick runs, hover is not yet visible.
    assert!(!HoverBridge.is_active(&client), "hover must not be active before tick delivery");

    // Tick fires and consumes the cache — hover shows on first press.
    let consumed = HoverBridge.tick(&mut client, &mut shared, &services);
    assert!(consumed, "tick should consume cache from first press");

    let state = client.get::<HoverState>().unwrap();
    assert!(state.active);
    assert_eq!(state.content, "first press result");

    // Cache is drained after tick.
    assert!(cache.take().is_none(), "cache should be drained after tick");
}

/// Regression: second press's own LSP result overwrites first press's stale cache.
///
/// If both the stale first-press result and the fresh second-press result
/// arrive before the tick runs, only the latest should be delivered.
/// With `ArcSwap`, the second `store()` overwrites the first.
#[test]
fn regression_second_press_overwrites_stale_cache() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    set_cursor_shadow(&mut client, 42, 10, 5);

    let cache = services.get_or_create::<HoverCache>();
    let shared_cache = cache.shared();

    // First press result (stale, never consumed because tick was killed).
    shared_cache.store(Arc::new(Some(HoverSnapshot {
        content: "stale first".to_owned(),
        content_type: HoverContentType::PlainText,
        buffer_id: 42,
        line: 10,
        col: 5,
    })));

    // Second press sends a new LSP request. If the LSP responds quickly
    // (before the new tick fires), the store overwrites the stale result.
    shared_cache.store(Arc::new(Some(HoverSnapshot {
        content: "fresh second".to_owned(),
        content_type: HoverContentType::Markdown,
        buffer_id: 42,
        line: 10,
        col: 5,
    })));

    // Tick runs — should deliver the LATEST (second press) result.
    assert!(HoverBridge.tick(&mut client, &mut shared, &services));
    let state = client.get::<HoverState>().unwrap();
    assert_eq!(state.content, "fresh second");
    assert_eq!(state.content_type, HoverContentType::Markdown);
}
