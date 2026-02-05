//! Notification builder - converts `StateChanges` to gRPC notifications.
//!
//! This module bridges the session driver's change tracking to gRPC protocol notifications.
//! Following the data/presentation separation model:
//! - Server emits raw state changes (what changed)
//! - Client interprets and renders them (how to display)
//!
//! # Design
//!
//! `StateChanges` tracks WHAT changed during an operation.
//! This module converts those changes to gRPC `Notification` messages.
//!
//! # Per-Client State (#486)
//!
//! Notifications use per-client state (mode, cursor, selection) via `Session::client_state()`.
//! This ensures Client A receives notifications about Client A's state, not Client B's.
//!
//! # Example
//!
//! ```ignore
//! use reovim_server::grpc::notification_builder::build_notifications;
//!
//! let changes = runtime.take_changes();
//! let notifications = build_notifications(&changes, &session, client_id);
//!
//! for notification in notifications {
//!     session.emit_notification(notification);
//! }
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

use {
    reovim_driver_session::api::StateChanges,
    reovim_protocol::v2::{
        BufferListChangedPayload, BufferModifiedPayload, CursorMovedPayload, LayoutChangedPayload,
        ModeChangedPayload, Notification, OptionChangedPayload, Position, SelectionChangedPayload,
        ViewportUpdatedPayload, WindowInfo, WindowRect, notification,
    },
};

use crate::session::{ClientId, Session};

/// Get current timestamp in milliseconds since Unix epoch.
#[allow(clippy::cast_possible_truncation)]
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX_EPOCH")
        .as_millis() as u64
}

/// Build gRPC notifications from state changes.
///
/// Converts `StateChanges` accumulated during an operation into a list of
/// gRPC `Notification` messages for streaming to connected clients.
///
/// # Arguments
///
/// * `changes` - The state changes to convert
/// * `session` - Session for reading per-client and shared state (#486)
/// * `client_id` - Client ID that originated these changes (for multi-client filtering)
///
/// # Returns
///
/// A vector of notifications to emit. May be empty if no relevant changes.
#[must_use]
pub fn build_notifications(
    changes: &StateChanges,
    session: &Session,
    client_id: u64,
) -> Vec<Notification> {
    let mut notifications = Vec::new();
    let timestamp = current_timestamp_ms();

    // Mode changed notification
    // Phase 14 (#471): Include client_id for multi-client mode filtering
    // Phase #486: Uses per-client mode_stack
    if changes.mode_changed {
        notifications.push(build_mode_notification(session, timestamp, client_id));
    }

    // Cursor moved notification (per affected buffer)
    // Phase #486: Uses per-client cursor position
    if changes.cursor_moved {
        for buffer_id in &changes.affected_buffers {
            if let Some(notification) =
                build_cursor_notification(session, *buffer_id, timestamp, client_id)
            {
                notifications.push(notification);
            }
        }
    }

    // Buffer modified notifications (buffers are shared, not per-client)
    if changes.buffer_modified {
        for buffer_id in &changes.modified_buffers {
            notifications.push(build_buffer_modified_notification(*buffer_id, timestamp));
        }
    }

    // Buffer lifecycle notifications (buffers are shared)
    for buffer_id in &changes.buffers_created {
        notifications.push(build_buffer_list_notification("added", *buffer_id, timestamp));
    }
    for buffer_id in &changes.buffers_deleted {
        notifications.push(build_buffer_list_notification("removed", *buffer_id, timestamp));
    }

    // Window/layout changed notification
    // Layout uses shared state (compositor) but may use per-client focused window
    if changes.window_changed || changes.focus_changed {
        notifications.push(build_layout_notification(session, timestamp, client_id));
    }

    // Selection changed notifications
    // Phase #486: Uses per-client selection
    if changes.selection_changed {
        for buffer_id in &changes.affected_buffers {
            if let Some(notification) =
                build_selection_notification(session, *buffer_id, timestamp, client_id)
            {
                notifications.push(notification);
            }
        }
    }

    // Option changed notifications (options are shared)
    for opt_change in &changes.options_changed {
        notifications.push(build_option_notification(opt_change, timestamp));
    }

    // Viewport updated notifications (Phase 11.1)
    // Phase #486: Uses per-client viewport
    if changes.scroll_changed {
        for window_id in &changes.scrolled_windows {
            if let Some(notification) =
                build_viewport_notification(session, *window_id, timestamp, client_id)
            {
                notifications.push(notification);
            }
        }
    }

    notifications
}

