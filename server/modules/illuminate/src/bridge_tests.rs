use {
    super::*,
    crate::state::{HighlightRange, IlluminateState},
    reovim_driver_session::{
        BufferReadAccess, CursorSnapshot, ExtensionMap, bridges::ExtensionStateBridge,
    },
    reovim_kernel::api::v1::{Buffer, BufferId, BufferManager, ServiceRegistry},
    std::sync::Arc,
};

fn make_extensions_with_state(state_fn: impl FnOnce(&mut IlluminateState)) -> ExtensionMap {
    let mut ext = ExtensionMap::new();
    let state = ext.get_or_insert::<IlluminateState>();
    state_fn(state);
    ext
}

// ========================================================================
// kind / scope
// ========================================================================

#[test]
fn test_kind() {
    assert_eq!(IlluminateBridge.kind(), "illuminate");
}

#[test]
fn test_scope_is_client() {
    assert_eq!(IlluminateBridge.scope(), ExtensionScope::Client);
}

// ========================================================================
// is_active
// ========================================================================

#[test]
fn test_is_active_no_state() {
    let ext = ExtensionMap::new();
    assert!(!IlluminateBridge.is_active(&ext));
}

#[test]
fn test_is_active_inactive_state() {
    let ext = make_extensions_with_state(|_| {});
    assert!(!IlluminateBridge.is_active(&ext));
}

#[test]
fn test_is_active_active_empty_ranges() {
    let ext = make_extensions_with_state(|s| {
        s.active = true;
    });
    assert!(!IlluminateBridge.is_active(&ext));
}

#[test]
fn test_is_active_with_ranges() {
    let ext = make_extensions_with_state(|s| {
        s.active = true;
        s.ranges.push(HighlightRange {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 3,
            kind: HighlightKind::Text,
        });
    });
    assert!(IlluminateBridge.is_active(&ext));
}

// ========================================================================
// snapshot
// ========================================================================

#[test]
fn test_snapshot_no_state() {
    let ext = ExtensionMap::new();
    assert!(IlluminateBridge.snapshot(&ext).is_none());
}

#[test]
fn test_snapshot_inactive() {
    let ext = make_extensions_with_state(|_| {});
    let json = IlluminateBridge.snapshot(&ext).unwrap();
    assert_eq!(json["active"], false);
    assert_eq!(json["sequence"], 0);
}

#[test]
fn test_snapshot_active_with_ranges() {
    let ext = make_extensions_with_state(|s| {
        s.set_highlights(
            BufferId::from_raw(42),
            "hello".to_string(),
            vec![
                HighlightRange {
                    start_line: 1,
                    start_col: 5,
                    end_line: 1,
                    end_col: 10,
                    kind: HighlightKind::Read,
                },
                HighlightRange {
                    start_line: 3,
                    start_col: 0,
                    end_line: 3,
                    end_col: 5,
                    kind: HighlightKind::Write,
                },
            ],
            1,
            5,
        );
    });

    let json = IlluminateBridge.snapshot(&ext).unwrap();
    assert_eq!(json["active"], true);
    assert_eq!(json["bufferId"], 42);
    assert_eq!(json["word"], "hello");
    assert_eq!(json["sequence"], 1);

    let ranges = json["ranges"].as_array().unwrap();
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0]["startLine"], 1);
    assert_eq!(ranges[0]["startCol"], 5);
    assert_eq!(ranges[0]["endLine"], 1);
    assert_eq!(ranges[0]["endCol"], 10);
    assert_eq!(ranges[0]["kind"], "read");
    assert_eq!(ranges[1]["kind"], "write");
}

#[test]
fn test_snapshot_active_empty_ranges_shows_inactive() {
    let ext = make_extensions_with_state(|s| {
        s.active = true;
        // No ranges — should show as inactive
    });

    let json = IlluminateBridge.snapshot(&ext).unwrap();
    assert_eq!(json["active"], false);
}

// ========================================================================
// on_mode_changed
// ========================================================================

