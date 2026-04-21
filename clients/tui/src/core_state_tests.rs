use super::*;

#[test]
fn test_core_state_default() {
    let state = TuiCoreState::default();
    assert_eq!(state.my_client_id, 0);
    assert!(state.mode_name.is_empty());
    assert!(state.other_clients.is_empty());
    assert_eq!(state.line_number_mode, LineNumberMode::None);
}

#[test]
fn test_core_state_new_with_client_id() {
    let state = TuiCoreState::new(42);
    assert_eq!(state.my_client_id, 42);
}

#[test]
fn test_core_state_new_with_size() {
    let state = TuiCoreState::new_with_size(1, 80, 24);
    assert_eq!(state.my_client_id, 1);
    assert_eq!(state.width, 80);
    assert_eq!(state.height, 24);
}

#[test]
fn test_update_local_cursor() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;
    state.update_local_cursor(10, 5, 3);

    // Verify cursor stored in window_cursors
    assert!(state.window_cursors.contains_key(&10));

    // Verify get_focused_cursor() returns correct position
    let cursor = state.get_focused_cursor().expect("cursor should exist");
    assert_eq!(cursor.line, 5);
    assert_eq!(cursor.column, 3);
}

#[test]
fn test_add_remove_remote_client() {
    let mut state = TuiCoreState::new(1);

    let remote = RemoteClient {
        client_id: 2,
        display_name: "Test".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(1),
        mode: "NORMAL".to_string(),
        selection: None,
    };

    state.add_remote_client(remote);
    assert!(state.other_clients.contains_key(&2));

    state.remove_remote_client(2);
    assert!(!state.other_clients.contains_key(&2));
}

#[test]
fn test_add_remote_client_skips_self() {
    let mut state = TuiCoreState::new(1);

    let remote = RemoteClient {
        client_id: 1, // Same as my_client_id
        display_name: "Self".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(1),
        mode: "NORMAL".to_string(),
        selection: None,
    };

    state.add_remote_client(remote);
    assert!(!state.other_clients.contains_key(&1)); // Should not be added
}

#[test]
fn test_client_role_display() {
    assert_eq!(ClientRole::Owner.as_str(), "Owner");
    assert_eq!(ClientRole::Follow.as_str(), "Follow");
    assert_eq!(ClientRole::Share.as_str(), "Share");
}

#[test]
fn test_line_number_mode_default() {
    assert_eq!(LineNumberMode::default(), LineNumberMode::None);
}

#[test]
fn test_update_remote_cursor() {
    let mut state = TuiCoreState::new(1);

    let remote = RemoteClient {
        client_id: 2,
        display_name: "Peer".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(1),
        mode: "NORMAL".to_string(),
        selection: None,
    };
    state.add_remote_client(remote);

    state.update_remote_cursor(2, 10, 5);

    let remote = state.other_clients.get(&2).unwrap();
    assert_eq!(remote.cursor_line, 10);
    assert_eq!(remote.cursor_col, 5);
}

#[test]
fn test_update_remote_cursor_unknown_client() {
    let mut state = TuiCoreState::new(1);
    // Update for a non-existent client should be a no-op (no panic)
    state.update_remote_cursor(999, 10, 5);
}

#[test]
fn test_update_local_selection() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;

    let sel = SelectionState {
        start: CursorPosition { line: 1, column: 0 },
        end: CursorPosition { line: 3, column: 5 },
        mode: "char".to_string(),
    };

    state.update_local_selection(10, Some(sel));
    assert!(state.window_selections.contains_key(&10));

    // Clear selection
    state.update_local_selection(10, None);
    assert!(!state.window_selections.contains_key(&10));
}

#[test]
fn test_update_remote_selection() {
    let mut state = TuiCoreState::new(1);

    let remote = RemoteClient {
        client_id: 2,
        display_name: "Peer".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(1),
        mode: "NORMAL".to_string(),
        selection: None,
    };
    state.add_remote_client(remote);

    let sel = SelectionState {
        start: CursorPosition { line: 0, column: 0 },
        end: CursorPosition {
            line: 2,
            column: 10,
        },
        mode: "line".to_string(),
    };
    state.update_remote_selection(2, Some(sel));

    assert!(state.other_clients.get(&2).unwrap().selection.is_some());

    // Clear remote selection
    state.update_remote_selection(2, None);
    assert!(state.other_clients.get(&2).unwrap().selection.is_none());
}

