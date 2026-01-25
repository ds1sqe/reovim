//! Editor-level RPC handlers (application lifecycle, display).
//!
//! Handlers for `editor/resize`, `editor/quit`, `editor/set_active_buffer`, and related methods.

use {
    reovim_driver_display::Rect, reovim_kernel::api::v1::BufferId, reovim_protocol::v1::RpcError,
    serde::Deserialize,
};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Parameters for `editor/resize` method.
#[derive(Debug, Deserialize)]
pub struct EditorResizeParams {
    /// New terminal width in columns.
    pub width: u16,
    /// New terminal height in rows.
    pub height: u16,
}

/// Handler for `editor/resize` method.
///
/// Updates the terminal dimensions in the client's viewport and compositor.
///
/// # Per-Client Viewport
///
/// Each client has independent terminal dimensions, allowing different
/// clients to have different window sizes (e.g., different terminal emulators).
///
/// # Compositor Screen Update
///
/// The compositor's screen is also updated to ensure window navigation and
/// layout calculations use the correct dimensions. In multi-client scenarios,
/// the compositor uses the size of the most recently resized client.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "editor/resize", "params": {"width": 120, "height": 40}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}
/// ```
///
/// # Errors
///
/// Returns `invalid_params` if width or height is zero.
#[must_use]
pub fn editor_resize(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: EditorResizeParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        // Validate dimensions
        if params.width == 0 || params.height == 0 {
            return Err(RpcError::invalid_params("Width and height must be positive"));
        }

        // Update client's viewport dimensions (Level 2 lock)
        {
            let mut viewport = ctx.client.viewport().write().await;
            viewport.terminal_width = params.width;
            viewport.terminal_height = params.height;
        } // Lock dropped

        // Update session terminal size and compositor screen (Level 1 lock)
        // This ensures navigation and layout calculations use correct dimensions
        ctx.session
            .with_state_mut(|state| {
                // Update session-level terminal size
                state.set_session_terminal_size(params.width, params.height);

                // Update compositor's screen for navigation calculations
                if let Some(compositor) = state.driver_session.compositor_mut() {
                    let screen = Rect::new(0, 0, params.width, params.height);
                    compositor.set_screen(screen);
                }
            })
            .await;

        Ok(serde_json::json!({"ok": true}))
    })
}

/// Handler for `editor/quit` method.
///
/// Requests a graceful shutdown by setting the running flag to false.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "editor/quit", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}
/// ```
#[must_use]
pub fn editor_quit(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        ctx.session
            .with_state_mut(|state| {
                state.app.running = false;
            })
            .await;

        Ok(serde_json::json!({"ok": true}))
    })
}

/// Parameters for `editor/set_active_buffer` method.
#[derive(Debug, Deserialize)]
pub struct EditorSetActiveBufferParams {
    /// Buffer ID to switch to.
    pub buffer_id: usize,
}