#[test]
fn test_on_mode_changed_to_insert_clears() {
    let mut ext = make_extensions_with_state(|s| {
        s.set_highlights(
            BufferId::from_raw(1),
            "x".to_string(),
            vec![HighlightRange {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 1,
                kind: HighlightKind::Text,
            }],
            0,
            0,
        );
    });

    IlluminateBridge.on_mode_changed("vim:normal", "vim:insert", &mut ext);

    let state = ext.get::<IlluminateState>().unwrap();
    assert!(!state.active);
    assert!(state.ranges.is_empty());
}

#[test]
fn test_on_mode_changed_to_command_clears() {
    let mut ext = make_extensions_with_state(|s| {
        s.active = true;
        s.word = "foo".to_string();
    });

    IlluminateBridge.on_mode_changed("vim:normal", "vim:commandline", &mut ext);

    let state = ext.get::<IlluminateState>().unwrap();
    assert!(!state.active);
}

#[test]
fn test_on_mode_changed_normal_to_visual_does_not_clear() {
    let mut ext = make_extensions_with_state(|s| {
        s.set_highlights(
            BufferId::from_raw(1),
            "x".to_string(),
            vec![HighlightRange {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 1,
                kind: HighlightKind::Text,
            }],
            0,
            0,
        );
    });

    IlluminateBridge.on_mode_changed("vim:normal", "vim:visual", &mut ext);

    let state = ext.get::<IlluminateState>().unwrap();
    assert!(state.active);
}

#[test]
fn test_on_mode_changed_no_state_no_panic() {
    let mut ext = ExtensionMap::new();
    IlluminateBridge.on_mode_changed("vim:normal", "vim:insert", &mut ext);
}

// ========================================================================
// tick
// ========================================================================

#[test]
fn test_tick_increments_idle() {
    let mut ext = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    // First tick — idle_ticks becomes 1
    let changed = IlluminateBridge.tick(&mut ext, &mut shared, &services);
    assert!(!changed);

    let state = ext.get::<IlluminateState>().unwrap();
    assert_eq!(state.idle_ticks, 1);
}

#[test]
fn test_tick_marks_computed_after_hold() {
    let mut ext = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    // Tick 3 times to reach HOLD_TICKS
    for _ in 0..3 {
        IlluminateBridge.tick(&mut ext, &mut shared, &services);
    }

    let state = ext.get::<IlluminateState>().unwrap();
    assert!(state.computed);
}

#[test]
fn test_tick_returns_false_when_already_computed() {
    let mut ext = make_extensions_with_state(|s| {
        s.computed = true;
    });
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let changed = IlluminateBridge.tick(&mut ext, &mut shared, &services);
    assert!(!changed);

    // idle_ticks should NOT have been incremented
    let state = ext.get::<IlluminateState>().unwrap();
    assert_eq!(state.idle_ticks, 0);
}

#[test]
fn test_tick_below_threshold_returns_false() {
    let mut ext = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    let changed = IlluminateBridge.tick(&mut ext, &mut shared, &services);
    assert!(!changed);
    assert!(!ext.get::<IlluminateState>().unwrap().computed);
}

// ========================================================================
// from_lsp_kind
// ========================================================================

#[test]
fn test_from_lsp_kind_none() {
    assert_eq!(from_lsp_kind(None), HighlightKind::Text);
}

#[test]
fn test_from_lsp_kind_text() {
    assert_eq!(
        from_lsp_kind(Some(reovim_driver_lsp::lsp_types::DocumentHighlightKind::TEXT)),
        HighlightKind::Text
    );
}

#[test]
fn test_from_lsp_kind_read() {
    assert_eq!(
        from_lsp_kind(Some(reovim_driver_lsp::lsp_types::DocumentHighlightKind::READ)),
        HighlightKind::Read
    );
}

#[test]
fn test_from_lsp_kind_write() {
    assert_eq!(
        from_lsp_kind(Some(reovim_driver_lsp::lsp_types::DocumentHighlightKind::WRITE)),
        HighlightKind::Write
    );
}

// ========================================================================
// tick with CursorSnapshot (#664)
// ========================================================================

#[test]
fn test_tick_reads_cursor_snapshot() {
    let mut ext = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    // Set cursor snapshot to a specific position
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 5;
    snap.col = 10;
    snap.buffer_id = 1;

    // First tick — cursor_moved detects new position, resets idle_ticks to 0, then tick adds 1
    IlluminateBridge.tick(&mut ext, &mut shared, &services);

    let state = ext.get::<IlluminateState>().unwrap();
    assert_eq!(state.shadow_line, 5);
    assert_eq!(state.shadow_col, 10);
    assert_eq!(state.idle_ticks, 1);
}

