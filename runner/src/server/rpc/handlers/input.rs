//! Input-related RPC handlers.
//!
//! Handlers for `input/keys` and related methods.

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeySequence, ModeTransition, Modifiers, ResolveResult,
    },
    reovim_protocol::v1::{InputKeysParams, InputKeysResult, RpcError},
};

use {
    super::super::dispatcher::{HandlerFuture, RpcContext},
    crate::session::{StateSnapshot, emit_state_changes},
};

/// Handler for `input/keys` method.
///
/// Parses vim notation keys and processes them through mode resolvers.
/// Keys are processed one at a time using the resolver registry, which
/// handles vim-style operator-pending mode and mode transitions.
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
/// - `"executed"`: At least one key was handled successfully
/// - `"pending"`: Final keys are waiting for more input
/// - `"not_found"`: No resolver handled the final key
///
/// # Panics
///
/// This function will not panic as `InputKeysResult` serialization is infallible.
#[must_use]
#[allow(clippy::too_many_lines)]
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

        // Process keys one at a time through resolvers
        let mut any_handled = false;
        let mut final_pending = false;

        for key in keys.as_slice() {
            let key_event = KeyEvent::with_modifiers(key.code, key.modifiers);

            // Resolve the key using mode resolvers
            if let Some((result, _changes)) = ctx.session.resolve_key(&key_event).await {
                match result {
                    ResolveResult::Execute(cmd_id, resolve_ctx) => {
                        // Build command context from resolve context
                        let mut cmd_ctx = reovim_driver_command::CommandContext::default();

                        if let Some(count) = resolve_ctx.count {
                            cmd_ctx.set("count", reovim_driver_command::ArgValue::Count(count));
                        }

                        if let Some(reg) = resolve_ctx.register {
                            cmd_ctx.set("register", reovim_driver_command::ArgValue::Register(reg));
                        }

                        // Transfer metadata (convert from input ArgValue to command ArgValue)
                        for (key, value) in resolve_ctx.metadata {
                            if let Some(v) = convert_input_arg_to_command_arg(&value) {
                                let static_key: &'static str = Box::leak(key.into_boxed_str());
                                cmd_ctx.set(static_key, v);
                            }
                        }

                        // Execute the command
                        if let Some(cmd_result) =
                            ctx.session.execute_command(&cmd_id, &cmd_ctx).await
                        {
                            tracing::debug!(?cmd_result, "Command executed");
                            ctx.session.handle_command_result(cmd_result).await;

                            // Per #388: Call on_command_complete for pending operators
                            // This handles the motion execution in operator-pending mode
                            if let Some(transition) = ctx
                                .session
                                .with_state_mut(try_resolver_on_command_complete)
                                .await
                            {
                                handle_mode_transition_async(&ctx, transition).await;
                            }

                            // Per #435: Execute cmdline action if cmdline was deactivated
                            // (e.g., Enter in search mode executes the search)
                            ctx.session.execute_cmdline_and_deactivate().await;
                        }

                        any_handled = true;
                        final_pending = false;
                    }

                    ResolveResult::ModeTransition(transition) => {
                        handle_mode_transition_async(&ctx, transition).await;
                        any_handled = true;
                        final_pending = false;
                    }

                    ResolveResult::Pending => {
                        final_pending = true;
                    }

                    ResolveResult::InsertChar(ch) => {
                        // Route character to appropriate target:
                        // - Cmdline buffer when cmdline is active (/, ?, :)
                        // - Document buffer otherwise (insert mode)
                        if ctx.session.is_cmdline_active().await {
                            ctx.session.cmdline_insert_char(ch).await;
                        } else {
                            ctx.session.insert_char(ch).await;
                        }
                        any_handled = true;
                        final_pending = false;
                    }

                    ResolveResult::NotHandled => {
                        // Try character insertion as fallback
                        if let KeyCode::Char(ch) = key.code
                            && (key.modifiers.is_empty() || key.modifiers == Modifiers::SHIFT)
                            && ctx.session.insert_char(ch).await
                        {
                            any_handled = true;
                        }
                        final_pending = false;
                    }

                    ResolveResult::Completed => {
                        // Resolver handled everything internally
                        any_handled = true;
                        final_pending = false;
                    }
                }
            } else {
                // No resolver for current mode - try character insertion
                tracing::debug!("No resolver for mode, trying char insert");
                if let KeyCode::Char(ch) = key.code
                    && (key.modifiers.is_empty() || key.modifiers == Modifiers::SHIFT)
                    && ctx.session.insert_char(ch).await
                {
                    any_handled = true;
                }
                final_pending = false;
            }
        }

        // Determine final status
        let result = if any_handled {
            if final_pending {
                InputKeysResult::pending()
            } else {
                InputKeysResult::executed()
            }
        } else if final_pending {
            InputKeysResult::pending()
        } else {
            InputKeysResult::not_found()
        };

        // Capture state AFTER and emit notifications for any changes
        let after = ctx.session.with_state(StateSnapshot::capture).await;
        emit_state_changes(&ctx.session, &before, &after).await;

        Ok(serde_json::to_value(result).expect("InputKeysResult serialization cannot fail"))
    })
}

