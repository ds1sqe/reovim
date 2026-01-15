//! State-related RPC handlers.
//!
//! Handlers for `state/mode`, `state/cursor`, and related methods.

use {
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v1::{CursorInfo, ModeInfo, ScreenInfo},
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
/// Returns the cursor position in the active buffer.
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
        // Query cursor position from session state
        // Falls back to (0, 0) if no active buffer
        let cursor = ctx
            .session
            .with_state(|state| {
                // Get active buffer and retrieve cursor position
                if let Some(buffer_id) = state.app.active_buffer
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
/// Returns the current screen/viewport information.
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
        let screen_info = ctx
            .session
            .with_state(|state| ScreenInfo {
                width: state.app.terminal_width,
                height: state.app.terminal_height,
                active_buffer_id: state.app.active_buffer.map_or(0, BufferId::as_usize),
                active_window_id: None,
                window_count: 1,
            })
            .await;

        Ok(serde_json::to_value(screen_info).expect("ScreenInfo serialization cannot fail"))
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::session::{ClientId, Session, SessionId},
        reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId},
        std::sync::Arc,
    };

    fn test_session() -> Arc<Session> {
        Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            ModeId::new(ModuleId::new("test"), "normal"),
        )
    }

    #[tokio::test]
    async fn test_state_mode_handler() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let result = state_mode(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("edit_mode").is_some());
    }

    #[tokio::test]
    async fn test_state_cursor_handler() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let result = state_cursor(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("line").is_some());
        assert!(value.get("column").is_some());
    }

    #[tokio::test]
    async fn test_state_screen_returns_dimensions() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let result = state_screen(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        // Default terminal size is 80x24
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
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let result = state_screen(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        // No active buffer should return buffer_id = 0
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

        // Resize the terminal
        session
            .with_state_mut(|state| {
                state.app.terminal_width = 200;
                state.app.terminal_height = 50;
            })
            .await;

        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let result = state_screen(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert_eq!(value.get("width").and_then(serde_json::Value::as_u64), Some(200));
        assert_eq!(value.get("height").and_then(serde_json::Value::as_u64), Some(50));
    }
}