#[test]
fn test_tick_cursor_move_resets_idle() {
    let mut ext = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    // Set initial cursor position
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 5;
    snap.col = 10;

    // Tick twice to build up idle_ticks
    IlluminateBridge.tick(&mut ext, &mut shared, &services);
    IlluminateBridge.tick(&mut ext, &mut shared, &services);
    assert_eq!(ext.get::<IlluminateState>().unwrap().idle_ticks, 2);

    // Move cursor to new position
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 8;
    snap.col = 3;

    // Next tick should reset idle_ticks (cursor_moved detects change, then tick adds 1)
    IlluminateBridge.tick(&mut ext, &mut shared, &services);

    let state = ext.get::<IlluminateState>().unwrap();
    assert_eq!(state.shadow_line, 8);
    assert_eq!(state.shadow_col, 3);
    assert_eq!(state.idle_ticks, 1);
    assert!(!state.computed);
}

#[test]
fn test_tick_hold_then_move_resets() {
    let mut ext = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();

    // Set cursor position
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 1;
    snap.col = 1;

    // Reach HOLD_TICKS threshold
    for _ in 0..3 {
        IlluminateBridge.tick(&mut ext, &mut shared, &services);
    }
    assert!(ext.get::<IlluminateState>().unwrap().computed);

    // Move cursor — should reset computed flag on next tick
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 2;
    snap.col = 2;

    // cursor_moved resets computed via shadow change
    IlluminateBridge.tick(&mut ext, &mut shared, &services);

    let state = ext.get::<IlluminateState>().unwrap();
    assert!(!state.computed);
    assert_eq!(state.idle_ticks, 1);
}

// ========================================================================
// Word-matching pipeline (#664)
// ========================================================================

fn services_with_buffer(content: &str) -> (ServiceRegistry, BufferId) {
    use reovim_kernel::testing::TestBufferManager;

    let manager = Arc::new(TestBufferManager::new());
    let buf = Buffer::from_string(content);
    let bid = manager.register(buf);

    let services = ServiceRegistry::new();
    services.register(Arc::new(BufferReadAccess::new(
        Arc::clone(&manager) as Arc<dyn BufferManager>,
    )));

    (services, bid)
}

fn tick_until_hold(
    ext: &mut ExtensionMap,
    shared: &mut ExtensionMap,
    services: &ServiceRegistry,
) -> bool {
    let mut changed = false;
    for _ in 0..HOLD_TICKS {
        changed = IlluminateBridge.tick(ext, shared, services);
    }
    changed
}

#[test]
fn test_tick_produces_word_highlights() {
    let (services, bid) = services_with_buffer("let foo = bar\nlet foo = baz");

    let mut ext = ExtensionMap::new();
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 0;
    snap.col = 4; // on "foo"
    snap.buffer_id = bid.as_usize() as u64;

    let mut shared = ExtensionMap::new();
    let changed = tick_until_hold(&mut ext, &mut shared, &services);

    assert!(changed);
    let state = ext.get::<IlluminateState>().unwrap();
    assert!(state.active);
    assert_eq!(state.word, "foo");
    assert_eq!(state.ranges.len(), 2);
    assert!(state.computed);

    // Verify range positions
    assert_eq!(state.ranges[0].start_line, 0);
    assert_eq!(state.ranges[0].start_col, 4);
    assert_eq!(state.ranges[0].end_col, 7);
    assert_eq!(state.ranges[1].start_line, 1);
    assert_eq!(state.ranges[1].start_col, 4);
}

#[test]
fn test_tick_cursor_on_non_word_clears() {
    let (services, bid) = services_with_buffer("hello world");

    let mut ext = ExtensionMap::new();
    // Pre-populate with active state to verify clearing
    let state = ext.get_or_insert::<IlluminateState>();
    state.set_highlights(
        bid,
        "hello".to_string(),
        vec![HighlightRange {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 5,
            kind: HighlightKind::Text,
        }],
        0,
        0,
    );

    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 0;
    snap.col = 5; // on space between "hello" and "world"
    snap.buffer_id = bid.as_usize() as u64;

    let mut shared = ExtensionMap::new();
    let changed = tick_until_hold(&mut ext, &mut shared, &services);

    assert!(changed); // was active, now cleared
    let state = ext.get::<IlluminateState>().unwrap();
    assert!(!state.active);
    assert!(state.ranges.is_empty());
}