/// Build a mode changed notification.
///
/// # Arguments
///
/// * `session` - Session for reading per-client mode (#486)
/// * `timestamp` - Notification timestamp
/// * `client_id` - Client that changed mode (Phase 14 #471: multi-client filtering)
#[allow(clippy::cast_possible_truncation)] // client_id u64→usize is safe on 64-bit
fn build_mode_notification(session: &Session, timestamp: u64, client_id: u64) -> Notification {
    // Phase #486: Read mode from per-client state
    let mode_name = session
        .client_state(ClientId::new(client_id as usize))
        .map_or_else(
            || {
                // Fallback to shared state if client not found
                tracing::warn!(%client_id, "Client not found for mode notification, using shared state");
                session.with_state_sync(|s| s.driver_session.mode_stack.current().name().to_string())
            },
            |s| s.mode_stack.current().name().to_string(),
        );

    // Derive display name (capitalize first letter, e.g., "normal" -> "NORMAL")
    let display_name = mode_name.to_uppercase();

    // Check if mode is an insert-like mode
    let is_insert = mode_name.contains("insert") || mode_name.contains("replace");

    Notification {
        event_type: "mode_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::ModeChanged(ModeChangedPayload {
            name: mode_name,
            display: display_name,
            is_insert,
            client_id,
        })),
    }
}

/// Build a cursor moved notification for a specific buffer.
///
/// Phase #486: Uses per-client cursor position from `session.client_state()`.
#[allow(clippy::cast_possible_truncation)] // client_id u64→usize is safe on 64-bit
fn build_cursor_notification(
    session: &Session,
    buffer_id: reovim_kernel::api::v1::BufferId,
    timestamp: u64,
    client_id: u64,
) -> Option<Notification> {
    // Phase #486: Get per-client windows for cursor position
    let editing_state = session.client_state(ClientId::new(client_id as usize))?;

    // Find the window displaying this buffer in per-client state
    let window = editing_state
        .windows
        .windows
        .iter()
        .find(|w| w.buffer_id == Some(buffer_id))?;

    Some(Notification {
        event_type: "cursor_moved".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::CursorMoved(CursorMovedPayload {
            window_id: window.id.as_usize() as u64,
            position: Some(Position {
                line: window.cursor.line as u64,
                column: window.cursor.column as u64,
            }),
            client_id,
        })),
    })
}

/// Build a buffer modified notification.
fn build_buffer_modified_notification(
    buffer_id: reovim_kernel::api::v1::BufferId,
    timestamp: u64,
) -> Notification {
    Notification {
        event_type: "buffer_modified".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::BufferModified(BufferModifiedPayload {
            buffer_id: buffer_id.as_usize() as u64,
            change: None, // Full content refresh for now
        })),
    }
}

/// Build a buffer list changed notification.
fn build_buffer_list_notification(
    action: &str,
    buffer_id: reovim_kernel::api::v1::BufferId,
    timestamp: u64,
) -> Notification {
    Notification {
        event_type: "buffer_list_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::BufferListChanged(BufferListChangedPayload {
            action: action.to_string(),
            buffer_id: buffer_id.as_usize() as u64,
        })),
    }
}

