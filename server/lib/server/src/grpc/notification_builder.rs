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
//! # Example
//!
//! ```ignore
//! use reovim_server::grpc::notification_builder::build_notifications;
//!
//! let changes = runtime.take_changes();
//! let notifications = build_notifications(&changes, &session_state);
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

use crate::session::SessionState;

/// Get current timestamp in milliseconds since Unix epoch.
#[allow(clippy::cast_possible_truncation)]
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Build gRPC notifications from state changes.
///
/// Converts `StateChanges` accumulated during an operation into a list of
/// gRPC `Notification` messages for streaming to connected clients.
///
/// # Arguments
///
/// * `changes` - The state changes to convert
/// * `state` - Current session state for reading values
///
/// # Returns
///
/// A vector of notifications to emit. May be empty if no relevant changes.
#[must_use]
pub fn build_notifications(changes: &StateChanges, state: &SessionState) -> Vec<Notification> {
    let mut notifications = Vec::new();
    let timestamp = current_timestamp_ms();

    // Mode changed notification
    if changes.mode_changed {
        notifications.push(build_mode_notification(state, timestamp));
    }

    // Cursor moved notification (per affected buffer)
    if changes.cursor_moved {
        for buffer_id in &changes.affected_buffers {
            if let Some(notification) = build_cursor_notification(state, *buffer_id, timestamp) {
                notifications.push(notification);
            }
        }
    }

    // Buffer modified notifications
    if changes.buffer_modified {
        for buffer_id in &changes.modified_buffers {
            notifications.push(build_buffer_modified_notification(*buffer_id, timestamp));
        }
    }

    // Buffer lifecycle notifications
    for buffer_id in &changes.buffers_created {
        notifications.push(build_buffer_list_notification("added", *buffer_id, timestamp));
    }
    for buffer_id in &changes.buffers_deleted {
        notifications.push(build_buffer_list_notification("removed", *buffer_id, timestamp));
    }

    // Window/layout changed notification
    if changes.window_changed || changes.focus_changed {
        notifications.push(build_layout_notification(state, timestamp));
    }

    // Selection changed notifications
    if changes.selection_changed {
        for buffer_id in &changes.affected_buffers {
            if let Some(notification) = build_selection_notification(state, *buffer_id, timestamp) {
                notifications.push(notification);
            }
        }
    }

    // Option changed notifications
    for opt_change in &changes.options_changed {
        notifications.push(build_option_notification(opt_change, timestamp));
    }

    // Viewport updated notifications (Phase 11.1)
    if changes.scroll_changed {
        for window_id in &changes.scrolled_windows {
            if let Some(notification) = build_viewport_notification(state, *window_id, timestamp) {
                notifications.push(notification);
            }
        }
    }

    notifications
}

/// Build a mode changed notification.
fn build_mode_notification(state: &SessionState, timestamp: u64) -> Notification {
    let mode = state.driver_session.mode_stack.current();
    let mode_name = mode.name();

    // Derive display name (capitalize first letter, e.g., "normal" -> "NORMAL")
    let display_name = mode_name.to_uppercase();

    // Check if mode is an insert-like mode
    let is_insert = mode_name.contains("insert") || mode_name.contains("replace");

    Notification {
        event_type: "mode_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::ModeChanged(ModeChangedPayload {
            name: mode_name.to_string(),
            display: display_name,
            is_insert,
        })),
    }
}