#[test]
fn test_tick_single_occurrence_no_highlight() {
    let (services, bid) = services_with_buffer("unique word here");

    let mut ext = ExtensionMap::new();
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 0;
    snap.col = 0; // on "unique" (appears only once)
    snap.buffer_id = bid.as_usize() as u64;

    let mut shared = ExtensionMap::new();
    let changed = tick_until_hold(&mut ext, &mut shared, &services);

    assert!(!changed); // was not active before, so clearing returns false
    let state = ext.get::<IlluminateState>().unwrap();
    assert!(!state.active);
}

#[test]
fn test_tick_same_word_optimization() {
    let (services, bid) = services_with_buffer("foo bar foo");

    let mut ext = ExtensionMap::new();
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 0;
    snap.col = 0; // on first "foo"
    snap.buffer_id = bid.as_usize() as u64;

    let mut shared = ExtensionMap::new();
    let changed = tick_until_hold(&mut ext, &mut shared, &services);
    assert!(changed);
    assert!(ext.get::<IlluminateState>().unwrap().active);

    // Move cursor to the other "foo" — same word, same buffer
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.col = 8; // second "foo"

    // Tick again — should detect same word and skip
    let changed2 = tick_until_hold(&mut ext, &mut shared, &services);
    assert!(!changed2);
    assert!(ext.get::<IlluminateState>().unwrap().active);
}

#[test]
fn test_tick_no_buffer_access_degrades_gracefully() {
    let services = ServiceRegistry::new(); // no BufferReadAccess registered

    let mut ext = ExtensionMap::new();
    let snap = ext.get_or_insert::<CursorSnapshot>();
    snap.line = 0;
    snap.col = 0;
    snap.buffer_id = 0;

    let mut shared = ExtensionMap::new();
    let changed = tick_until_hold(&mut ext, &mut shared, &services);

    assert!(!changed);
    let state = ext.get::<IlluminateState>().unwrap();
    assert!(state.computed);
    assert!(!state.active);
}

// ========================================================================
// find_word_occurrences unit tests
// ========================================================================

#[test]
fn test_find_occurrences_whole_word_only() {
    let buf = Buffer::from_string("counter count recount");
    let ranges = find_word_occurrences(&buf, "count");

    assert_eq!(ranges.len(), 1); // only standalone "count"
    assert_eq!(ranges[0].start_col, 8);
    assert_eq!(ranges[0].end_col, 13);
}

#[test]
fn test_find_occurrences_multiline() {
    let buf = Buffer::from_string("fn main() {\n    let x = main;\n}");
    let ranges = find_word_occurrences(&buf, "main");

    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0].start_line, 0);
    assert_eq!(ranges[0].start_col, 3);
    assert_eq!(ranges[1].start_line, 1);
    assert_eq!(ranges[1].start_col, 12);
}

#[test]
fn test_find_occurrences_word_at_line_boundaries() {
    let buf = Buffer::from_string("foo bar\nbaz foo");
    let ranges = find_word_occurrences(&buf, "foo");

    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0].start_col, 0); // start of line
    assert_eq!(ranges[1].start_col, 4); // end of line
    assert_eq!(ranges[1].end_col, 7);
}

#[test]
fn test_find_occurrences_adjacent_punctuation() {
    let buf = Buffer::from_string("(foo) [foo] foo.bar");
    let ranges = find_word_occurrences(&buf, "foo");

    assert_eq!(ranges.len(), 3);
    assert_eq!(ranges[0].start_col, 1); // inside parens
    assert_eq!(ranges[1].start_col, 7); // inside brackets
    assert_eq!(ranges[2].start_col, 12); // before dot
}

#[test]
fn test_find_occurrences_empty_buffer() {
    let buf = Buffer::from_string("");
    let ranges = find_word_occurrences(&buf, "word");
    assert!(ranges.is_empty());
}
