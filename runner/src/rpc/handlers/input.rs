//! Input-related RPC handlers.
//!
//! Handlers for `input/keys` and related methods.

use reovim_protocol::v1::{InputKeysParams, OkResult, RpcError};

use super::super::dispatcher::{HandlerFuture, RpcContext, RpcResult};

/// Handler for `input/keys` method.
///
/// Parses vim notation keys and acknowledges receipt.
/// Full key processing will be implemented in a future iteration.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "input/keys", "params": {"keys": "iHello<Esc>"}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}
/// ```
#[must_use]
pub fn input_keys(_ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move { handle_input_keys(params) })
}

fn handle_input_keys(params: serde_json::Value) -> RpcResult {
    // Parse params to validate the request
    let _params: InputKeysParams =
        serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

    // For MVP, just acknowledge receipt
    // Full key processing will be implemented later
    Ok(serde_json::to_value(OkResult::new()).expect("OkResult serialization should never fail"))
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
    async fn test_input_keys_handler_valid() {
        let session = test_session();
        let ctx = RpcContext { session };

        let params = serde_json::json!({"keys": "j"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_input_keys_handler_invalid_params() {
        let session = test_session();
        let ctx = RpcContext { session };

        // Missing required field
        let params = serde_json::json!({});
        let result = input_keys(ctx, params).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_input_keys_handler_multiple_keys() {
        let session = test_session();
        let ctx = RpcContext { session };

        let params = serde_json::json!({"keys": "jjk"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_ok());
    }
}
