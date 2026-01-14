//! State-related RPC handlers.
//!
//! Handlers for `state/mode`, `state/cursor`, and related methods.

use reovim_protocol::v1::{CursorInfo, ModeInfo};

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

        Ok(serde_json::to_value(mode_info).unwrap_or_default())
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
#[must_use]
pub fn state_cursor(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Query cursor position from session state
        // Falls back to (0, 0) if no active buffer
        let cursor = ctx
            .session
            .with_state(|state| {
                // TODO: Get actual cursor position from active buffer when implemented
                let _ = state.app.active_buffer;
                CursorInfo { line: 0, column: 0 }
            })
            .await;

        Ok(serde_json::to_value(cursor).unwrap_or_default())
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
}
