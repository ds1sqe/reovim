//! State-related RPC handlers.
//!
//! Handlers for `state/mode`, `state/cursor`, and related methods.

use reovim_protocol::v1::{CursorInfo, ModeInfo};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Handler for `state/mode` method.
///
/// Returns the current mode information.
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
pub fn state_mode(_ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move { Ok(handle_state_mode()) })
}

fn handle_state_mode() -> serde_json::Value {
    // For MVP, return a default mode info
    // Full mode tracking will be implemented later
    let mode_info = ModeInfo {
        focus: "Editor".to_string(),
        edit_mode: "Normal".to_string(),
        sub_mode: "None".to_string(),
        display: "NORMAL".to_string(),
    };

    serde_json::to_value(mode_info).expect("ModeInfo serialization should never fail")
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
pub fn state_cursor(_ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move { Ok(handle_state_cursor()) })
}

fn handle_state_cursor() -> serde_json::Value {
    // For MVP, return default cursor position
    // Full cursor tracking will be implemented later
    let cursor = CursorInfo { line: 0, column: 0 };

    serde_json::to_value(cursor).expect("CursorInfo serialization should never fail")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::session::{Session, SessionId},
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
        let ctx = RpcContext { session };

        let result = state_mode(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("edit_mode").is_some());
    }

    #[tokio::test]
    async fn test_state_cursor_handler() {
        let session = test_session();
        let ctx = RpcContext { session };

        let result = state_cursor(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("line").is_some());
        assert!(value.get("column").is_some());
    }
}
