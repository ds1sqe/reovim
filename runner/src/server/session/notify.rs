//! State change notification emission.
//!
//! Compares state snapshots and emits appropriate notifications
//! to connected clients for any detected changes.

use reovim_protocol::v1::{
    BufferModifiedPayload, CursorMovedPayload, ModeChangedPayload, ModeInfo,
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
///
/// # Buffer-Scoped Notifications
///
/// Cursor and buffer-modified notifications are sent only to clients viewing
/// the affected buffer. This is more efficient than session-wide broadcasts
/// and prevents irrelevant notifications (e.g., a client viewing buffer A
/// doesn't need cursor updates for buffer B).
///
/// Mode changes remain session-wide since mode is shared across all clients.
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
pub async fn emit_state_changes(session: &Session, before: &StateSnapshot, after: &StateSnapshot) {
    // Mode changed - broadcast to ALL clients (mode is session-wide)
    if before.mode != after.mode {
        let mode_info = session
            .with_state(|state| {
                let display = state.mode_registry.status_text(&after.mode).to_string();
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
        emit_state_changes(&session, &before, &after).await;
    }
}