/// Build a cursor moved notification for a specific buffer.
fn build_cursor_notification(
    state: &SessionState,
    buffer_id: reovim_kernel::api::v1::BufferId,
    timestamp: u64,
) -> Option<Notification> {
    // Find the window displaying this buffer
    let window = state
        .driver_session
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
#[allow(clippy::cast_possible_truncation, clippy::option_if_let_else)]
fn build_layout_notification(state: &SessionState, timestamp: u64) -> Notification {
    // Get focused window from compositor or fallback to driver session
    let focused_id = state
        .driver_session
        .compositor
        .as_ref()
        .and_then(|c| c.focused())
        .or_else(|| state.driver_session.windows.active_id());

    // Build window info list
    let windows: Vec<WindowInfo> = if let Some(compositor) = &state.driver_session.compositor {
        // Get placements from compositor
        let (width, height) = state.driver_session.terminal_size();
        let screen = reovim_driver_display::Rect::new(0, 0, width, height);
        let composite = compositor.composite(screen);

        composite
            .placements
            .iter()
            .map(|p| {
                let buffer_id = state
                    .driver_session
                    .windows
                    .get(p.window_id)
                    .and_then(|w| w.buffer_id)
                    .map_or(0, |id| id.as_usize() as u64);

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
                buffer_id: w.buffer_id.map_or(0, |id| id.as_usize() as u64),
                rect: Some(WindowRect {
                    x: 0,
                    y: 0,
                    width: 80, // Default size
                    height: 24,
                }),
                focused: focused_id == Some(w.id),
            })
            .collect()
    };

    Notification {
        event_type: "layout_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::LayoutChanged(LayoutChangedPayload {
            focused_window_id: focused_id.map_or(0, |id| id.as_usize() as u64),
            windows,
        })),
    }
}

/// Build a selection changed notification.
#[allow(clippy::cast_possible_truncation, clippy::significant_drop_tightening)]
fn build_selection_notification(
    state: &SessionState,
    buffer_id: reovim_kernel::api::v1::BufferId,
    timestamp: u64,
) -> Option<Notification> {
    // Find the window displaying this buffer
    let window = state
        .driver_session
        .windows
        .windows
        .iter()
        .find(|w| w.buffer_id == Some(buffer_id))?;

    // Get selection state from buffer - extract data while holding lock
    let (has_selection, selection, visual_mode) =
        state
            .app
            .kernel
            .buffers
            .get(buffer_id)
            .map_or((false, None, None), |buf_arc| {
                let buf = buf_arc.read();
                let sel = buf.selection();
                if sel.is_active() {
                    let cursor = buf.position();
                    let (start, end) = if sel.anchor <= cursor {
                        (sel.anchor, cursor)
                    } else {
                        (cursor, sel.anchor)
                    };

                    let mode_str = match sel.mode() {
                        reovim_kernel::api::v1::SelectionMode::Character => "char",
                        reovim_kernel::api::v1::SelectionMode::Line => "line",
                        reovim_kernel::api::v1::SelectionMode::Block => "block",
                    };

                    (
                        true,
                        Some(reovim_protocol::v2::Selection {
                            start: Some(Position {
                                line: start.line as u64,
                                column: start.column as u64,
                            }),
                            end: Some(Position {
                                line: end.line as u64,
                                column: end.column as u64,
                            }),
                        }),
                        Some(mode_str.to_string()),
                    )
                } else {
                    (false, None, None)
                }
            });

    Some(Notification {
        event_type: "selection_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::SelectionChanged(SelectionChangedPayload {
            window_id: window.id.as_usize() as u64,
            has_selection,
            selection,
            visual_mode,
        })),
    })
}

/// Build a viewport updated notification (Phase 11.1).
///
/// Sent when scroll position changes in a window, enabling clients to
/// apply incremental updates without re-fetching layout.
#[allow(clippy::cast_possible_truncation)]
fn build_viewport_notification(
    state: &SessionState,
    window_id: reovim_kernel::api::v1::WindowId,
    timestamp: u64,
) -> Option<Notification> {
    // Find the window
    let window = state.driver_session.windows.get(window_id)?;

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
    use super::*;

    #[test]
    fn test_current_timestamp_ms() {
        let ts = current_timestamp_ms();
        // Should be a reasonable recent timestamp (after 2024)
        assert!(ts > 1_700_000_000_000);
    }

    #[test]
    fn test_build_notifications_empty_changes() {
        let changes = StateChanges::new();
        let state = SessionState::default();
        let notifications = build_notifications(&changes, &state);
        assert!(notifications.is_empty());
    }

    #[test]
    fn test_build_notifications_mode_changed() {
        let mut changes = StateChanges::new();
        changes.record_mode_change();

        let state = SessionState::default();
        let notifications = build_notifications(&changes, &state);

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].event_type, "mode_changed");
    }
}