#[test]
fn test_update_remote_selection_unknown_client() {
    let mut state = TuiCoreState::new(1);
    // Should be a no-op for unknown client
    state.update_remote_selection(999, Some(SelectionState::default()));
}

#[test]
fn test_cleanup_stale_cursors() {
    let mut state = TuiCoreState::new(1);

    // Set up cursors for windows 10 and 20
    state.update_local_cursor(10, 5, 3);
    state.update_local_cursor(20, 8, 1);
    state.update_local_selection(
        10,
        Some(SelectionState {
            start: CursorPosition { line: 0, column: 0 },
            end: CursorPosition { line: 1, column: 5 },
            mode: "char".to_string(),
        }),
    );
    state.update_local_selection(
        20,
        Some(SelectionState {
            start: CursorPosition { line: 2, column: 0 },
            end: CursorPosition { line: 3, column: 5 },
            mode: "line".to_string(),
        }),
    );

    // Only window 10 survives the layout change
    state.windows = vec![WindowInfo {
        window_id: 10,
        buffer_id: Some(1),
        rect: None,
        focused: true,
        opacity: None,
        primary_domain_id: 0,
        embedded_domain_ids: vec![],
        spatial_placement: None,
    }];

    state.cleanup_stale_cursors();

    assert!(state.window_cursors.contains_key(&10));
    assert!(!state.window_cursors.contains_key(&20));
    assert!(state.window_selections.contains_key(&10));
    assert!(!state.window_selections.contains_key(&20));
}

#[test]
fn test_get_focused_cursor_none() {
    let state = TuiCoreState::new(1);
    assert!(state.get_focused_cursor().is_none());
}

#[test]
fn test_get_focused_cursor_wrong_window() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;
    state.update_local_cursor(20, 5, 3); // Different window

    assert!(state.get_focused_cursor().is_none());
}

#[test]
fn test_cursor_position_default() {
    let pos = CursorPosition::default();
    assert_eq!(pos.line, 0);
    assert_eq!(pos.column, 0);
}

#[test]
fn test_selection_state_default() {
    let sel = SelectionState::default();
    assert_eq!(sel.start.line, 0);
    assert_eq!(sel.end.line, 0);
    assert!(sel.mode.is_empty());
}

#[test]
fn test_client_role_default() {
    assert_eq!(ClientRole::default(), ClientRole::Owner);
}

#[test]
fn test_remote_client_debug() {
    let remote = RemoteClient {
        client_id: 2,
        display_name: "Test".to_string(),
        cursor_line: 5,
        cursor_col: 10,
        buffer_id: Some(1),
        mode: "NORMAL".to_string(),
        selection: None,
    };
    let debug = format!("{remote:?}");
    assert!(debug.contains("RemoteClient"));
}

#[test]
fn test_line_number_mode_variants() {
    // Test all variants for equality
    assert_ne!(LineNumberMode::None, LineNumberMode::Absolute);
    assert_ne!(LineNumberMode::Absolute, LineNumberMode::Relative);
    assert_ne!(LineNumberMode::Relative, LineNumberMode::Hybrid);
}

#[test]
fn test_core_state_needs_redraw() {
    let mut state = TuiCoreState::new(1);
    assert!(!state.needs_redraw());
    state.set_needs_redraw(true);
    assert!(state.needs_redraw());
}

#[test]
fn test_core_state_last_error() {
    let mut state = TuiCoreState::new(1);
    assert!(state.last_error.is_none());
    state.last_error = Some("test error".to_string());
    assert_eq!(state.last_error.as_deref(), Some("test error"));
}

