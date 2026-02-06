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

            // Invalidate cache first
            ctx.state_mut().buffer_cache.remove(&buffer_id);

            // Refetch buffer content
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
            #[allow(clippy::cast_possible_truncation)]
            let width = resize_req.width as u16;
            #[allow(clippy::cast_possible_truncation)]
            let height = resize_req.height as u16;

            if width > 0 && height > 0 {
                tracing::debug!(width, height, "Resize request from CLI");
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
}
