//! Input-related RPC handlers.
//!
//! Handlers for `input/keys` and related methods.

use {
    reovim_driver_input::KeySequence,
    reovim_protocol::v1::{InputKeysParams, InputKeysResult, RpcError},
};

use {
    super::super::dispatcher::{HandlerFuture, RpcContext},
    crate::{
        registry::KeyLookupResult,
        session::{StateSnapshot, emit_state_changes},
    },
};

/// Handler for `input/keys` method.
///
/// Parses vim notation keys and processes them through the session's keymap.
/// Returns a status indicating whether keys matched a command, are pending
/// (prefix match), or not found.
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
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true, "status": "executed"}}
/// ```
///
/// Status values:
/// - `"executed"`: Keys matched a binding and the command was executed
/// - `"pending"`: Keys are a prefix of a binding, waiting for more keys
/// - `"not_found"`: Keys don't match any binding
///
/// # Panics
///
/// This function will not panic as `InputKeysResult` serialization is infallible.
#[must_use]
pub fn input_keys(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Parse params
        let params: InputKeysParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        // Parse vim notation keys
        let keys = KeySequence::parse(&params.keys)
            .ok_or_else(|| RpcError::invalid_params("Invalid key notation"))?;

        // Capture state BEFORE processing (for notification emission)
        let before = ctx.session.with_state(StateSnapshot::capture).await;

        // Get current mode and look up keys
        let mode = ctx.session.current_mode().await;
        let lookup_result = ctx.session.lookup_keys(&mode, &keys).await;

        // Process based on lookup result
        let result = match &lookup_result {
            KeyLookupResult::Found(cmd_id) => {
                // Execute the command
                let cmd_ctx = reovim_driver_command::CommandContext::default();
                ctx.session.execute_command(cmd_id, &cmd_ctx).await;
                InputKeysResult::executed()
            }
            KeyLookupResult::Prefix => InputKeysResult::pending(),
            KeyLookupResult::NotFound => InputKeysResult::not_found(),
        };

        // Capture state AFTER and emit notifications for any changes
        let after = ctx.session.with_state(StateSnapshot::capture).await;
        emit_state_changes(&ctx.session, &before, &after).await;

        Ok(serde_json::to_value(result).expect("InputKeysResult serialization cannot fail"))
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
    async fn test_input_keys_handler_valid() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let params = serde_json::json!({"keys": "j"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("ok").unwrap().as_bool().unwrap());
        assert!(value.get("status").is_some());
    }

    #[tokio::test]
    async fn test_input_keys_handler_invalid_params() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        // Missing required field
        let params = serde_json::json!({});
        let result = input_keys(ctx, params).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_input_keys_handler_multiple_keys() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        let params = serde_json::json!({"keys": "jjk"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("ok").unwrap().as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_input_keys_handler_invalid_notation() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        // Unclosed angle bracket is invalid notation
        let params = serde_json::json!({"keys": "<Ctrl"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_input_keys_handler_unbound_returns_not_found() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        // Single key with no bindings should return "not_found"
        let params = serde_json::json!({"keys": "z"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        // With an empty keymap, all keys should be not_found
        assert_eq!(value.get("status").unwrap().as_str().unwrap(), "not_found");
    }
}
