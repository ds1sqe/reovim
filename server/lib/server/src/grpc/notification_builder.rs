//! Notification builder - converts `ChangeSet` to gRPC notifications.
//!
//! This module bridges the session driver's change tracking to gRPC protocol notifications.
//! Following the data/presentation separation model:
//! - Server emits raw state changes (what changed)
//! - Client interprets and renders them (how to display)
//!
//! # Design
//!
//! `ChangeSet` tracks WHAT changed during an operation.
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
//! let changes = ChangeSet::new();
//! let notifications = build_notifications(&changes, &session, client_id, None);
//!
//! for notification in notifications {
//!     session.emit_notification(notification);
//! }
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

use {
    reovim_protocol::v2::{
        BufferClosedPayload, BufferOpenedPayload, DomainDatum, ExtensionUpdatedPayload,
        LayoutChangedPayload, Notification, ProjectionUpdatedPayload, TabPageInfo, WindowInfo,
        WindowRect, notification,
    },
    reovim_subsys_session::bridges::BridgeRegistry,
};

use crate::session::{ClientId, Session, change_set::ChangeSet};

/// Get current timestamp in milliseconds since Unix epoch.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX_EPOCH")
        .as_millis() as u64
}

/// Build gRPC notifications from a domain-neutral change set.
///
/// Converts a [`ChangeSet`] accumulated during an operation into a list of
/// gRPC `Notification` messages for streaming to connected clients.
///
/// # Arguments
///
/// * `changes` - The change set to convert
/// * `session` - Session for reading per-client and shared state (#486)
/// * `client_id` - Client ID that originated these changes (for multi-client filtering)
/// * `bridges` - Optional bridge registry for extension notifications (#514)
///
/// # Returns
///
/// A vector of notifications to emit. May be empty if no relevant changes.
#[must_use]
pub(crate) fn build_notifications(
    changes: &ChangeSet,
    session: &Session,
    client_id: u64,
    bridges: Option<&BridgeRegistry>,
) -> Vec<Notification> {
    let mut notifications = Vec::new();
    let timestamp = current_timestamp_ms();

    // Buffer lifecycle notifications (v3: Opened/Closed replace Modified+ListChanged)
    for buffer_id in &changes.created_buffers {
        notifications.push(build_buffer_opened_notification(*buffer_id, timestamp));
    }
    for buffer_id in &changes.deleted_buffers {
        notifications.push(build_buffer_closed_notification(*buffer_id, timestamp));
    }

    // Window/layout changed notification
    // Layout uses shared state (compositor) but may use per-client focused window
    if changes.layout_changed || changes.focus_changed {
        notifications.push(build_layout_notification(session, timestamp, client_id));
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

/// Build a buffer opened notification.
fn build_buffer_opened_notification(
    buffer_id: reovim_kernel::api::v1::BufferId,
    timestamp: u64,
) -> Notification {
    Notification {
        event_type: "buffer_opened".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::BufferOpened(BufferOpenedPayload {
            buffer_id: buffer_id.as_usize() as u64,
            path: None,
            name: String::new(),
        })),
    }
}

/// Build a buffer closed notification.
fn build_buffer_closed_notification(
    buffer_id: reovim_kernel::api::v1::BufferId,
    timestamp: u64,
) -> Notification {
    Notification {
        event_type: "buffer_closed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::BufferClosed(BufferClosedPayload {
            buffer_id: buffer_id.as_usize() as u64,
        })),
    }
}

