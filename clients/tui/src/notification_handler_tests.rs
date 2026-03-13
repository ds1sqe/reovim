use super::*;

// Mock context for testing
struct MockContext {
    state: TuiCoreState,
    buffer_modified_calls: Vec<u64>,
    option_changed_calls: Vec<String>,
    extensions: Vec<Box<dyn ClientModule>>,
}

impl MockContext {
    fn new(client_id: u64) -> Self {
        Self {
            state: TuiCoreState::new(client_id),
            buffer_modified_calls: Vec::new(),
            option_changed_calls: Vec::new(),
            extensions: Vec::new(),
        }
    }

    fn with_extensions(mut self, extensions: Vec<Box<dyn ClientModule>>) -> Self {
        self.extensions = extensions;
        self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl NotificationContext for MockContext {
    fn state_mut(&mut self) -> &mut TuiCoreState {
        &mut self.state
    }

    fn client_mut(&mut self) -> &mut TuiGrpcClient {
        // This would panic in tests that try to use the client
        // For unit tests, we avoid calling methods that need the client
        unimplemented!("Mock context doesn't have a real client")
    }

    fn on_buffer_modified(&mut self, buffer_id: u64) {
        self.buffer_modified_calls.push(buffer_id);
    }

    fn on_option_changed(&mut self, name: &str, _value: Option<OptionValue>) {
        self.option_changed_calls.push(name.to_string());
    }

    fn extensions_mut(&mut self) -> &mut [Box<dyn ClientModule>] {
        &mut self.extensions
    }
}

#[test]
fn test_notification_result_variants() {
    // Just ensure the variants exist
    let _ = NotificationResult::Redraw;
    let _ = NotificationResult::NoRedraw;
    let _ = NotificationResult::Stop;
}

#[test]
fn test_mock_context() {
    let mut ctx = MockContext::new(1);
    assert_eq!(ctx.state_mut().my_client_id, 1);

    ctx.on_buffer_modified(42);
    assert_eq!(ctx.buffer_modified_calls, vec![42]);

    ctx.on_option_changed("colorscheme", None);
    assert_eq!(ctx.option_changed_calls, vec!["colorscheme".to_string()]);
}

// =========================================================================
// Default trait method tests
// =========================================================================

#[test]
fn test_default_on_buffer_modified_is_noop() {
    // Ensure the default trait implementation doesn't panic
    struct MinimalContext {
        state: TuiCoreState,
        extensions: Vec<Box<dyn ClientModule>>,
    }
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl NotificationContext for MinimalContext {
        fn state_mut(&mut self) -> &mut TuiCoreState {
            &mut self.state
        }
        fn client_mut(&mut self) -> &mut crate::grpc_client::TuiGrpcClient {
            unimplemented!()
        }
        fn extensions_mut(&mut self) -> &mut [Box<dyn ClientModule>] {
            &mut self.extensions
        }
    }

    let mut ctx = MinimalContext {
        state: TuiCoreState::new(1),
        extensions: Vec::new(),
    };
    // Default trait methods should be no-ops and not panic
    ctx.on_buffer_modified(42);
    ctx.on_option_changed("test", None);
    assert!(ctx.on_capture_request(1, "plain_text", 1).is_none());
    ctx.on_resize(80, 24);
}

// =========================================================================
// Synchronous notification dispatch tests (no client needed)
// =========================================================================

fn make_notif(payload: reovim_protocol::v2::notification::Payload) -> Notification {
    Notification {
        event_type: String::new(),
        timestamp_ms: 0,
        payload: Some(payload),
    }
}

#[tokio::test]
async fn test_handle_empty_payload() {
    let mut ctx = MockContext::new(1);
    let notif = Notification {
        event_type: String::new(),
        timestamp_ms: 0,
        payload: None,
    };
    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::NoRedraw));
}

