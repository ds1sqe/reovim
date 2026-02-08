//! Unified notification handler for interactive and headless TUI.
//!
//! This module provides a single source of truth for handling server
//! notifications. `TuiApp<O: TuiOutput>` implements
//! the `NotificationContext` trait and uses `handle_notification()`.
//!
//! # Design
//!
//! The `NotificationContext` trait provides:
//! - Access to shared state (`state_mut()`)
//! - Access to gRPC client (`client_mut()`)
//! - Optional hooks for interactive-specific behavior (`on_buffer_modified()`)
//!
//! Default hook implementations are no-ops, allowing headless TUI to skip
//! syntax highlighting refresh while interactive TUI provides real implementations.

use reovim_protocol::v2::{
    Notification, notification::Payload, option_changed_payload::Value as OptionValue,
};

use crate::{
    CursorPosition, RemoteClient, SelectionState, TuiCoreState,
    core_helpers::apply_layout_notification,
    grpc_client::{TuiGrpcClient, TuiGrpcError},
};

/// Context trait for notification handling.
///
/// Provides access to shared state and client, plus optional hooks
/// for TUI-specific behavior (syntax refresh, theme changes, etc.).
pub trait NotificationContext {
    /// Get mutable reference to the core state.
    fn state_mut(&mut self) -> &mut TuiCoreState;

    /// Get mutable reference to the gRPC client.
    fn client_mut(&mut self) -> &mut TuiGrpcClient;

    /// Called when a buffer is modified.
    ///
    /// Default implementation does nothing. Interactive TUI overrides
    /// to refresh syntax tokens.
    #[allow(unused_variables)]
    fn on_buffer_modified(&mut self, buffer_id: u64) {
        // Default: no-op
    }

    /// Called when an option changes.
    ///
    /// Default implementation does nothing. Interactive TUI overrides
    /// to handle colorscheme changes.
    #[allow(unused_variables)]
    fn on_option_changed(&mut self, name: &str, value: Option<OptionValue>) {
        // Default: no-op
    }

    /// Called when a capture request is received.
    ///
    /// Returns the frame content if this TUI should handle the request,
    /// or None if the request is for a different client.
    ///
    /// Default implementation returns None. Both TUIs override this
    /// to provide capture handling.
    #[allow(unused_variables)]
    fn on_capture_request(
        &mut self,
        request_id: u64,
        format: &str,
        target_client_id: u64,
    ) -> Option<String> {
        None
    }

    /// Called when a resize request is received.
    ///
    /// Default implementation does nothing. Headless TUI overrides
    /// to resize its frame buffer.
    #[allow(unused_variables)]
    fn on_resize(&mut self, width: u16, height: u16) {
        // Default: no-op (interactive TUI uses screen.resize() separately)
    }
}

/// Result of notification handling.
#[derive(Debug)]
pub enum NotificationResult {
    /// Notification handled successfully, redraw needed.
    Redraw,
    /// Notification handled, no redraw needed.
    NoRedraw,
    /// Client should stop (detach received).
    Stop,
}

