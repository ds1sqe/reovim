//! Server-related RPC handlers.
//!
//! Handlers for `server/kill` and related methods.

use reovim_protocol::v1::OkResult;

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Handler for `server/kill` method.
///
/// Requests the server to shut down gracefully.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "server/kill", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}
/// ```
#[must_use]
pub fn server_kill(_ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move { Ok(handle_server_kill()) })
}

fn handle_server_kill() -> serde_json::Value {
    // For MVP, just acknowledge the kill request
    // The actual shutdown will be handled by the server
    serde_json::to_value(OkResult::new()).expect("OkResult serialization should never fail")
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
    async fn test_server_kill_handler() {
        let session = test_session();
        let ctx = RpcContext { session };

        let result = server_kill(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
    }
}
