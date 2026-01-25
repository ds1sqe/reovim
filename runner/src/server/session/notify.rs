//! State change notification emission.
//!
//! Provides two notification mechanisms:
//!
//! 1. [`emit_state_changes`] - Compares before/after snapshots to detect changes
//! 2. [`emit_from_state_changes`] - Uses pre-computed `StateChanges` from resolvers
//!
//! The second method is preferred when resolvers use the `SessionApi` directly,
//! as changes are tracked during execution rather than inferred via snapshot diff.
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
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::StateChanges;
//!
//! // From resolver using SessionApi
//! let changes = runtime.take_changes();
//! if changes.has_changes() {
//!     emit_from_state_changes(&session, &changes).await;
//! }
//! ```

use {
    reovim_driver_display::Rect,
    reovim_driver_session::api::StateChanges,
    reovim_protocol::v1::{
        BufferId as ProtocolBufferId, BufferModifiedPayload, CursorMovedPayload,
        LayoutChangedPayload, ModeChangedPayload, ModeInfo, RenderCompletePayload,
        WireLayoutChangeKind, WireLayoutInfo, WireWindowId,
    },
};

use {
    super::{Session, snapshot::StateSnapshot, state::SessionState},
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
            buffer_id: ProtocolBufferId::from(buffer_id.as_usize()),
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
            buffer_id: ProtocolBufferId::from(buffer_id.as_usize()),
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

/// Determine layout change kind from `StateChanges`.
///
/// Maps the change tracking to appropriate wire format kind.
fn determine_layout_change_kind(changes: &StateChanges) -> WireLayoutChangeKind {
    // Priority: Split > Close > Focus > Resize > Equalize
    if let Some(&window_id) = changes.windows_created.first() {
        // Window was created - this is a split
        WireLayoutChangeKind::Split {
            new_window: WireWindowId::from(window_id.as_usize()),
            // We don't know the direction from StateChanges, default to vertical
            direction: reovim_protocol::v1::WireSplitDirection::Vertical,
        }
    } else if let Some(&window_id) = changes.windows_closed.first() {
        // Window was closed
        WireLayoutChangeKind::Close {
            closed_window: WireWindowId::from(window_id.as_usize()),
            new_focus: None, // Will be filled by the layout info
        }
    } else if changes.focus_changed {
        // Focus changed (we don't have from/to in StateChanges)
        WireLayoutChangeKind::Focus {
            from: None,
            to: WireWindowId::from(0), // Default, will need layout context
        }
    } else {
        // Generic window change (resize, equalize, etc.)
        WireLayoutChangeKind::Equalize
    }
}

/// Get current layout info from session state.
#[allow(clippy::option_if_let_else)] // if/else is clearer than map_or_else here
fn get_layout_info(state: &SessionState, width: u16, height: u16) -> WireLayoutInfo {
    use reovim_protocol::v1::{WireLayerId, WireRect, WireWindowPlacement, WireZone};

    let screen = Rect::new(0, 0, width, height);

    // Get compositor from driver_session
    if let Some(compositor) = state.driver_session.compositor() {
        use reovim_driver_display::layout::Zone;

        let result = compositor.composite(screen);

        // Convert to wire format
        let windows: Vec<WireWindowPlacement> = result
            .placements
            .iter()
            .map(|p| WireWindowPlacement {
                window_id: WireWindowId::from(p.window_id.as_usize()),
                layer_id: WireLayerId::from(p.layer_id.as_u16() as usize),
                zone: match p.zone {
                    Zone::Tiled => WireZone::Tiled,
                    Zone::Float => WireZone::Float,
                    Zone::Overlay => WireZone::Overlay,
                },
                bounds: WireRect::new(p.bounds.x, p.bounds.y, p.bounds.width, p.bounds.height),
                z_order: p.z_order.as_u16(),
                visible: p.visible,
                focusable: p.focusable,
                buffer_id: None, // TODO: window-buffer mapping (#440)
            })
            .collect();

        WireLayoutInfo {
            screen: WireRect::new(
                result.screen.x,
                result.screen.y,
                result.screen.width,
                result.screen.height,
            ),
            windows,
            focused_window: result.focused.map(|id| WireWindowId::from(id.as_usize())),
            active_layer: result
                .active_layer
                .map(|id| WireLayerId::from(id.as_u16() as usize)),
            window_count: result.placements.len(),
        }
    } else {
        // No compositor - single window fallback
        WireLayoutInfo::single_window(width, height, None)
    }
}

/// Emit notifications from pre-computed `StateChanges`.
///
/// Unlike [`emit_state_changes`] which compares before/after snapshots,
/// this function uses `StateChanges` that resolvers accumulated during
/// execution via the `SessionApi`. This is more efficient and provides
/// richer information (e.g., lists of affected buffers).
///
/// # Use Case
///
/// When resolvers use `SessionApi` methods (like `ModeApi::push_mode` or
/// `BufferApi::move_cursor`), changes are tracked internally. After resolution,
/// call this function to broadcast the accumulated changes:
///
/// ```ignore
/// let mut runtime = SessionRuntime::new(&mut session, &kernel, &executor);
/// let result = resolver.resolve_with_session(key, state, input, &mut runtime, extensions);
/// let changes = runtime.take_changes();
///
/// if changes.has_changes() {
///     emit_from_state_changes(&session, &changes).await;
/// }
/// ```
///
/// # Notifications Emitted
///
/// - `notify/mode_changed` - When `changes.mode_changed` is true (session-wide)
/// - `notify/cursor_moved` - For each buffer in `changes.affected_buffers` where cursor moved
/// - `notify/buffer_modified` - For each buffer in `changes.modified_buffers`
/// - `notify/render_complete` - Final signal when any changes occurred
///
/// # Design Note
///
/// This function bridges the resolver's `StateChanges` (from session driver)
/// to the notification system (in runner). It follows mechanism vs policy:
/// - Mechanism: `StateChanges` tracks WHAT changed
/// - Policy: This function decides HOW to notify
///
/// # Panics
///
/// Panics if notification serialization fails, which should never happen
/// as the notification types implement `Serialize` correctly.
#[expect(
    clippy::useless_let_if_seq,
    reason = "any_emitted is set in multiple conditionals, not a single if/else"
)]
pub async fn emit_from_state_changes(session: &Session, changes: &StateChanges) {
    if !changes.has_changes() {
        return;
    }

    let mut any_emitted = false;

    // Mode changed - broadcast to ALL clients (mode is session-wide)
    if changes.mode_changed {
        // Get current mode info from session
        let mode_info = session
            .with_state(|state| {
                let mode = state.current_mode();
                let display = state.mode_registry.display_name(mode).to_string();
                ModeInfo {
                    focus: "Editor".to_string(),
                    edit_mode: mode.name().to_string(),
                    sub_mode: "None".to_string(),
                    display,
                }
            })
            .await;

        let payload = ModeChangedPayload { mode: mode_info };
        let json = serde_json::to_string(&payload.into_notification())
            .expect("notification serialization cannot fail");
        NotificationBroadcaster::broadcast_to_session(session, &json).await;
        any_emitted = true;
    }

    // Cursor moved - broadcast to clients viewing affected buffers
    if changes.cursor_moved {
        for &buffer_id in &changes.affected_buffers {
            // Get cursor position from the buffer
            let cursor_opt = session
                .with_state(|state| {
                    state
                        .app
                        .kernel
                        .buffers
                        .get(buffer_id)
                        .map(|buf| buf.read().position())
                })
                .await;

            if let Some(pos) = cursor_opt {
                let payload = CursorMovedPayload {
                    buffer_id: ProtocolBufferId::from(buffer_id.as_usize()),
                    position: reovim_protocol::v1::Position {
                        line: pos.line,
                        column: pos.column,
                    },
                };
                let json = serde_json::to_string(&payload.into_notification())
                    .expect("notification serialization cannot fail");
                NotificationBroadcaster::broadcast_to_buffer(session, buffer_id, &json).await;
                any_emitted = true;
            }
        }
    }

    // Buffer modified - broadcast to clients viewing modified buffers
    if changes.buffer_modified {
        for &buffer_id in &changes.modified_buffers {
            // Get modified status from buffer
            let modified = session
                .with_state(|state| {
                    state
                        .app
                        .kernel
                        .buffers
                        .get(buffer_id)
                        .is_some_and(|buf| buf.read().is_modified())
                })
                .await;

            let payload = BufferModifiedPayload {
                buffer_id: ProtocolBufferId::from(buffer_id.as_usize()),
                modified,
            };
            let json = serde_json::to_string(&payload.into_notification())
                .expect("notification serialization cannot fail");
            NotificationBroadcaster::broadcast_to_buffer(session, buffer_id, &json).await;
            any_emitted = true;
        }
    }

    // TODO: Handle buffer lifecycle notifications (created, deleted, renamed)
    // These would require new notification types in the protocol

    // Layout changed - broadcast to ALL clients (layout is session-wide)
    if changes.window_changed || changes.focus_changed {
        // Determine the kind of layout change
        let kind = determine_layout_change_kind(changes);

        // Get current layout from compositor using session's terminal size
        let layout_info = session
            .with_state(|state| {
                let (width, height) = state.session_terminal_size();
                get_layout_info(state, width, height)
            })
            .await;

        let payload = LayoutChangedPayload::new(kind, layout_info);
        let json = serde_json::to_string(&payload.into_notification())
            .expect("LayoutChangedPayload serialization cannot fail");
        NotificationBroadcaster::broadcast_to_session(session, &json).await;
        any_emitted = true;
    }

    // Emit render_complete if any notifications were sent
    if any_emitted {
        // Determine active buffer for scoped broadcast (from driver_session SSOT)
        let active_buffer = session
            .with_state(SessionState::session_active_buffer)
            .await;
        emit_render_complete(session, active_buffer).await;
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

    // ==========================================================================
    // emit_from_state_changes tests
    // ==========================================================================

    #[tokio::test]
    async fn test_emit_from_changes_empty() {
        let session = test_session();
        let changes = StateChanges::new();

        // Empty changes should return immediately without emitting
        emit_from_state_changes(&session, &changes).await;
    }

    #[tokio::test]
    async fn test_emit_from_changes_mode_changed() {
        let session = test_session();
        let mut changes = StateChanges::new();
        changes.record_mode_change();

        // Should not panic with no clients
        // Should emit mode_changed notification
        emit_from_state_changes(&session, &changes).await;
    }

    #[tokio::test]
    async fn test_emit_from_changes_cursor_moved() {
        use reovim_kernel::api::v1::BufferId;

        let session = test_session();
        let buffer_id = BufferId::new();

        let mut changes = StateChanges::new();
        changes.record_cursor_move(buffer_id);

        // Should not panic with no clients (and buffer doesn't exist)
        emit_from_state_changes(&session, &changes).await;
    }

    #[tokio::test]
    async fn test_emit_from_changes_buffer_modified() {
        use reovim_kernel::api::v1::BufferId;

        let session = test_session();
        let buffer_id = BufferId::new();

        let mut changes = StateChanges::new();
        changes.record_buffer_modified(buffer_id);

        // Should not panic with no clients (and buffer doesn't exist)
        emit_from_state_changes(&session, &changes).await;
    }

    #[tokio::test]
    async fn test_emit_from_changes_combined() {
        use reovim_kernel::api::v1::BufferId;

        let session = test_session();
        let buffer_id = BufferId::new();

        let mut changes = StateChanges::new();
        changes.record_mode_change();
        changes.record_cursor_move(buffer_id);
        changes.record_buffer_modified(buffer_id);

        // Combined changes should emit multiple notifications
        emit_from_state_changes(&session, &changes).await;
    }

    #[tokio::test]
    async fn test_emit_from_changes_has_changes() {
        let mut changes = StateChanges::new();
        assert!(!changes.has_changes());

        changes.record_mode_change();
        assert!(changes.has_changes());
    }

    // ==========================================================================
    // Layout notification tests (#444)
    // ==========================================================================

    #[tokio::test]
    async fn test_emit_from_changes_window_created() {
        use reovim_kernel::api::v1::WindowId;

        let session = test_session();
        let window_id = WindowId::new();

        let mut changes = StateChanges::new();
        changes.record_window_created(window_id);

        // Should not panic with no clients
        // Should emit layout_changed notification
        emit_from_state_changes(&session, &changes).await;
    }

    #[tokio::test]
    async fn test_emit_from_changes_window_closed() {
        use reovim_kernel::api::v1::WindowId;

        let session = test_session();
        let window_id = WindowId::new();

        let mut changes = StateChanges::new();
        changes.record_window_closed(window_id);

        // Should not panic with no clients
        // Should emit layout_changed notification
        emit_from_state_changes(&session, &changes).await;
    }

    #[tokio::test]
    async fn test_emit_from_changes_focus_changed() {
        let session = test_session();

        let mut changes = StateChanges::new();
        changes.record_focus_change();

        // Should not panic with no clients
        // Should emit layout_changed notification for focus change
        emit_from_state_changes(&session, &changes).await;
    }

    #[test]
    fn test_determine_layout_change_kind_split() {
        use reovim_kernel::api::v1::WindowId;

        let mut changes = StateChanges::new();
        let window_id = WindowId::new();
        changes.record_window_created(window_id);

        let kind = determine_layout_change_kind(&changes);
        assert!(matches!(kind, WireLayoutChangeKind::Split { .. }));
    }

    #[test]
    fn test_determine_layout_change_kind_close() {
        use reovim_kernel::api::v1::WindowId;

        let mut changes = StateChanges::new();
        let window_id = WindowId::new();
        changes.record_window_closed(window_id);

        let kind = determine_layout_change_kind(&changes);
        assert!(matches!(kind, WireLayoutChangeKind::Close { .. }));
    }

    #[test]
    fn test_determine_layout_change_kind_focus() {
        let mut changes = StateChanges::new();
        changes.record_focus_change();

        let kind = determine_layout_change_kind(&changes);
        assert!(matches!(kind, WireLayoutChangeKind::Focus { .. }));
    }

    #[test]
    fn test_determine_layout_change_kind_equalize_fallback() {
        let mut changes = StateChanges::new();
        changes.window_changed = true; // Generic window change

        let kind = determine_layout_change_kind(&changes);
        assert!(matches!(kind, WireLayoutChangeKind::Equalize));
    }
}
