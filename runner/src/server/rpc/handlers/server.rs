//! Server-related RPC handlers.
//!
//! Handlers for `server/kill` and related methods.

use reovim_protocol::v1::OkResult;

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Handler for `server/kill` method.
///
/// Requests the server to shut down gracefully by setting the session's
/// quit flag. The server will exit after completing any pending operations.
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
///
/// # Panics
///
/// This function will not panic as `OkResult` serialization is infallible.
#[must_use]
pub fn server_kill(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Request the session to quit
        ctx.session.request_quit().await;

        Ok(serde_json::to_value(OkResult::new()).expect("OkResult serialization cannot fail"))
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::server::rpc::handlers::test_utils::test_ctx};

    #[tokio::test]
    async fn test_server_kill_handler() {
        let ctx = test_ctx();

        let result = server_kill(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
    }
}
