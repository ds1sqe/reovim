//! Command execution RPC handler.
//!
//! Handler for `command/execute` method.

use {
    reovim_driver_command::CommandContext,
    reovim_protocol::v1::{CommandExecuteParams, OkResult, RpcError},
};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Handler for `command/execute` method.
///
/// Executes a command by name with optional arguments.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "command/execute", "params": {"cmd": "quit"}}
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
pub fn command_execute(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: CommandExecuteParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        // Look up command by name in registry
        let cmd_id = ctx
            .session
            .with_state(|state| {
                // Search through registered commands by name
                state
                    .command_registry
                    .ids()
                    .find(|id| id.name() == params.cmd.as_str())
                    .cloned()
            })
            .await
            .ok_or_else(|| {
                RpcError::invalid_params(format!("Command '{}' not found", params.cmd))
            })?;

        // Build CommandContext from args (TODO: parse args into context)
        let cmd_ctx = CommandContext::default();

        // Execute the command
        ctx.session.execute_command(&cmd_id, &cmd_ctx).await;

        Ok(serde_json::to_value(OkResult::new()).expect("OkResult serialization cannot fail"))
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::server::session::{ClientId, Session, SessionId},
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
    async fn test_command_execute_invalid_params() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        // Invalid JSON params
        let result = command_execute(ctx, serde_json::json!("invalid")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_command_execute_not_found() {
        let session = test_session();
        let ctx = RpcContext {
            session,
            client_id: ClientId::new(1),
        };

        // Command that doesn't exist
        let result = command_execute(ctx, serde_json::json!({"cmd": "nonexistent_command"})).await;
        assert!(result.is_err());
    }
}
