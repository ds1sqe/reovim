//! State-related RPC handlers.
//!
//! Handlers for `state/mode`, `state/cursor`, and related methods.

use {
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v1::{
        CursorInfo, ModeInfo, Position, ScreenInfo, SelectionInfo, SelectionMode,
    },
};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Handler for `state/mode` method.
///
/// Returns the current mode information from the session state.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/mode", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"focus": "Editor", "edit_mode": "Normal", ...}}
/// ```
///
/// # Panics
///
/// This function will not panic as `ModeInfo` serialization is infallible.
#[must_use]
pub fn state_mode(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Get the current mode from session state
        let mode_id = ctx.session.current_mode().await;

        // Get the display text from mode registry
        let display = ctx
            .session
            .with_state(|state| state.mode_registry.status_text(&mode_id).to_string())
            .await;

        let mode_info = ModeInfo {
            focus: "Editor".to_string(),
            edit_mode: mode_id.name().to_string(),
            sub_mode: "None".to_string(),
            display,
        };

        Ok(serde_json::to_value(mode_info).expect("ModeInfo serialization cannot fail"))
    })
}

/// Handler for `state/cursor` method.
///
/// Returns the cursor position in this client's active buffer.
///
/// # Per-Client Viewport
///
/// This handler reads from the client's viewport to determine the active buffer,
/// allowing different clients to view different buffers simultaneously.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/cursor", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"line": 0, "column": 0}}
/// ```
///
/// # Panics
///
/// This function will not panic as `CursorInfo` serialization is infallible.
#[must_use]
pub fn state_cursor(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Read client's active buffer from viewport (Level 2 lock)
        let active_buffer = {
            let viewport = ctx.client.viewport().read().await;
            viewport.active_buffer
        }; // Lock dropped before acquiring session lock

        // Query cursor position from session state
        // Falls back to (0, 0) if no active buffer
        let cursor = ctx
            .session
            .with_state(|state| {
                // Get cursor from client's active buffer
                if let Some(buffer_id) = active_buffer
                    && let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id)
                {
                    let pos = buffer_arc.read().position();
                    return CursorInfo {
                        line: pos.line,
                        column: pos.column,
                    };
                }
                // Fallback when no active buffer
                CursorInfo { line: 0, column: 0 }
            })
            .await;

        Ok(serde_json::to_value(cursor).expect("CursorInfo serialization cannot fail"))
    })
}

/// Handler for `state/screen` method.
///
/// Returns the current screen/viewport information for this client.
///
/// # Per-Client Viewport
///
/// This handler reads from the client's viewport, allowing each client to have
/// independent terminal dimensions and active buffer.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/screen", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"width": 80, "height": 24, "active_buffer_id": 0, "window_count": 1}}
/// ```
///
/// # Panics
///
/// This function will not panic as `ScreenInfo` serialization is infallible.
#[must_use]
pub fn state_screen(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Read screen info from client's viewport (Level 2 lock)
        let screen_info = {
            let viewport = ctx.client.viewport().read().await;
            ScreenInfo {
                width: viewport.terminal_width,
                height: viewport.terminal_height,
                active_buffer_id: viewport.active_buffer.map_or(0, BufferId::as_usize),
                active_window_id: None,
                window_count: 1,
            }
        }; // Lock dropped

        Ok(serde_json::to_value(screen_info).expect("ScreenInfo serialization cannot fail"))
    })
}

/// Handler for `state/selection` method.
///
/// Returns the current selection state.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/selection", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"active": false, "mode": "character", ...}}
/// ```
///
/// # Panics
///
/// This function will not panic as `SelectionInfo` serialization is infallible.
#[must_use]
pub fn state_selection(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let selection_info = ctx
            .session
            .with_state(|state| {
                // Check if we have an active buffer with a selection
                if let Some(buffer_id) = state.app.active_buffer
                    && let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id)
                {
                    let buffer = buffer_arc.read();
                    let selection = buffer.selection();

                    // Check if selection is active
                    if selection.is_active() {
                        let anchor = selection.anchor;
                        let cursor_pos = buffer.position();

                        return SelectionInfo {
                            active: true,
                            mode: match selection.mode() {
                                reovim_kernel::api::v1::SelectionMode::Character => {
                                    SelectionMode::Character
                                }
                                reovim_kernel::api::v1::SelectionMode::Line => SelectionMode::Line,
                                reovim_kernel::api::v1::SelectionMode::Block => {
                                    SelectionMode::Block
                                }
                            },
                            anchor: Position::new(anchor.line, anchor.column),
                            cursor: Position::new(cursor_pos.line, cursor_pos.column),
                        };
                    }
                }

                // No selection active
                SelectionInfo {
                    active: false,
                    mode: SelectionMode::Character,
                    anchor: Position::new(0, 0),
                    cursor: Position::new(0, 0),
                }
            })
            .await;

        Ok(serde_json::to_value(selection_info).expect("SelectionInfo serialization cannot fail"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        server::rpc::{
            RpcContext,
            handlers::test_utils::{test_client, test_ctx, test_session},
        },
        session::ClientId,
    };

    #[tokio::test]
    async fn test_state_mode_handler() {
        let ctx = test_ctx();

        let result = state_mode(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("edit_mode").is_some());
    }

    #[tokio::test]
    async fn test_state_cursor_handler() {
        let ctx = test_ctx();

        let result = state_cursor(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("line").is_some());
        assert!(value.get("column").is_some());
    }

    #[tokio::test]
    async fn test_state_screen_returns_dimensions() {
        let ctx = test_ctx();

        let result = state_screen(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        // Default viewport size is 80x24 (VT100 defaults)
        assert_eq!(value.get("width").and_then(serde_json::Value::as_u64), Some(80));
        assert_eq!(value.get("height").and_then(serde_json::Value::as_u64), Some(24));
        assert_eq!(
            value
                .get("active_buffer_id")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
        assert_eq!(
            value
                .get("window_count")
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
    }

    #[tokio::test]
    async fn test_state_screen_no_active_buffer() {
        let ctx = test_ctx();

        let result = state_screen(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        // No active buffer in viewport should return buffer_id = 0
        assert_eq!(
            value
                .get("active_buffer_id")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
    }

    #[tokio::test]
    async fn test_state_screen_after_resize() {
        let session = test_session();
        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        // Resize the client's viewport
        {
            let mut viewport = client.viewport().write().await;
            viewport.terminal_width = 200;
            viewport.terminal_height = 50;
        }

        let ctx = RpcContext {
            session,
            client_id,
            client,
        };

        let result = state_screen(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert_eq!(value.get("width").and_then(serde_json::Value::as_u64), Some(200));
        assert_eq!(value.get("height").and_then(serde_json::Value::as_u64), Some(50));
    }
}
