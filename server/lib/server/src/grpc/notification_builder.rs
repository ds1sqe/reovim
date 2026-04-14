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
//! let notifications = build_notifications(&changes, &session, client_id, None);
//!
//! for notification in notifications {
//!     session.emit_notification(notification);
//! }
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

use {
    reovim_driver_text_session::api::StateChanges,
    reovim_protocol::v2::{
        BufferListChangedPayload, BufferModifiedPayload, CursorMovedPayload,
        ExtensionUpdatedPayload, LayoutChangedPayload, ModeChangedPayload, Notification,
        OptionChangedPayload, Position, SelectionChangedPayload, TabPageInfo,
        ViewportUpdatedPayload, WindowInfo, WindowRect, notification,
    },
    reovim_subsys_session::bridges::BridgeRegistry,
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
/// * `bridges` - Optional bridge registry for extension notifications (#514)
///
/// # Returns
///
/// A vector of notifications to emit. May be empty if no relevant changes.
#[must_use]
pub fn build_notifications(
    changes: &StateChanges,
    session: &Session,
    client_id: u64,
    bridges: Option<&BridgeRegistry>,
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

    // Presence updated notifications (#471)
    if changes.presence_changed {
        for &cid in &changes.presence_updates {
            if let Some(presence) = session.presence().get(ClientId::new(cid)) {
                notifications.push(super::presence::build_presence_updated_notification(&presence));
            }
        }
    }

    // Extension state updated notifications (#514)
    if changes.extension_changed
        && let Some(registry) = bridges
    {
        for kind in &changes.extensions_updated {
            if let Some(notification) =
                build_extension_notification(kind, session, timestamp, client_id, registry)
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
    // Phase #486, #491: Read mode from per-client state
    let mode_name = session
        .clients()
        .client_state(ClientId::new(client_id as usize))
        .map_or_else(
            || {
                // Fallback to home_mode if client not found (#491)
                tracing::warn!(%client_id, "Client not found for mode notification, using home_mode");
                session.with_state_sync(|s| s.home_mode().name().to_string())
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
    let editing_state = session
        .clients()
        .client_state(ClientId::new(client_id as usize))?;

    // Find the ACTIVE window displaying this buffer in per-client state.
    // When multiple windows share the same buffer (splits), prefer the
    // active window so cursor notifications target the correct pane.
    let active_id = editing_state.windows.active_id();
    let window = editing_state
        .windows
        .windows
        .iter()
        .find(|w| w.buffer_id == Some(buffer_id) && Some(w.id) == active_id)
        .or_else(|| {
            editing_state
                .windows
                .windows
                .iter()
                .find(|w| w.buffer_id == Some(buffer_id))
        })?;

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
    // #474: Use per-client compositor and windows (same ID namespace).
    // Each client owns a cloned compositor — window IDs from compositor.composite()
    // match the per-client WindowLayout IDs, fixing the cross-namespace mismatch.
    let editing_state = session
        .clients()
        .client_state(ClientId::new(client_id as usize));

    let focused_id = editing_state.as_ref().and_then(|s| s.windows.active_id());

    // Build window info list from per-client compositor (#474)
    let windows: Vec<WindowInfo> = if let Some(ref state) = editing_state {
        if let Some(ref compositor) = state.compositor {
            // Per-client compositor: IDs match per-client windows
            // Per-client terminal_size and active_buffer (#471)
            let (tw, th) = state.terminal_size;
            let screen = reovim_subsys_layout::Rect::new(0, 0, tw, th);
            let composite = compositor.composite(screen);

            let active_buffer = state.active_buffer;

            composite
                .placements
                .iter()
                .map(|p| {
                    // #474: w.get(p.window_id) now works — both use same ID namespace
                    let buffer_id = state
                        .windows
                        .get(p.window_id)
                        .and_then(|w| w.buffer_id)
                        .or(active_buffer)
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
                        // #400: Always send explicit opacity from compositor placement
                        opacity: Some(p.opacity),
                    }
                })
                .collect()
        } else {
            // No compositor — use per-client windows with default geometry
            state
                .windows
                .windows
                .iter()
                .map(|w| WindowInfo {
                    window_id: w.id.as_usize() as u64,
                    buffer_id: w.buffer_id.map(|id| id.as_usize() as u64),
                    rect: Some(WindowRect {
                        x: 0,
                        y: 0,
                        width: 80,
                        height: 24,
                    }),
                    focused: focused_id == Some(w.id),
                    opacity: None, // No compositor → default opaque
                })
                .collect()
        }
    } else {
        Vec::new()
    };

    // #401 Phase 5: Populate tab info from per-client TabPageSet
    let (active_tab_id, tabs_info) = if let Some(ref state) = editing_state {
        let tab_set = &state.tabs;
        let active_id = Some(tab_set.active_tab_id().as_usize() as u64);
        let info: Vec<TabPageInfo> = tab_set
            .tab_info()
            .into_iter()
            .map(|(id, label, is_active)| TabPageInfo {
                tab_id: id.as_usize() as u64,
                label: label.to_string(),
                active: is_active,
            })
            .collect();
        (active_id, info)
    } else {
        (None, Vec::new())
    };

    Notification {
        event_type: "layout_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::LayoutChanged(LayoutChangedPayload {
            focused_window_id: focused_id.map(|id| id.as_usize() as u64),
            windows,
            client_id,
            active_tab_id,
            tabs: tabs_info,
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
    let editing_state = session
        .clients()
        .client_state(ClientId::new(client_id as usize))?;

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
                use reovim_driver_text_session::SelectionMode;

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
    let editing_state = session
        .clients()
        .client_state(ClientId::new(client_id as usize))?;

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
    opt_change: &reovim_driver_text_session::api::OptionChange,
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

/// Build an extension state updated notification (#514).
///
/// Looks up the bridge by kind, gets the snapshot from the client's `ExtensionMap`,
/// and returns a notification with the JSON data.
///
/// Returns `None` if the bridge is unknown or the extension has no state.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn build_extension_notification(
    kind: &str,
    session: &Session,
    timestamp: u64,
    client_id: u64,
    bridges: &BridgeRegistry,
) -> Option<Notification> {
    use reovim_subsys_session::bridges::{BridgeContext, ExtensionScope};

    let bridge = bridges.get(kind)?;
    let cid = ClientId::new(client_id as usize);

    let data = match bridge.scope() {
        ExtensionScope::Client => {
            // Use with_bridge_context for cross-client reads (#543).
            // Holds both clients + state locks, pre-collects opponent data.
            session.with_bridge_context(cid, |own_ext, shared_ext, opponents| {
                let driver_cid = reovim_subsys_session::ClientId::new(client_id as usize);
                let context = BridgeContext::new(driver_cid, shared_ext, opponents);
                bridge.snapshot_with_context(own_ext, &context)
            })??
        }
        ExtensionScope::Shared => {
            session.with_state_sync(|state| bridge.snapshot(&state.app.extensions))?
        }
    };

    Some(Notification {
        event_type: "extension_updated".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::ExtensionUpdated(ExtensionUpdatedPayload {
            kind: kind.to_string(),
            data: data.to_string(),
            client_id,
        })),
    })
}

#[cfg(test)]
#[path = "notification_builder_tests.rs"]
mod tests;
