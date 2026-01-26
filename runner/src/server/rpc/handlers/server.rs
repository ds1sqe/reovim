//! Server-related RPC handlers.
//!
//! Handlers for `server/kill` and related methods.

use reovim_protocol::v1::OkResult;

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Handler for `server/kill` method.
///
/// Requests the server to shut down gracefully by:
/// 1. Setting the session's quit flag
/// 2. Signaling the server's accept loop to exit
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
        // Request the session to quit (for session-level cleanup)
        ctx.session.request_quit().await;

        // Signal the server's accept loop to exit (#446)
        // This bridges the RPC handler to the server's main loop.
        // Using watch::send which preserves state even if sent during accept().
        let _ = ctx.shutdown_tx.send(true);

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