/// Build a layout changed notification.
///
/// Uses the per-client compositor for geometry (#474).
/// Window buffer_id and tabs are domain-owned (#753 E3) — queried via domain driver.
///
/// # Arguments
///
/// * `session` - Session for reading per-client compositor and terminal_size
/// * `timestamp` - Notification timestamp
/// * `client_id` - Client for per-client compositor lookup
#[allow(clippy::cast_possible_truncation, clippy::option_if_let_else)]
fn build_layout_notification(session: &Session, timestamp: u64, client_id: u64) -> Notification {
    let cid = ClientId::new(client_id as usize);
    let editing_state = session.clients().client_state(cid);

    // Build window info list from per-client compositor (#474).
    // buffer_id is queried from the domain driver (domain-owned, #753 E3).
    let windows: Vec<WindowInfo> = if let Some(ref state) = editing_state {
        if let Some(ref compositor) = state.compositor {
            let (tw, th) = state.terminal_size;
            let screen = reovim_subsys_layout::Rect::new(0, 0, tw, th);
            let composite = compositor.composite(screen);
            let focused_id = composite.focused;

            // buffer_id: query domain driver; fall back to None (domain not yet wired)
            let active_buffer = session.active_buffer_for_client(cid);

            composite
                .placements
                .iter()
                .map(|p| {
                    let buffer_id = active_buffer.map(|id| id.as_usize() as u64);
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
                        opacity: Some(p.opacity),
                        primary_domain_id: 0,
                        embedded_domain_ids: vec![],
                        spatial_placement: None,
                    }
                })
                .collect()
        } else {
            // No compositor — empty window list (domain driver provides layout)
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Focused window: from compositor if available
    let focused_window_id = editing_state.as_ref().and_then(|state| {
        let compositor = state.compositor.as_ref()?;
        let (tw, th) = state.terminal_size;
        let screen = reovim_subsys_layout::Rect::new(0, 0, tw, th);
        compositor
            .composite(screen)
            .focused
            .map(|id| id.as_usize() as u64)
    });

    // tabs: domain-owned — return empty (#753 E3, wired in E5/E6)
    let active_tab_id = None;
    let tabs_info: Vec<TabPageInfo> = Vec::new();

    Notification {
        event_type: "layout_changed".to_string(),
        timestamp_ms: timestamp,
        payload: Some(notification::Payload::LayoutChanged(LayoutChangedPayload {
            focused_window_id,
            windows,
            client_id,
            active_tab_id,
            tabs: tabs_info,
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

/// Build projection notifications from a `StoreUpdateResult` (#753).
///
/// Converts changed persistent projections and transient projections into
/// `ProjectionUpdated` gRPC notifications. Pure function — no `Session` access needed.
///
/// # Arguments
///
/// * `result` - The store update result from `ProjectionStore::update()`
/// * `client_id` - Client that owns these projections
/// * `timestamp` - Notification timestamp (batch consistency)
#[must_use]
pub fn build_projection_notifications(
    result: &crate::session::projection_store::StoreUpdateResult,
    client_id: u64,
    timestamp: u64,
) -> Vec<Notification> {
    let mut notifications = Vec::with_capacity(result.changed.len() + result.transient.len());

    // Persistent projections that changed (new version assigned)
    for versioned in &result.changed {
        let proj = &versioned.projection;
        notifications.push(Notification {
            event_type: "projection_updated".to_string(),
            timestamp_ms: timestamp,
            payload: Some(notification::Payload::ProjectionUpdated(ProjectionUpdatedPayload {
                tag: proj.tag.as_str().to_string(),
                domain_id: proj.domain_id.0,
                window_id: proj.window_id.map(|w| w.as_usize() as u64),
                datum: Some(DomainDatum {
                    content: proj.payload.clone(),
                    display: proj.display.clone(),
                }),
                transient: false,
                version: versioned.version,
                client_id,
            })),
        });
    }

    // Transient projections (fire-and-forget, never cached)
    for proj in &result.transient {
        notifications.push(Notification {
            event_type: "projection_updated".to_string(),
            timestamp_ms: timestamp,
            payload: Some(notification::Payload::ProjectionUpdated(ProjectionUpdatedPayload {
                tag: proj.tag.as_str().to_string(),
                domain_id: proj.domain_id.0,
                window_id: proj.window_id.map(|w| w.as_usize() as u64),
                datum: Some(DomainDatum {
                    content: proj.payload.clone(),
                    display: proj.display.clone(),
                }),
                transient: true,
                version: 0,
                client_id,
            })),
        });
    }

    notifications
}

#[cfg(test)]
#[path = "notification_builder_tests.rs"]
mod tests;