#[test]
fn test_core_state_buffer_cache() {
    let mut state = TuiCoreState::new(1);
    state
        .buffer_cache
        .insert(100, vec!["line 1".to_string(), "line 2".to_string()]);
    assert_eq!(state.buffer_cache.get(&100).unwrap().len(), 2);
    assert!(!state.buffer_cache.contains_key(&999));
}

#[test]
fn test_compute_scroll_top_cursor_in_viewport() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;
    state.update_local_cursor(10, 5, 0);

    let scroll = state.compute_scroll_top(10, 24);
    assert_eq!(scroll, 0); // cursor at line 5, viewport 0..24 — no scroll needed
}

#[test]
fn test_compute_scroll_top_cursor_below_viewport() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;
    state.update_local_cursor(10, 30, 0);

    let scroll = state.compute_scroll_top(10, 24);
    assert_eq!(scroll, 7); // cursor at 30, height 24 → 30 - 24 + 1 = 7
}

#[test]
fn test_compute_scroll_top_cursor_above_viewport() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;
    state.update_local_cursor(10, 30, 0);
    state.compute_scroll_top(10, 24); // scroll_top = 7

    // Move cursor up to line 2 (above scroll_top=7)
    state.update_local_cursor(10, 2, 0);
    let scroll = state.compute_scroll_top(10, 24);
    assert_eq!(scroll, 2); // snaps to cursor line
}

#[test]
fn test_compute_scroll_top_zero_height() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;
    state.update_local_cursor(10, 50, 0);

    let scroll = state.compute_scroll_top(10, 0);
    assert_eq!(scroll, 0); // zero height → always 0
}

#[test]
fn test_get_scroll_top_default() {
    let state = TuiCoreState::new(1);
    assert_eq!(state.get_scroll_top(999), 0);
}

#[test]
fn test_get_focused_scroll_top() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;
    state.update_local_cursor(10, 30, 0);
    state.compute_scroll_top(10, 24);

    assert_eq!(state.get_focused_scroll_top(), 7);
}

#[test]
fn test_cleanup_stale_scroll_tops() {
    let mut state = TuiCoreState::new(1);
    state.update_local_cursor(10, 30, 0);
    state.update_local_cursor(20, 40, 0);
    state.compute_scroll_top(10, 24);
    state.compute_scroll_top(20, 24);

    // Only window 10 survives layout change
    state.windows = vec![WindowInfo {
        window_id: 10,
        buffer_id: Some(1),
        rect: None,
        focused: true,
        opacity: None,
        primary_domain_id: 0,
        embedded_domain_ids: vec![],
        spatial_placement: None,
    }];

    state.cleanup_stale_cursors();
    assert!(state.scroll_tops.contains_key(&10));
    assert!(!state.scroll_tops.contains_key(&20));
}

#[test]
fn test_get_focused_buffer_id_found() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;
    state.windows = vec![WindowInfo {
        window_id: 10,
        buffer_id: Some(42),
        rect: None,
        focused: true,
        opacity: None,
        primary_domain_id: 0,
        embedded_domain_ids: vec![],
        spatial_placement: None,
    }];
    assert_eq!(state.get_focused_buffer_id(), Some(42));
}

#[test]
fn test_get_focused_buffer_id_no_match() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 99;
    state.windows = vec![WindowInfo {
        window_id: 10,
        buffer_id: Some(1),
        rect: None,
        focused: false,
        opacity: None,
        primary_domain_id: 0,
        embedded_domain_ids: vec![],
        spatial_placement: None,
    }];
    assert_eq!(state.get_focused_buffer_id(), None);
}

#[test]
fn test_get_focused_buffer_id_empty_windows() {
    let state = TuiCoreState::new(1);
    assert_eq!(state.get_focused_buffer_id(), None);
}

#[test]
fn test_get_focused_buffer_id_none_buffer() {
    let mut state = TuiCoreState::new(1);
    state.focused_window_id = 10;
    state.windows = vec![WindowInfo {
        window_id: 10,
        buffer_id: None,
        rect: None,
        focused: true,
        opacity: None,
        primary_domain_id: 0,
        embedded_domain_ids: vec![],
        spatial_placement: None,
    }];
    assert_eq!(state.get_focused_buffer_id(), None);
}