/// Handle a server notification using the provided context.
///
/// This is the single source of truth for notification handling. Both
/// interactive and headless TUI call this function.
///
/// # Arguments
///
/// * `ctx` - Notification context (interactive or headless TUI)
/// * `notif` - Notification from server
///
/// # Returns
///
/// Returns the result indicating what action to take.
///
/// # Errors
///
/// Returns an error if buffer refetch fails.
#[allow(clippy::too_many_lines)]
pub async fn handle_notification<C: NotificationContext>(
    ctx: &mut C,
    notif: Notification,
) -> Result<NotificationResult, TuiGrpcError> {
    let Some(payload) = notif.payload else {
        return Ok(NotificationResult::NoRedraw);
    };

    match payload {
        Payload::ModeChanged(mode) => {
            let state = ctx.state_mut();
            let is_local = mode.client_id == state.my_client_id;

            if is_local {
                state.mode_name = mode.name;
                state.mode_display = mode.display;
                state.is_insert_mode = mode.is_insert;
            } else {
                // Remote mode update
                if let Some(remote) = state.other_clients.get_mut(&mode.client_id) {
                    remote.mode.clone_from(&mode.display);
                }
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::CursorMoved(cursor) => {
            let state = ctx.state_mut();
            let is_local = cursor.client_id == state.my_client_id;

            // Debug: trace cursor notifications to diagnose position bugs
            tracing::debug!(
                notif_client_id = cursor.client_id,
                my_client_id = state.my_client_id,
                is_local,
                window_id = cursor.window_id,
                position = ?cursor.position,
                "CursorMoved notification"
            );

            if let Some(pos) = cursor.position {
                if is_local {
                    state.update_local_cursor(cursor.window_id, pos.line, pos.column);
                } else {
                    state.update_remote_cursor(cursor.client_id, pos.line, pos.column);
                }
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::BufferModified(buf) => {
            let buffer_id = buf.buffer_id;

            // Refetch buffer content (keep stale data until refetch completes
            // to avoid race condition where capture reads empty cache)
            match ctx
                .client_mut()
                .get_buffer_content(Some(buffer_id), None, None)
                .await
            {
                Ok(content) => {
                    ctx.state_mut()
                        .buffer_cache
                        .insert(buffer_id, content.lines);
                }
                Err(e) => {
                    tracing::warn!(buffer_id, error = %e, "Failed to refetch buffer");
                }
            }

            // Notify context for optional syntax refresh
            ctx.on_buffer_modified(buffer_id);

            Ok(NotificationResult::Redraw)
        }

        Payload::LayoutChanged(layout) => {
            let state = ctx.state_mut();
            apply_layout_notification(state, layout.focused_window_id, layout.windows);
            Ok(NotificationResult::Redraw)
        }

        Payload::RenderComplete(_) => Ok(NotificationResult::Redraw),

        Payload::Detach(detach) => {
            tracing::info!("Server requested detach: {}", detach.reason);
            Ok(NotificationResult::Stop)
        }

        Payload::OptionChanged(opt) => {
            let value = opt.value.clone();
            ctx.on_option_changed(&opt.name, value);
            Ok(NotificationResult::Redraw)
        }

        Payload::PresenceJoined(p) => {
            if let Some(client) = p.client {
                let state = ctx.state_mut();
                if client.client_id != state.my_client_id {
                    tracing::info!(
                        client_id = client.client_id,
                        display_name = %client.display_name,
                        buffer_id = ?client.buffer_id,
                        "PresenceJoined: Adding remote client"
                    );
                    state.add_remote_client(RemoteClient {
                        client_id: client.client_id,
                        display_name: client.display_name,
                        cursor_line: 0, // Updated via CursorMoved
                        cursor_col: 0,
                        buffer_id: client.buffer_id,
                        mode: client.mode,
                        selection: None, // Updated via SelectionChanged
                    });
                }
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::PresenceUpdated(p) => {
            if let Some(client) = p.client {
                let state = ctx.state_mut();
                if client.client_id != state.my_client_id {
                    let old = state.other_clients.get(&client.client_id);

                    // Check if buffer changed - if so, reset cursor to (0,0)
                    // The old cursor position doesn't make sense in a new buffer
                    let buffer_changed = old.is_none_or(|o| o.buffer_id != client.buffer_id);

                    let (cursor_line, cursor_col) = if buffer_changed {
                        // Reset cursor for new buffer context
                        // Server will send CursorMoved with actual position
                        (0, 0)
                    } else {
                        // Preserve existing cursor position within same buffer
                        (old.map_or(0, |c| c.cursor_line), old.map_or(0, |c| c.cursor_col))
                    };

                    let selection = if buffer_changed {
                        // Also clear selection on buffer switch
                        None
                    } else {
                        old.and_then(|c| c.selection.clone())
                    };

                    state.other_clients.insert(
                        client.client_id,
                        RemoteClient {
                            client_id: client.client_id,
                            display_name: client.display_name,
                            cursor_line,
                            cursor_col,
                            buffer_id: client.buffer_id,
                            mode: client.mode,
                            selection,
                        },
                    );
                }
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::PresenceLeft(p) => {
            let state = ctx.state_mut();
            state.remove_remote_client(p.client_id);
            Ok(NotificationResult::Redraw)
        }

        Payload::SelectionChanged(sel) => {
            let state = ctx.state_mut();
            let is_local = sel.client_id == state.my_client_id;

            let selection = if sel.has_selection {
                sel.selection.map(|s| {
                    let start = s
                        .start
                        .map_or_else(CursorPosition::default, |p| CursorPosition {
                            line: p.line,
                            column: p.column,
                        });
                    let end = s
                        .end
                        .map_or_else(CursorPosition::default, |p| CursorPosition {
                            line: p.line,
                            column: p.column,
                        });
                    SelectionState {
                        start,
                        end,
                        mode: sel.visual_mode.clone().unwrap_or_default(),
                    }
                })
            } else {
                None
            };

            if is_local {
                state.update_local_selection(sel.window_id, selection);
            } else {
                state.update_remote_selection(sel.client_id, selection);
            }

            Ok(NotificationResult::Redraw)
        }

        Payload::ResizeRequest(resize_req) => {
            // Only handle resize targeted at us (0 = no target for backward compat)
            let my_client_id = ctx.state_mut().my_client_id;
            if resize_req.target_client_id != 0 && resize_req.target_client_id != my_client_id {
                tracing::trace!(
                    target_client_id = resize_req.target_client_id,
                    my_client_id,
                    "Ignoring resize request for different client"
                );
                return Ok(NotificationResult::NoRedraw);
            }

            #[allow(clippy::cast_possible_truncation)]
            let width = resize_req.width as u16;
            #[allow(clippy::cast_possible_truncation)]
            let height = resize_req.height as u16;

            if width > 0 && height > 0 {
                tracing::debug!(width, height, "Resize request");
                let state = ctx.state_mut();
                state.width = width;
                state.height = height;
                // Notify context for frame buffer resize (headless TUI)
                ctx.on_resize(width, height);
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::CaptureRequest(capture_req) => {
            // Check if this request is for us
            let my_client_id = ctx.state_mut().my_client_id;
            if capture_req.target_client_id != my_client_id {
                tracing::trace!(
                    target_client_id = capture_req.target_client_id,
                    my_client_id,
                    "Ignoring capture request for different client"
                );
                return Ok(NotificationResult::NoRedraw);
            }

            // Let context handle capture
            if let Some(content) = ctx.on_capture_request(
                capture_req.request_id,
                &capture_req.format,
                capture_req.target_client_id,
            ) {
                // Submit capture response - scope client reference tightly
                let (width, height) = {
                    let state = ctx.state_mut();
                    (state.width, state.height)
                };
                let result = {
                    let client = ctx.client_mut();
                    client
                        .submit_capture_response(
                            capture_req.request_id,
                            u64::from(width),
                            u64::from(height),
                            &capture_req.format,
                            content,
                        )
                        .await
                };

                match result {
                    Ok(reply) => {
                        tracing::debug!(
                            request_id = capture_req.request_id,
                            ok = reply.ok,
                            "Submitted capture response"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            request_id = capture_req.request_id,
                            error = %e,
                            "Failed to submit capture response"
                        );
                    }
                }
            }
            Ok(NotificationResult::NoRedraw)
        }

        _ => {
            // Other notifications - trigger redraw
            Ok(NotificationResult::Redraw)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock context for testing
    struct MockContext {
        state: TuiCoreState,
        buffer_modified_calls: Vec<u64>,
        option_changed_calls: Vec<String>,
    }

    impl MockContext {
        fn new(client_id: u64) -> Self {
            Self {
                state: TuiCoreState::new(client_id),
                buffer_modified_calls: Vec::new(),
                option_changed_calls: Vec::new(),
            }
        }
    }

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
        }
        impl NotificationContext for MinimalContext {
            fn state_mut(&mut self) -> &mut TuiCoreState {
                &mut self.state
            }
            fn client_mut(&mut self) -> &mut crate::grpc_client::TuiGrpcClient {
                unimplemented!()
            }
        }

        let mut ctx = MinimalContext {
            state: TuiCoreState::new(1),
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
        assert!(ctx.state.is_insert_mode);
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
        let notif = make_notif(Payload::LayoutChanged(LayoutChangedPayload {
            focused_window_id: Some(3),
            windows: vec![
                WindowInfo {
                    window_id: 3,
                    buffer_id: Some(100),
                    rect: None,
                    focused: true,
                },
                WindowInfo {
                    window_id: 4,
                    buffer_id: Some(200),
                    rect: None,
                    focused: false,
                },
            ],
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

    #[tokio::test]
    async fn test_notification_result_debug() {
        let redraw = NotificationResult::Redraw;
        let no_redraw = NotificationResult::NoRedraw;
        let stop = NotificationResult::Stop;
        assert!(format!("{redraw:?}").contains("Redraw"));
        assert!(format!("{no_redraw:?}").contains("NoRedraw"));
        assert!(format!("{stop:?}").contains("Stop"));
    }
}
