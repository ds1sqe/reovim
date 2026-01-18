//! State change notification emission.
//!
//! Compares state snapshots and emits appropriate notifications
//! to connected clients for any detected changes.
//!
//! # Notifications Emitted
//!
//! This module emits four notification types:
//!
//! | Notification | Trigger | Scope |
//! |--------------|---------|-------|
//! | `notify/mode_changed` | Mode changes | Session-wide |
//! | `notify/cursor_moved` | Cursor position changes | Buffer-scoped |
//! | `notify/buffer_modified` | Buffer modified flag changes | Buffer-scoped |
//! | `notify/render_complete` | Any visual state change | Buffer-scoped (or session-wide) |
//!
//! The `render_complete` notification is always emitted last, after all
//! specific change notifications, to signal clients that they can refresh.

use reovim_protocol::v1::{
    BufferModifiedPayload, CursorMovedPayload, ModeChangedPayload, ModeInfo, RenderCompletePayload,
};

use {
    super::{Session, snapshot::StateSnapshot},
    crate::notification::NotificationBroadcaster,
};

/// Emit notifications for state changes between snapshots.
///
/// Compares `before` and `after` snapshots and broadcasts appropriate
/// notifications for any differences:
/// - `notify/mode_changed` when mode changes (session-wide broadcast)
/// - `notify/cursor_moved` when cursor position changes (buffer-scoped broadcast)
/// - `notify/buffer_modified` when buffer modified flag changes (buffer-scoped broadcast)
/// - `notify/render_complete` when any visual state changes (buffer-scoped or session-wide)
///
/// # Buffer-Scoped Notifications
///
/// Cursor, buffer-modified, and render-complete notifications are sent only to
/// clients viewing the affected buffer when an active buffer exists. This is more
/// efficient than session-wide broadcasts and prevents irrelevant notifications
/// (e.g., a client viewing buffer A doesn't need cursor updates for buffer B).
///
/// Mode changes remain session-wide since mode is shared across all clients.
/// When no active buffer exists, render-complete falls back to session-wide broadcast.
///
/// # Render Complete
///
/// The `render_complete` notification is emitted after all specific change
/// notifications (mode, cursor, buffer) when any visual state has changed.
/// Clients use this as a signal to refresh their display.
///
/// # Arguments
///
/// * `session` - The session to broadcast to
/// * `before` - State snapshot captured before the operation
/// * `after` - State snapshot captured after the operation
///
/// # Panics
///
/// This function will not panic as notification serialization is infallible.
#[allow(clippy::useless_let_if_seq)] // We track changes across multiple independent conditions
pub async fn emit_state_changes(session: &Session, before: &StateSnapshot, after: &StateSnapshot) {
    let mut any_changes = false;

    // Mode changed - broadcast to ALL clients (mode is session-wide)
    if before.mode != after.mode {
        let mode_info = session
            .with_state(|state| {
                let display = state.mode_registry.display_name(&after.mode).to_string();
                ModeInfo {
                    focus: "Editor".to_string(),
                    edit_mode: after.mode.name().to_string(),
                    sub_mode: "None".to_string(),
                    display,
                }
            })
            .await;

        let payload = ModeChangedPayload { mode: mode_info };
        let json = serde_json::to_string(&payload.into_notification())
            .expect("notification serialization cannot fail");
        NotificationBroadcaster::broadcast_to_session(session, &json).await;
        any_changes = true;
    }

    // Cursor moved - broadcast only to clients viewing this buffer
    if let (Some(buffer_id), Some(pos)) = (after.active_buffer, after.cursor)
        && before.cursor != after.cursor
    {
        let payload = CursorMovedPayload {
            buffer_id: buffer_id.as_usize(),
            position: reovim_protocol::v1::Position {
                line: pos.line,
                column: pos.column,
            },
        };
        let json = serde_json::to_string(&payload.into_notification())
            .expect("notification serialization cannot fail");
        NotificationBroadcaster::broadcast_to_buffer(session, buffer_id, &json).await;
        any_changes = true;
    }

    // Buffer modified flag changed - broadcast only to clients viewing this buffer
    if let Some(buffer_id) = after.active_buffer
        && before.modified != after.modified
    {
        let payload = BufferModifiedPayload {
            buffer_id: buffer_id.as_usize(),
            modified: after.modified.unwrap_or(false),
        };
        let json = serde_json::to_string(&payload.into_notification())
            .expect("notification serialization cannot fail");
        NotificationBroadcaster::broadcast_to_buffer(session, buffer_id, &json).await;
        any_changes = true;
    }

    // Emit render_complete if any visual state changed
    if any_changes {
        emit_render_complete(session, after.active_buffer).await;
    }
}