/// Convert input layer `ArgValue` to command layer `ArgValue`.
///
/// These are different types because input and command layers have different needs.
fn convert_input_arg_to_command_arg(
    value: &reovim_driver_input::ArgValue,
) -> Option<reovim_driver_command::ArgValue> {
    use {reovim_driver_command::ArgValue as CmdArg, reovim_driver_input::ArgValue as InputArg};

    match value {
        InputArg::Bool(b) => Some(CmdArg::Bang(*b)),
        InputArg::Int(n) => {
            // Negative numbers don't map cleanly to command args
            usize::try_from(*n).ok().map(CmdArg::Count)
        }
        InputArg::Uint(n) => usize::try_from(*n).ok().map(CmdArg::Count),
        InputArg::Float(_) => None, // No command equivalent
        InputArg::String(s) => Some(CmdArg::String(s.clone())),
        InputArg::Char(c) => Some(CmdArg::Char(*c)),
        InputArg::Position(pos) => Some(CmdArg::Position(pos.line, pos.column)),
        InputArg::Range {
            start,
            end,
            linewise,
        } => {
            // For ranges, we store start/end as separate Position args
            // The linewise flag is stored separately
            // Command layer expects Range(start_line, end_line) for simple cases
            if *linewise {
                Some(CmdArg::Range(start.line, end.line))
            } else {
                // For characterwise ranges, store as start position
                // The range handling is more complex in commands
                Some(CmdArg::Position(start.line, start.column))
            }
        }
    }
}

/// Try to call `on_command_complete` on the current mode's resolver.
///
/// This handles the post-motion operator execution (e.g., after `w` in `dw`).
fn try_resolver_on_command_complete(
    state: &mut crate::session::SessionState,
) -> Option<ModeTransition> {
    use {
        crate::server::event_loop::RuntimeAdapter,
        reovim_driver_session::{SessionRuntime, api::CommandExecutor},
    };

    // Stub command executor
    struct StubExecutor;
    impl CommandExecutor for StubExecutor {
        fn execute(
            &self,
            _cmd: &reovim_kernel::api::v1::CommandId,
            _ctx: &reovim_driver_command::CommandContext,
            _kernel: &mut reovim_kernel::api::v1::KernelContext,
        ) -> Option<reovim_driver_command::CommandResult> {
            Some(reovim_driver_command::CommandResult::Success)
        }
    }

    let mode = state.driver_session.current_mode().clone();

    // Get resolver for current mode
    let resolver = state.resolver_registry.get(&mode)?;

    // Create runtime
    let stub_executor = StubExecutor;
    let session_runtime =
        SessionRuntime::new(&mut state.driver_session, &state.app.kernel, &stub_executor);
    let mut runtime =
        RuntimeAdapter::new(session_runtime, &mut state.app.windows, &state.app.kernel);

    // Call on_command_complete
    resolver.on_command_complete(&mut runtime, &mut state.app.extensions)
}

/// Handle mode transition asynchronously.
async fn handle_mode_transition_async(ctx: &RpcContext, transition: ModeTransition) {
    match transition {
        ModeTransition::Push { mode, context: _ } => {
            ctx.session
                .with_state_mut(|state| {
                    state.mode_stack_mut().push(mode);
                })
                .await;
        }

        ModeTransition::Pop { result } => {
            // Pop first, THEN handle result
            // This ensures commands that transition to a new mode (like change→insert)
            // operate on the base mode, not the mode being popped.
            ctx.session
                .with_state_mut(|state| {
                    state.mode_stack_mut().pop();
                })
                .await;
            if let Some(ref pop_result) = result {
                handle_pop_result_async(ctx, pop_result).await;
            }
        }

        ModeTransition::Set { mode, context: _ } => {
            ctx.session
                .with_state_mut(|state| {
                    state.mode_stack_mut().set(mode);
                })
                .await;
        }
    }
}

/// Handle pop result (`ExecuteCommand`) asynchronously.
async fn handle_pop_result_async(ctx: &RpcContext, result: &reovim_driver_session::PopResult) {
    use reovim_driver_session::PopResult;

    match result {
        PopResult::ExecuteCommand { command, args } => {
            let mut cmd_ctx = reovim_driver_command::CommandContext::new();

            // Transfer all arguments from the pop result
            for (key, value) in args {
                let static_key: &'static str = Box::leak(key.clone().into_boxed_str());
                cmd_ctx.set(static_key, value.clone());
            }

            // Set active buffer (required for operators like delete/yank)
            // Without this, operators can't find the buffer to operate on
            if let Some(buffer_id) = ctx
                .session
                .with_state(crate::session::SessionState::session_active_buffer)
                .await
            {
                cmd_ctx.set_buffer_id(buffer_id);
            }

            // Execute the command
            if let Some(cmd_result) = ctx.session.execute_command(command, &cmd_ctx).await {
                ctx.session.handle_command_result(cmd_result).await;
            }
        }

        PopResult::Cancelled | PopResult::Data { .. } => {
            // Nothing to do - operator was cancelled or data returned without command
        }
    }
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
        // With an empty keymap/no resolver, all keys should be not_found
        assert_eq!(value.get("status").unwrap().as_str().unwrap(), "not_found");
    }
}