/// Build a layout changed notification.
///
/// Layout is session-wide (shared compositor), but focused window may be per-client.
///
/// # Arguments
///
/// * `session` - Session for reading shared layout and per-client focus (#486)
/// * `timestamp` - Notification timestamp
/// * `client_id` - Client for per-client focused window
#[allow(clippy::cast_possible_truncation, clippy::option_if_let_else)]
fn build_layout_notification(session: &Session, timestamp: u64, client_id: u64) -> Notification {
    // Phase #486: Try per-client focused window first, fallback to shared state
    let focused_id = session
        .client_state(ClientId::new(client_id as usize))
        .and_then(|s| s.windows.active_id())
        .or_else(|| {
            // Fallback to shared state
            session.with_state_sync(|state| {
                state
                    .driver_session
                    .shared
                    .compositor
                    .as_ref()
                    .and_then(|c| c.focused())
                    .or_else(|| state.driver_session.windows.active_id())
            })
        });

    // Build window info list from shared state (layout is session-wide)
    let windows: Vec<WindowInfo> = session.with_state_sync(|state| {
        if let Some(compositor) = &state.driver_session.shared.compositor {
            // Get placements from compositor
            let (width, height) = state.driver_session.terminal_size();
            let screen = reovim_driver_display::Rect::new(0, 0, width, height);
            let composite = compositor.composite(screen);

            composite
                .placements
                .iter()
                .map(|p| {
                    // Phase #479: buffer_id is Option to eliminate ID ambiguity
                    let buffer_id = state
                        .driver_session
                        .windows
                        .get(p.window_id)
                        .and_then(|w| w.buffer_id)
                        .map(|id| id.as_usize() as u64);

                    WindowInfo {
                        window_id: p.window_id.as_usize() as u64,
                        buffer_id,
                        rect: Some(WindowRect {
                            x: u64::from(p.bounds.x),
                            y: u64::from(p.bounds.y),
                            width: u64::from(p.bounds.width),
                            height: u64::from(p.bounds.height),
                        }),
                        focused: focused_id == Some(p.window_id),
                    }
                })
                .collect()
        } else {
            // Fallback: create window info from driver session windows
            state
                .driver_session
                .windows
                .windows
                .iter()
                .map(|w| WindowInfo {
                    window_id: w.id.as_usize() as u64,
                    // Phase #479: buffer_id is now Option<u64>
                    buffer_id: w.buffer_id.map(|id| id.as_usize() as u64),
                    rect: Some(WindowRect {
                        x: 0,
                        y: 0,
                        width: 80, // Default size
                        height: 24,
                    }),
                    focused: focused_id == Some(w.id),
                })
                .collect()
        }
    });

    Notification {
        event_type: "layout_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::LayoutChanged(LayoutChangedPayload {
            // Phase #479: focused_window_id is now Option<u64>
            focused_window_id: focused_id.map(|id| id.as_usize() as u64),
            windows,
        })),
    }
}

/// Build a selection changed notification.
///
/// Phase #486: Uses per-client selection from `session.client_state()`.
///
/// # Arguments
///
/// * `session` - Session for reading per-client selection
/// * `buffer_id` - Buffer to find selection for
/// * `timestamp` - Notification timestamp
/// * `client_id` - Client whose selection to read
#[allow(clippy::cast_possible_truncation, clippy::significant_drop_tightening)]
fn build_selection_notification(
    session: &Session,
    buffer_id: reovim_kernel::api::v1::BufferId,
    timestamp: u64,
    client_id: u64,
) -> Option<Notification> {
    // Phase #486: Get per-client windows for selection
    let editing_state = session.client_state(ClientId::new(client_id as usize))?;

    // Find window displaying this buffer in per-client state
    let window = editing_state
        .windows
        .windows
        .iter()
        .find(|w| w.buffer_id == Some(buffer_id))?;

    // Phase 8 (#465): Selection now lives in Window with explicit start/end.
    // Read directly from window.selection - no cursor computation needed.
    let (has_selection, selection, visual_mode) =
        window
            .selection
            .as_ref()
            .map_or((false, None, None), |sel| {
                use reovim_driver_session::SelectionMode;

                let mode_str = match sel.mode {
                    SelectionMode::Character => "char",
                    SelectionMode::Line => "line",
                    SelectionMode::Block => "block",
                };

                (
                    true,
                    Some(reovim_protocol::v2::Selection {
                        start: Some(Position {
                            line: sel.start.line as u64,
                            column: sel.start.column as u64,
                        }),
                        end: Some(Position {
                            line: sel.end.line as u64,
                            column: sel.end.column as u64,
                        }),
                    }),
                    Some(mode_str.to_string()),
                )
            });

    Some(Notification {
        event_type: "selection_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::SelectionChanged(SelectionChangedPayload {
            window_id: window.id.as_usize() as u64,
            has_selection,
            selection,
            visual_mode,
            client_id,
        })),
    })
}

