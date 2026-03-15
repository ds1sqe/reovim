use {
    super::*,
    crate::state::{HighlightRange, IlluminateState},
    reovim_driver_session::{ExtensionMap, bridges::ExtensionStateBridge},
    reovim_kernel::api::v1::{BufferId, ServiceRegistry},
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