/// Emit `render_complete` notification to signal clients to refresh display.
///
/// Uses buffer-scoped broadcast when an active buffer exists, otherwise
/// falls back to session-wide broadcast.
async fn emit_render_complete(
    session: &Session,
    active_buffer: Option<reovim_kernel::api::v1::BufferId>,
) {
    let payload = RenderCompletePayload::default();
    let json = serde_json::to_string(&payload.into_notification())
        .expect("RenderCompletePayload serialization cannot fail");

    if let Some(buffer_id) = active_buffer {
        NotificationBroadcaster::broadcast_to_buffer(session, buffer_id, &json).await;
    } else {
        NotificationBroadcaster::broadcast_to_session(session, &json).await;
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::session::SessionId,
        reovim_driver_vfs::{MockVfs, VfsDriver},
        reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId},
        std::sync::Arc,
    };

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(MockVfs::new())
    }

    fn test_session() -> Arc<Session> {
        Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            ModeId::new(ModuleId::new("test"), "normal"),
            test_vfs(),
        )
    }

    fn normal_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn insert_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "insert")
    }

    #[tokio::test]
    async fn test_emit_no_changes() {
        let session = test_session();
        let snapshot = StateSnapshot {
            mode: normal_mode(),
            active_buffer: None,
            cursor: None,
            modified: None,
        };

        // Should not panic with no clients and no changes
        emit_state_changes(&session, &snapshot, &snapshot).await;
    }

    #[tokio::test]
    async fn test_emit_mode_change() {
        let session = test_session();
        let before = StateSnapshot {
            mode: normal_mode(),
            active_buffer: None,
            cursor: None,
            modified: None,
        };
        let after = StateSnapshot {
            mode: insert_mode(),
            active_buffer: None,
            cursor: None,
            modified: None,
        };

        // Should not panic with no clients
        emit_state_changes(&session, &before, &after).await;
    }

    #[tokio::test]
    async fn test_emit_modified_change() {
        use reovim_kernel::api::v1::BufferId;

        let session = test_session();
        let buffer_id = BufferId::new();

        let before = StateSnapshot {
            mode: normal_mode(),
            active_buffer: Some(buffer_id),
            cursor: None,
            modified: Some(false),
        };
        let after = StateSnapshot {
            mode: normal_mode(),
            active_buffer: Some(buffer_id),
            cursor: None,
            modified: Some(true),
        };

        // Should not panic with no clients
        // Also emits render_complete (buffer-scoped)
        emit_state_changes(&session, &before, &after).await;
    }

    #[tokio::test]
    async fn test_emit_cursor_move() {
        use reovim_kernel::api::v1::{BufferId, Position};

        let session = test_session();
        let buffer_id = BufferId::new();

        let before = StateSnapshot {
            mode: normal_mode(),
            active_buffer: Some(buffer_id),
            cursor: Some(Position { line: 0, column: 0 }),
            modified: None,
        };
        let after = StateSnapshot {
            mode: normal_mode(),
            active_buffer: Some(buffer_id),
            cursor: Some(Position { line: 1, column: 5 }),
            modified: None,
        };

        // Should not panic with no clients
        // Emits cursor_moved + render_complete (buffer-scoped)
        emit_state_changes(&session, &before, &after).await;
    }

    #[tokio::test]
    async fn test_emit_combined_changes() {
        use reovim_kernel::api::v1::{BufferId, Position};

        let session = test_session();
        let buffer_id = BufferId::new();

        let before = StateSnapshot {
            mode: normal_mode(),
            active_buffer: Some(buffer_id),
            cursor: Some(Position { line: 0, column: 0 }),
            modified: Some(false),
        };
        let after = StateSnapshot {
            mode: insert_mode(),
            active_buffer: Some(buffer_id),
            cursor: Some(Position { line: 1, column: 5 }),
            modified: Some(true),
        };

        // Should not panic with no clients
        // Emits mode_changed + cursor_moved + buffer_modified + render_complete
        // render_complete is emitted exactly once despite multiple changes
        emit_state_changes(&session, &before, &after).await;
    }

    #[tokio::test]
    async fn test_render_complete_session_wide_when_no_buffer() {
        let session = test_session();

        let before = StateSnapshot {
            mode: normal_mode(),
            active_buffer: None,
            cursor: None,
            modified: None,
        };
        let after = StateSnapshot {
            mode: insert_mode(),
            active_buffer: None,
            cursor: None,
            modified: None,
        };

        // Should not panic with no clients
        // render_complete is session-wide when no active buffer
        emit_state_changes(&session, &before, &after).await;
    }

    #[tokio::test]
    async fn test_render_complete_buffer_scoped_when_buffer_exists() {
        use reovim_kernel::api::v1::{BufferId, Position};

        let session = test_session();
        let buffer_id = BufferId::new();

        let before = StateSnapshot {
            mode: normal_mode(),
            active_buffer: Some(buffer_id),
            cursor: Some(Position { line: 0, column: 0 }),
            modified: None,
        };
        let after = StateSnapshot {
            mode: normal_mode(),
            active_buffer: Some(buffer_id),
            cursor: Some(Position { line: 0, column: 1 }),
            modified: None,
        };

        // Should not panic with no clients
        // render_complete is buffer-scoped when active buffer exists
        emit_state_changes(&session, &before, &after).await;
    }
}