/// Build a viewport updated notification (Phase 11.1).
///
/// Sent when scroll position changes in a window, enabling clients to
/// apply incremental updates without re-fetching layout.
///
/// Phase #486: Uses per-client viewport scroll position.
///
/// # Arguments
///
/// * `session` - Session for reading per-client viewport
/// * `window_id` - Window whose viewport changed
/// * `timestamp` - Notification timestamp
/// * `client_id` - Client whose viewport to read
#[allow(clippy::cast_possible_truncation)]
fn build_viewport_notification(
    session: &Session,
    window_id: reovim_kernel::api::v1::WindowId,
    timestamp: u64,
    client_id: u64,
) -> Option<Notification> {
    // Phase #486: Get per-client windows for viewport
    let editing_state = session.client_state(ClientId::new(client_id as usize))?;

    // Find the window in per-client state
    let window = editing_state.windows.get(window_id)?;

    Some(Notification {
        event_type: "viewport_updated".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::ViewportUpdated(ViewportUpdatedPayload {
            viewport_id: window_id.as_usize() as u64,
            top_line: Some(window.viewport.scroll_top as u32),
            left_col: Some(window.viewport.scroll_left as u32),
            cursor_line: Some(window.cursor.line as u32),
            cursor_col: Some(window.cursor.column as u32),
        })),
    })
}

/// Build an option changed notification.
fn build_option_notification(
    opt_change: &reovim_driver_session::api::OptionChange,
    timestamp: u64,
) -> Notification {
    use {reovim_kernel::api::v1::OptionValue, reovim_protocol::v2::option_changed_payload::Value};

    let value = match &opt_change.value {
        OptionValue::Bool(b) => Some(Value::BoolValue(*b)),
        OptionValue::Integer(i) => Some(Value::IntValue(*i)),
        OptionValue::String(s) => Some(Value::StringValue(s.clone())),
        OptionValue::Choice { value, .. } => Some(Value::StringValue(value.clone())),
    };

    Notification {
        event_type: "option_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::OptionChanged(OptionChangedPayload {
            name: opt_change.name.clone(),
            value,
        })),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::session::SessionId};

    #[test]
    fn test_current_timestamp_ms() {
        let ts = current_timestamp_ms();
        // Should be a reasonable recent timestamp (after 2024)
        assert!(ts > 1_700_000_000_000);
    }

    #[test]
    fn test_build_notifications_empty_changes() {
        let changes = StateChanges::new();
        let session = Session::new(SessionId::new("test"));
        // Use client_id 0 - no client registered, so per-client lookups return None
        // and fallback to shared state (or return empty notifications)
        let notifications = build_notifications(&changes, &session, 0);
        assert!(notifications.is_empty());
    }

    #[test]
    fn test_build_notifications_mode_changed() {
        let mut changes = StateChanges::new();
        changes.record_mode_change();

        let session = Session::new(SessionId::new("test"));
        // Use client_id 0 - fallback to shared state for mode
        let notifications = build_notifications(&changes, &session, 0);

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].event_type, "mode_changed");
    }

    #[test]
    fn test_mode_notification_fallback_uses_shared_state() {
        // Phase #486: When client_id is not found, should fallback to shared state
        let session = Session::new(SessionId::new("fallback-test"));

        // Build mode notification for non-existent client
        let notification = build_mode_notification(&session, 12345, 999);

        // Should still produce a valid notification using shared state
        assert_eq!(notification.event_type, "mode_changed");
        if let Some(notification::Payload::ModeChanged(payload)) = notification.payload {
            // Default shared state mode is "normal"
            assert_eq!(payload.name, "normal");
            assert_eq!(payload.client_id, 999);
        } else {
            panic!("Expected ModeChangedPayload");
        }
    }

    #[test]
    fn test_cursor_notification_returns_none_for_unknown_client() {
        // Phase #486: When client_id is not found, cursor notification returns None
        let session = Session::new(SessionId::new("cursor-test"));
        let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);

        // Non-existent client should return None (no cursor to report)
        let notification = build_cursor_notification(&session, buffer_id, 12345, 999);
        assert!(notification.is_none());
    }

    #[test]
    fn test_selection_notification_returns_none_for_unknown_client() {
        // Phase #486: When client_id is not found, selection notification returns None
        let session = Session::new(SessionId::new("selection-test"));
        let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);

        // Non-existent client should return None (no selection to report)
        let notification = build_selection_notification(&session, buffer_id, 12345, 999);
        assert!(notification.is_none());
    }

    #[test]
    fn test_viewport_notification_returns_none_for_unknown_client() {
        // Phase #486: When client_id is not found, viewport notification returns None
        let session = Session::new(SessionId::new("viewport-test"));
        let window_id = reovim_kernel::api::v1::WindowId::from_raw(1);

        // Non-existent client should return None (no viewport to report)
        let notification = build_viewport_notification(&session, window_id, 12345, 999);
        assert!(notification.is_none());
    }
}