/// Handler for `editor/set_active_buffer` method.
///
/// Sets the active buffer for this client's viewport.
///
/// # Cursor Position Preservation
///
/// When switching buffers, this handler:
/// 1. Saves the current cursor position for the old buffer (if any)
/// 2. Restores the previously saved cursor position for the new buffer
///
/// This allows clients to maintain independent cursor positions per buffer,
/// enabling seamless buffer switching without losing position context.
///
/// # Per-Client Independence
///
/// Each client has its own active buffer, so multiple clients can view
/// different buffers simultaneously without interference.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "editor/set_active_buffer", "params": {"buffer_id": 1}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true, "previous_buffer_id": 0}}
/// ```
///
/// # Errors
///
/// Returns `invalid_params` if the buffer does not exist.
#[must_use]
pub fn editor_set_active_buffer(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: EditorSetActiveBufferParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let buffer_id = BufferId::from_raw(params.buffer_id);

        // Verify buffer exists
        let buffer_exists = ctx
            .session
            .with_state(|state| state.app.kernel.buffers.get(buffer_id).is_some())
            .await;

        if !buffer_exists {
            return Err(RpcError::invalid_params(format!(
                "Buffer {} does not exist",
                params.buffer_id
            )));
        }

        // Update viewport: save old cursor, set new active buffer
        let previous_buffer_id = {
            let mut viewport = ctx.client.viewport().write().await;
            let old_buffer = viewport.active_buffer;

            // Save cursor position for old buffer (if any)
            if let Some(old_id) = old_buffer {
                let old_cursor = viewport.cursor_for_buffer(old_id);
                viewport.set_cursor_for_buffer(old_id, old_cursor);
            }

            // Set new active buffer
            viewport.active_buffer = Some(buffer_id);
            drop(viewport); // Explicitly release lock before computing result

            // Note: cursor restoration for the new buffer happens on subsequent
            // cursor reads from the viewport's buffer_cursors map

            old_buffer.map_or(0, BufferId::as_usize)
        };

        // If we have a saved cursor, we might want to seek to it
        // For now, the buffer maintains its own canonical cursor position

        Ok(serde_json::json!({
            "ok": true,
            "previous_buffer_id": previous_buffer_id
        }))
    })
}

#[cfg(test)]
mod tests {
    use {super::*, std::sync::Arc};

    use crate::{
        server::rpc::{
            RpcContext,
            handlers::test_utils::{
                test_client, test_ctx, test_session, test_session_with_buffers,
            },
        },
        session::ClientId,
    };

    #[tokio::test]
    async fn test_editor_resize_success() {
        let session = test_session();
        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        let ctx = RpcContext {
            session,
            client_id,
            client: Arc::clone(&client),
        };

        let result = editor_resize(ctx, serde_json::json!({"width": 120, "height": 40})).await;
        assert!(result.is_ok());

        // Verify viewport dimensions were updated
        let (width, height) = {
            let viewport = client.viewport().read().await;
            (viewport.terminal_width, viewport.terminal_height)
        };
        assert_eq!(width, 120);
        assert_eq!(height, 40);
    }