#[tokio::test]
async fn test_handle_mode_changed_local() {
    use reovim_protocol::v2::ModeChangedPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::ModeChanged(ModeChangedPayload {
        name: "insert".to_string(),
        display: "INSERT".to_string(),
        is_insert: true,
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert_eq!(ctx.state.mode_name, "insert");
    assert_eq!(ctx.state.mode_display, "INSERT");
    assert!(ctx.state.is_insert_mode());
}

#[tokio::test]
async fn test_handle_mode_changed_remote() {
    use reovim_protocol::v2::ModeChangedPayload;

    let mut ctx = MockContext::new(1);
    // Add a remote client first
    ctx.state.add_remote_client(RemoteClient {
        client_id: 2,
        display_name: "other".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(1),
        mode: "NORMAL".to_string(),
        selection: None,
    });

    let notif = make_notif(Payload::ModeChanged(ModeChangedPayload {
        name: "visual".to_string(),
        display: "VISUAL".to_string(),
        is_insert: false,
        client_id: 2,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Local mode should NOT have changed
    assert!(ctx.state.mode_name.is_empty());
    // Remote mode should be updated
    assert_eq!(ctx.state.other_clients[&2].mode, "VISUAL");
}

#[tokio::test]
async fn test_handle_mode_changed_remote_unknown_client() {
    use reovim_protocol::v2::ModeChangedPayload;

    let mut ctx = MockContext::new(1);
    // No remote client registered for id 99
    let notif = make_notif(Payload::ModeChanged(ModeChangedPayload {
        name: "visual".to_string(),
        display: "VISUAL".to_string(),
        is_insert: false,
        client_id: 99,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Should not panic, mode display stays empty
    assert!(ctx.state.mode_display.is_empty());
}

#[tokio::test]
async fn test_handle_cursor_moved_local() {
    use reovim_protocol::v2::{CursorMovedPayload, Position};

    let mut ctx = MockContext::new(1);
    ctx.state.focused_window_id = 5;

    let notif = make_notif(Payload::CursorMoved(CursorMovedPayload {
        window_id: 5,
        position: Some(Position {
            line: 10,
            column: 3,
        }),
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));

    let cursor = ctx.state.get_focused_cursor().unwrap();
    assert_eq!(cursor.line, 10);
    assert_eq!(cursor.column, 3);
}

#[tokio::test]
async fn test_handle_cursor_moved_remote() {
    use reovim_protocol::v2::{CursorMovedPayload, Position};

    let mut ctx = MockContext::new(1);
    ctx.state.add_remote_client(RemoteClient {
        client_id: 2,
        display_name: "other".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(1),
        mode: "NORMAL".to_string(),
        selection: None,
    });

    let notif = make_notif(Payload::CursorMoved(CursorMovedPayload {
        window_id: 10,
        position: Some(Position {
            line: 7,
            column: 15,
        }),
        client_id: 2,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert_eq!(ctx.state.other_clients[&2].cursor_line, 7);
    assert_eq!(ctx.state.other_clients[&2].cursor_col, 15);
}

#[tokio::test]
async fn test_handle_cursor_moved_no_position() {
    use reovim_protocol::v2::CursorMovedPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::CursorMoved(CursorMovedPayload {
        window_id: 5,
        position: None,
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // No cursor should be stored
    assert!(ctx.state.get_focused_cursor().is_none());
}

#[tokio::test]
async fn test_handle_layout_changed() {
    use reovim_protocol::v2::{LayoutChangedPayload, WindowInfo};

    let mut ctx = MockContext::new(1);
    // Pre-populate buffer cache so the handler doesn't try to fetch
    // (MockContext.client_mut() panics).
    ctx.state
        .buffer_cache
        .insert(100, vec!["line1".to_string()]);
    ctx.state
        .buffer_cache
        .insert(200, vec!["line2".to_string()]);

    let notif = make_notif(Payload::LayoutChanged(LayoutChangedPayload {
        focused_window_id: Some(3),
        windows: vec![
            WindowInfo {
                window_id: 3,
                buffer_id: Some(100),
                rect: None,
                focused: true,
                opacity: None,
            },
            WindowInfo {
                window_id: 4,
                buffer_id: Some(200),
                rect: None,
                focused: false,
                opacity: None,
            },
        ],
        client_id: 1,
        active_tab_id: None,
        tabs: Vec::new(),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert_eq!(ctx.state.focused_window_id, 3);
    assert_eq!(ctx.state.windows.len(), 2);
}

#[tokio::test]
async fn test_handle_render_complete() {
    use reovim_protocol::v2::RenderCompletePayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::RenderComplete(RenderCompletePayload { frame_id: 42 }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
}

#[tokio::test]
async fn test_handle_detach() {
    use reovim_protocol::v2::DetachPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::Detach(DetachPayload {
        reason: "server shutting down".to_string(),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Stop));
}

#[tokio::test]
async fn test_handle_option_changed() {
    use reovim_protocol::v2::OptionChangedPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::OptionChanged(OptionChangedPayload {
        name: "colorscheme".to_string(),
        value: Some(OptionValue::StringValue("dark".to_string())),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert_eq!(ctx.option_changed_calls, vec!["colorscheme"]);
}

#[tokio::test]
async fn test_handle_presence_joined_remote() {
    use reovim_protocol::v2::{ClientPresence, PresenceJoinedPayload};

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceJoined(PresenceJoinedPayload {
        client: Some(ClientPresence {
            client_id: 5,
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
            buffer_id: Some(100),
            visible_lines: None,
            mode: "NORMAL".to_string(),
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(ctx.state.other_clients.contains_key(&5));
    assert_eq!(ctx.state.other_clients[&5].display_name, "laptop");
}

#[tokio::test]
async fn test_handle_presence_joined_self_ignored() {
    use reovim_protocol::v2::{ClientPresence, PresenceJoinedPayload};

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceJoined(PresenceJoinedPayload {
        client: Some(ClientPresence {
            client_id: 1, // Same as our ID
            client_type: "tui".to_string(),
            display_name: "self".to_string(),
            buffer_id: Some(100),
            visible_lines: None,
            mode: "NORMAL".to_string(),
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(!ctx.state.other_clients.contains_key(&1));
}

#[tokio::test]
async fn test_handle_presence_joined_no_client() {
    use reovim_protocol::v2::PresenceJoinedPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceJoined(PresenceJoinedPayload { client: None }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(ctx.state.other_clients.is_empty());
}

#[tokio::test]
async fn test_handle_presence_updated_buffer_changed() {
    use reovim_protocol::v2::{ClientPresence, PresenceUpdatedPayload};

    let mut ctx = MockContext::new(1);
    // First add the remote client
    ctx.state.add_remote_client(RemoteClient {
        client_id: 2,
        display_name: "remote".to_string(),
        cursor_line: 10,
        cursor_col: 5,
        buffer_id: Some(100),
        mode: "NORMAL".to_string(),
        selection: Some(SelectionState::default()),
    });

    // Update with different buffer_id => cursor and selection reset
    let notif = make_notif(Payload::PresenceUpdated(PresenceUpdatedPayload {
        client: Some(ClientPresence {
            client_id: 2,
            client_type: "tui".to_string(),
            display_name: "remote".to_string(),
            buffer_id: Some(200), // Different buffer
            visible_lines: None,
            mode: "INSERT".to_string(),
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    let remote = &ctx.state.other_clients[&2];
    assert_eq!(remote.cursor_line, 0); // Reset
    assert_eq!(remote.cursor_col, 0); // Reset
    assert!(remote.selection.is_none()); // Cleared
    assert_eq!(remote.buffer_id, Some(200));
    assert_eq!(remote.mode, "INSERT");
}

#[tokio::test]
async fn test_handle_presence_updated_same_buffer_preserves_cursor() {
    use reovim_protocol::v2::{ClientPresence, PresenceUpdatedPayload};

    let mut ctx = MockContext::new(1);
    ctx.state.add_remote_client(RemoteClient {
        client_id: 2,
        display_name: "remote".to_string(),
        cursor_line: 10,
        cursor_col: 5,
        buffer_id: Some(100),
        mode: "NORMAL".to_string(),
        selection: None,
    });

    // Update with same buffer_id => cursor preserved
    let notif = make_notif(Payload::PresenceUpdated(PresenceUpdatedPayload {
        client: Some(ClientPresence {
            client_id: 2,
            client_type: "tui".to_string(),
            display_name: "remote-updated".to_string(),
            buffer_id: Some(100), // Same buffer
            visible_lines: None,
            mode: "VISUAL".to_string(),
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    let remote = &ctx.state.other_clients[&2];
    assert_eq!(remote.cursor_line, 10); // Preserved
    assert_eq!(remote.cursor_col, 5); // Preserved
    assert_eq!(remote.display_name, "remote-updated");
}

#[tokio::test]
async fn test_handle_presence_updated_self_ignored() {
    use reovim_protocol::v2::{ClientPresence, PresenceUpdatedPayload};

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceUpdated(PresenceUpdatedPayload {
        client: Some(ClientPresence {
            client_id: 1, // Self
            client_type: "tui".to_string(),
            display_name: "self".to_string(),
            buffer_id: Some(100),
            visible_lines: None,
            mode: "NORMAL".to_string(),
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(ctx.state.other_clients.is_empty());
}

#[tokio::test]
async fn test_handle_presence_updated_no_client() {
    use reovim_protocol::v2::PresenceUpdatedPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceUpdated(PresenceUpdatedPayload { client: None }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
}

#[tokio::test]
async fn test_handle_presence_left() {
    use reovim_protocol::v2::PresenceLeftPayload;

    let mut ctx = MockContext::new(1);
    ctx.state.add_remote_client(RemoteClient {
        client_id: 5,
        display_name: "gone".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(1),
        mode: "NORMAL".to_string(),
        selection: None,
    });
    assert!(ctx.state.other_clients.contains_key(&5));

    let notif = make_notif(Payload::PresenceLeft(PresenceLeftPayload {
        client_id: 5,
        display_name: "gone".to_string(),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(!ctx.state.other_clients.contains_key(&5));
}

#[tokio::test]
async fn test_handle_selection_changed_local_with_selection() {
    use reovim_protocol::v2::{Position, Selection, SelectionChangedPayload};

    let mut ctx = MockContext::new(1);
    ctx.state.focused_window_id = 5;

    let notif = make_notif(Payload::SelectionChanged(SelectionChangedPayload {
        window_id: 5,
        has_selection: true,
        selection: Some(Selection {
            start: Some(Position { line: 1, column: 3 }),
            end: Some(Position { line: 4, column: 7 }),
        }),
        visual_mode: Some("char".to_string()),
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    let sel = ctx.state.window_selections.get(&5).unwrap();
    assert_eq!(sel.start.line, 1);
    assert_eq!(sel.start.column, 3);
    assert_eq!(sel.end.line, 4);
    assert_eq!(sel.end.column, 7);
    assert_eq!(sel.mode, "char");
}

#[tokio::test]
async fn test_handle_selection_changed_local_clear() {
    use reovim_protocol::v2::SelectionChangedPayload;

    let mut ctx = MockContext::new(1);
    ctx.state.focused_window_id = 5;
    // Pre-populate a selection
    ctx.state
        .window_selections
        .insert(5, SelectionState::default());

    let notif = make_notif(Payload::SelectionChanged(SelectionChangedPayload {
        window_id: 5,
        has_selection: false,
        selection: None,
        visual_mode: None,
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(!ctx.state.window_selections.contains_key(&5));
}

#[tokio::test]
async fn test_handle_selection_changed_remote() {
    use reovim_protocol::v2::{Position, Selection, SelectionChangedPayload};

    let mut ctx = MockContext::new(1);
    ctx.state.add_remote_client(RemoteClient {
        client_id: 2,
        display_name: "other".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(1),
        mode: "VISUAL".to_string(),
        selection: None,
    });

    let notif = make_notif(Payload::SelectionChanged(SelectionChangedPayload {
        window_id: 10,
        has_selection: true,
        selection: Some(Selection {
            start: Some(Position { line: 0, column: 0 }),
            end: Some(Position {
                line: 5,
                column: 10,
            }),
        }),
        visual_mode: Some("line".to_string()),
        client_id: 2,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    let remote = &ctx.state.other_clients[&2];
    assert!(remote.selection.is_some());
    let sel = remote.selection.as_ref().unwrap();
    assert_eq!(sel.mode, "line");
}

#[tokio::test]
async fn test_handle_resize_request_for_us() {
    use reovim_protocol::v2::ResizeRequestPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::ResizeRequest(ResizeRequestPayload {
        width: 120,
        height: 40,
        target_client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert_eq!(ctx.state.width, 120);
    assert_eq!(ctx.state.height, 40);
}

#[tokio::test]
async fn test_handle_resize_request_zero_target_broadcast() {
    use reovim_protocol::v2::ResizeRequestPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::ResizeRequest(ResizeRequestPayload {
        width: 100,
        height: 30,
        target_client_id: 0, // Broadcast
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert_eq!(ctx.state.width, 100);
    assert_eq!(ctx.state.height, 30);
}

#[tokio::test]
async fn test_handle_resize_request_for_different_client() {
    use reovim_protocol::v2::ResizeRequestPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::ResizeRequest(ResizeRequestPayload {
        width: 120,
        height: 40,
        target_client_id: 99, // Different client
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::NoRedraw));
    // State should NOT be updated
    assert_eq!(ctx.state.width, 0);
    assert_eq!(ctx.state.height, 0);
}

#[tokio::test]
async fn test_handle_resize_request_zero_dimensions_ignored() {
    use reovim_protocol::v2::ResizeRequestPayload;

    let mut ctx = MockContext::new(1);
    ctx.state.width = 80;
    ctx.state.height = 24;

    let notif = make_notif(Payload::ResizeRequest(ResizeRequestPayload {
        width: 0,
        height: 0,
        target_client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Dimensions should not have changed
    assert_eq!(ctx.state.width, 80);
    assert_eq!(ctx.state.height, 24);
}

#[tokio::test]
async fn test_handle_capture_request_different_client() {
    use reovim_protocol::v2::CaptureRequestPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::CaptureRequest(CaptureRequestPayload {
        request_id: 42,
        format: "plain_text".to_string(),
        target_client_id: 99, // Different client
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::NoRedraw));
}

#[tokio::test]
async fn test_handle_capture_request_for_us_no_handler() {
    use reovim_protocol::v2::CaptureRequestPayload;

    let mut ctx = MockContext::new(1);
    // MockContext::on_capture_request returns None by default
    let notif = make_notif(Payload::CaptureRequest(CaptureRequestPayload {
        request_id: 42,
        format: "plain_text".to_string(),
        target_client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::NoRedraw));
}

#[tokio::test]
async fn test_handle_selection_no_selection_data_when_has_selection_true() {
    use reovim_protocol::v2::SelectionChangedPayload;

    let mut ctx = MockContext::new(1);
    // has_selection=true but selection=None => selection is None
    let notif = make_notif(Payload::SelectionChanged(SelectionChangedPayload {
        window_id: 5,
        has_selection: true,
        selection: None,
        visual_mode: Some("char".to_string()),
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // selection should be None because there's no selection data
    assert!(!ctx.state.window_selections.contains_key(&5));
}

#[tokio::test]
async fn test_handle_selection_missing_positions_use_defaults() {
    use reovim_protocol::v2::{Selection, SelectionChangedPayload};

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::SelectionChanged(SelectionChangedPayload {
        window_id: 5,
        has_selection: true,
        selection: Some(Selection {
            start: None, // No start position
            end: None,   // No end position
        }),
        visual_mode: None,
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    let sel = ctx.state.window_selections.get(&5).unwrap();
    // Default positions should be (0, 0)
    assert_eq!(sel.start.line, 0);
    assert_eq!(sel.start.column, 0);
    assert_eq!(sel.end.line, 0);
    assert_eq!(sel.end.column, 0);
    assert!(sel.mode.is_empty()); // visual_mode was None => default
}

// =========================================================================
// ExtensionUpdated generic dispatch tests (#468)
// =========================================================================

// Test extension stub — engine has ZERO knowledge of real extensions.
// This verifies the generic dispatch mechanism only.
use std::sync::{Arc, Mutex};

use reovim_client_driver::{ClientModuleError, ProbeResult, Version};

/// Shared handle to inspect what data a `StubExtension` received.
#[derive(Clone)]
struct StubHandle(Arc<Mutex<String>>);

impl StubHandle {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(String::new())))
    }

    fn received_data(&self) -> bool {
        !self.0.lock().unwrap().is_empty()
    }
}

struct StubExtension {
    ext_kind: &'static str,
    shared: StubHandle,
}

impl StubExtension {
    fn new(kind: &'static str) -> (Self, StubHandle) {
        let handle = StubHandle::new();
        (
            Self {
                ext_kind: kind,
                shared: handle.clone(),
            },
            handle,
        )
    }
}

impl ClientModule for StubExtension {
    fn id(&self) -> &'static str {
        self.ext_kind
    }
    fn kind(&self) -> &'static str {
        self.ext_kind
    }
    fn name(&self) -> &'static str {
        "Stub"
    }
    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }
    fn init(
        &mut self,
        _ctx: &reovim_client_driver::ModuleContext,
    ) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn on_notification(&mut self, data: &str) {
        *self.shared.0.lock().unwrap() = data.to_string();
    }
}

#[tokio::test]
async fn test_handle_extension_updated_dispatches_to_matching() {
    use reovim_protocol::v2::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let (whichkey, whichkey_h) = StubExtension::new("whichkey");
    let mut ctx =
        MockContext::new(1).with_extensions(vec![Box::new(cmdline), Box::new(whichkey)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#.to_string(),
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // The matching extension should have received data
    assert!(cmdline_h.received_data());
    // The other extension should NOT be affected
    assert!(!whichkey_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_remote_ignored() {
    use reovim_protocol::v2::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let mut ctx = MockContext::new(1).with_extensions(vec![Box::new(cmdline)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: r#"{"active":true}"#.to_string(),
        client_id: 99, // Different client
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Extension should NOT be updated (remote client)
    assert!(!cmdline_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_unknown_kind_ignored() {
    use reovim_protocol::v2::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let (whichkey, whichkey_h) = StubExtension::new("whichkey");
    let mut ctx =
        MockContext::new(1).with_extensions(vec![Box::new(cmdline), Box::new(whichkey)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "unknown_ext".to_string(),
        data: r#"{"active":true}"#.to_string(),
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Neither extension should be affected
    assert!(!cmdline_h.received_data());
    assert!(!whichkey_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_client_id_zero_is_local() {
    use reovim_protocol::v2::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let mut ctx = MockContext::new(1).with_extensions(vec![Box::new(cmdline)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: r#"{"active":true}"#.to_string(),
        client_id: 0, // 0 means local/unspecified
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Extension should be updated (client_id 0 treated as local)
    assert!(cmdline_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_invalid_json_no_panic() {
    use reovim_protocol::v2::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let mut ctx = MockContext::new(1).with_extensions(vec![Box::new(cmdline)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: "not valid json{{{".to_string(),
        client_id: 1,
    }));

    // Should not panic — extensions handle their own JSON parsing
    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Extension received data (even invalid JSON — it's up to the module to parse)
    assert!(cmdline_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_no_extensions() {
    use reovim_protocol::v2::ExtensionUpdatedPayload;

    // No extensions registered — should not panic
    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: r#"{"active":true}"#.to_string(),
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
}

#[tokio::test]
async fn test_notification_result_debug() {
    let redraw = NotificationResult::Redraw;
    let no_redraw = NotificationResult::NoRedraw;
    let stop = NotificationResult::Stop;
    assert!(format!("{redraw:?}").contains("Redraw"));
    assert!(format!("{no_redraw:?}").contains("NoRedraw"));
    assert!(format!("{stop:?}").contains("Stop"));
}
