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
/// Keys are processed one at a time, similar to the event loop, to support
/// vim-style incremental key matching (e.g., "jj" executes 'j' twice, not
/// as a two-key binding).
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
/// - `"executed"`: At least one key matched a binding and was executed
/// - `"pending"`: Final keys are a prefix of a binding, waiting for more
/// - `"not_found"`: No keys matched any binding
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

        // Process keys one at a time, like the event loop does
        // This allows "jj" to execute 'j' twice rather than looking for a "jj" binding
        let mut pending = KeySequence::new();
        let mut any_executed = false;
        let mut final_result = KeyLookupResult::NotFound;

        for key in keys.as_slice() {
            pending.push(*key);

            // Get current mode (may change after each command)
            let mode = ctx.session.current_mode().await;
            let lookup_result = ctx.session.lookup_keys(&mode, &pending).await;

            match &lookup_result {
                KeyLookupResult::Found(cmd_id) => {
                    // Execute the command
                    let cmd_ctx = reovim_driver_command::CommandContext::default();
                    if let Some(cmd_result) = ctx.session.execute_command(cmd_id, &cmd_ctx).await {
                        ctx.session.handle_command_result(cmd_result).await;
                    }
                    any_executed = true;
                    pending.clear();
                    final_result = KeyLookupResult::Found(cmd_id.clone());
                }
                KeyLookupResult::Prefix => {
                    // Keep accumulating keys
                    final_result = KeyLookupResult::Prefix;
                }
                KeyLookupResult::NotFound => {
                    // No match - clear pending and continue
                    pending.clear();
                    final_result = KeyLookupResult::NotFound;
                }
            }
        }

        // Determine final status
        let result = if any_executed {
            // At least one command was executed
            if pending.is_empty() {
                InputKeysResult::executed()
            } else {
                // Executed something but have leftover pending keys (prefix)
                InputKeysResult::pending()
            }
        } else {
            // No commands executed
            match final_result {
                KeyLookupResult::Prefix => InputKeysResult::pending(),
                _ => InputKeysResult::not_found(),
            }
        };

        // Capture state AFTER and emit notifications for any changes
        let after = ctx.session.with_state(StateSnapshot::capture).await;
        emit_state_changes(&ctx.session, &before, &after).await;

        Ok(serde_json::to_value(result).expect("InputKeysResult serialization cannot fail"))
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::server::rpc::handlers::test_utils::test_ctx};

    #[tokio::test]
    async fn test_input_keys_handler_valid() {
        let ctx = test_ctx();

        let params = serde_json::json!({"keys": "j"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("ok").unwrap().as_bool().unwrap());
        assert!(value.get("status").is_some());
    }

    #[tokio::test]
    async fn test_input_keys_handler_invalid_params() {
        let ctx = test_ctx();

        // Missing required field
        let params = serde_json::json!({});
        let result = input_keys(ctx, params).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_input_keys_handler_multiple_keys() {
        let ctx = test_ctx();

        let params = serde_json::json!({"keys": "jjk"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("ok").unwrap().as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_input_keys_handler_invalid_notation() {
        let ctx = test_ctx();

        // Unclosed angle bracket is invalid notation
        let params = serde_json::json!({"keys": "<Ctrl"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_input_keys_handler_unbound_returns_not_found() {
        let ctx = test_ctx();

        // Single key with no bindings should return "not_found"
        let params = serde_json::json!({"keys": "z"});
        let result = input_keys(ctx, params).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        // With an empty keymap, all keys should be not_found
        assert_eq!(value.get("status").unwrap().as_str().unwrap(), "not_found");
    }
}