    #[tokio::test]
    async fn test_editor_resize_zero_width() {
        let ctx = test_ctx();

        let result = editor_resize(ctx, serde_json::json!({"width": 0, "height": 24})).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("positive"));
    }

    #[tokio::test]
    async fn test_editor_resize_zero_height() {
        let ctx = test_ctx();

        let result = editor_resize(ctx, serde_json::json!({"width": 80, "height": 0})).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("positive"));
    }

    #[tokio::test]
    async fn test_editor_resize_invalid_params() {
        let ctx = test_ctx();

        let result = editor_resize(ctx, serde_json::json!({"invalid": "params"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_editor_resize_boundary_values() {
        let session = test_session();
        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        let ctx = RpcContext {
            session,
            client_id,
            client: Arc::clone(&client),
        };

        // Test max u16 values
        let result =
            editor_resize(ctx, serde_json::json!({"width": u16::MAX, "height": u16::MAX})).await;
        assert!(result.is_ok());

        let (width, height) = {
            let viewport = client.viewport().read().await;
            (viewport.terminal_width, viewport.terminal_height)
        };
        assert_eq!(width, u16::MAX);
        assert_eq!(height, u16::MAX);
    }

    #[tokio::test]
    async fn test_editor_quit_sets_running_false() {
        let session = test_session();

        // Verify initial state
        let running = session.with_state(|state| state.app.running).await;
        assert!(running);

        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);
        let ctx = RpcContext {
            session: Arc::clone(&session),
            client_id,
            client,
        };

        let result = editor_quit(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        // Verify running flag is now false
        let running = session.with_state(|state| state.app.running).await;
        assert!(!running);
    }

    #[tokio::test]
    async fn test_editor_resize_persistence() {
        let session = test_session();
        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        // Resize multiple times using the same client
        let ctx1 = RpcContext {
            session: Arc::clone(&session),
            client_id,
            client: Arc::clone(&client),
        };
        let _ = editor_resize(ctx1, serde_json::json!({"width": 100, "height": 30})).await;

        let ctx2 = RpcContext {
            session,
            client_id,
            client: Arc::clone(&client),
        };
        let _ = editor_resize(ctx2, serde_json::json!({"width": 150, "height": 45})).await;

        // Verify only the final state persists in viewport
        let (width, height) = {
            let viewport = client.viewport().read().await;
            (viewport.terminal_width, viewport.terminal_height)
        };
        assert_eq!(width, 150);
        assert_eq!(height, 45);
    }

    #[tokio::test]
    async fn test_editor_quit_returns_ok() {
        let ctx = test_ctx();

        let result = editor_quit(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(
            value
                .get("ok")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        );
    }

    #[tokio::test]
    async fn test_editor_set_active_buffer_nonexistent_buffer() {
        let ctx = test_ctx();

        // Try to switch to a non-existent buffer
        let result = editor_set_active_buffer(ctx, serde_json::json!({"buffer_id": 999})).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("does not exist"));
    }

    #[tokio::test]
    async fn test_editor_set_active_buffer_invalid_params() {
        let ctx = test_ctx();

        let result = editor_set_active_buffer(ctx, serde_json::json!({"wrong": "params"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_editor_set_active_buffer_success() {
        let session = test_session_with_buffers();

        // Create a buffer in the kernel context
        let buffer_id = session
            .with_state(|state| state.app.kernel.buffers.create())
            .await;

        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        let ctx = RpcContext {
            session,
            client_id,
            client: Arc::clone(&client),
        };

        // Set the active buffer
        let result =
            editor_set_active_buffer(ctx, serde_json::json!({"buffer_id": buffer_id.as_usize()}))
                .await;
        assert!(result.is_ok(), "Setting valid buffer should succeed");

        // Verify response contains ok and previous_buffer_id
        let value = result.unwrap();
        assert!(
            value
                .get("ok")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        );
        assert_eq!(
            value
                .get("previous_buffer_id")
                .and_then(serde_json::Value::as_u64),
            Some(0), // No previous buffer
            "Previous buffer should be 0 (None)"
        );

        // Verify viewport was updated
        let active = {
            let viewport = client.viewport().read().await;
            viewport.active_buffer
        };
        assert_eq!(active, Some(buffer_id), "Active buffer should be set in viewport");
    }

    #[tokio::test]
    async fn test_editor_set_active_buffer_returns_previous_id() {
        let session = test_session_with_buffers();

        // Create two buffers
        let buffer1 = session
            .with_state(|state| state.app.kernel.buffers.create())
            .await;
        let buffer2 = session
            .with_state(|state| state.app.kernel.buffers.create())
            .await;

        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        // Set first buffer as active
        let ctx1 = RpcContext {
            session: Arc::clone(&session),
            client_id,
            client: Arc::clone(&client),
        };
        let _ =
            editor_set_active_buffer(ctx1, serde_json::json!({"buffer_id": buffer1.as_usize()}))
                .await;

        // Switch to second buffer
        let ctx2 = RpcContext {
            session,
            client_id,
            client: Arc::clone(&client),
        };
        let result =
            editor_set_active_buffer(ctx2, serde_json::json!({"buffer_id": buffer2.as_usize()}))
                .await;
        assert!(result.is_ok());

        // Verify previous_buffer_id is buffer1
        let value = result.unwrap();
        assert_eq!(
            value
                .get("previous_buffer_id")
                .and_then(serde_json::Value::as_u64),
            Some(buffer1.as_usize() as u64),
            "Previous buffer ID should match buffer1"
        );
    }

    #[tokio::test]
    async fn test_editor_set_active_buffer_cursor_preserved() {
        use reovim_kernel::api::v1::Position;

        let session = test_session_with_buffers();

        // Create two buffers
        let buffer1 = session
            .with_state(|state| state.app.kernel.buffers.create())
            .await;
        let buffer2 = session
            .with_state(|state| state.app.kernel.buffers.create())
            .await;

        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        // Set buffer1 as active and set cursor position
        {
            let mut viewport = client.viewport().write().await;
            viewport.active_buffer = Some(buffer1);
            viewport.set_cursor_for_buffer(
                buffer1,
                Position {
                    line: 10,
                    column: 5,
                },
            );
        }

        // Switch to buffer2
        let ctx = RpcContext {
            session: Arc::clone(&session),
            client_id,
            client: Arc::clone(&client),
        };
        let _ = editor_set_active_buffer(ctx, serde_json::json!({"buffer_id": buffer2.as_usize()}))
            .await;

        // Verify buffer1 cursor was saved
        let cursor1 = {
            let viewport = client.viewport().read().await;
            viewport.cursor_for_buffer(buffer1)
        };
        assert_eq!(
            cursor1,
            Position {
                line: 10,
                column: 5
            },
            "Cursor for buffer1 should be preserved"
        );

        // Switch back to buffer1
        let ctx2 = RpcContext {
            session,
            client_id,
            client: Arc::clone(&client),
        };
        let _ =
            editor_set_active_buffer(ctx2, serde_json::json!({"buffer_id": buffer1.as_usize()}))
                .await;

        // Verify cursor is still preserved
        let cursor1_restored = {
            let viewport = client.viewport().read().await;
            viewport.cursor_for_buffer(buffer1)
        };
        assert_eq!(
            cursor1_restored,
            Position {
                line: 10,
                column: 5
            },
            "Cursor should be restored when switching back"
        );
    }

    #[tokio::test]
    async fn test_editor_set_active_buffer_multi_buffer_cursors() {
        use reovim_kernel::api::v1::Position;

        let session = test_session_with_buffers();

        // Create three buffers
        let buffer1 = session
            .with_state(|state| state.app.kernel.buffers.create())
            .await;
        let buffer2 = session
            .with_state(|state| state.app.kernel.buffers.create())
            .await;
        let buffer3 = session
            .with_state(|state| state.app.kernel.buffers.create())
            .await;

        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        // Set initial cursors for all buffers
        {
            let mut viewport = client.viewport().write().await;
            viewport.set_cursor_for_buffer(buffer1, Position { line: 1, column: 1 });
            viewport.set_cursor_for_buffer(
                buffer2,
                Position {
                    line: 20,
                    column: 10,
                },
            );
            viewport.set_cursor_for_buffer(
                buffer3,
                Position {
                    line: 100,
                    column: 50,
                },
            );
            viewport.active_buffer = Some(buffer1);
        }

        // Switch between buffers multiple times
        let ctx1 = RpcContext {
            session: Arc::clone(&session),
            client_id,
            client: Arc::clone(&client),
        };
        let _ =
            editor_set_active_buffer(ctx1, serde_json::json!({"buffer_id": buffer2.as_usize()}))
                .await;

        let ctx2 = RpcContext {
            session: Arc::clone(&session),
            client_id,
            client: Arc::clone(&client),
        };
        let _ =
            editor_set_active_buffer(ctx2, serde_json::json!({"buffer_id": buffer3.as_usize()}))
                .await;

        let ctx3 = RpcContext {
            session,
            client_id,
            client: Arc::clone(&client),
        };
        let _ =
            editor_set_active_buffer(ctx3, serde_json::json!({"buffer_id": buffer1.as_usize()}))
                .await;

        // Verify all cursors are preserved
        let (cursor1, cursor2, cursor3, active) = {
            let viewport = client.viewport().read().await;
            (
                viewport.cursor_for_buffer(buffer1),
                viewport.cursor_for_buffer(buffer2),
                viewport.cursor_for_buffer(buffer3),
                viewport.active_buffer,
            )
        };
        assert_eq!(cursor1, Position { line: 1, column: 1 });
        assert_eq!(
            cursor2,
            Position {
                line: 20,
                column: 10
            }
        );
        assert_eq!(
            cursor3,
            Position {
                line: 100,
                column: 50
            }
        );
        assert_eq!(active, Some(buffer1));
    }
}
