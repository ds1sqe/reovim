//! Input-related RPC handlers.
//!
//! Handlers for `input/keys` and related methods.

use std::sync::{Arc, Mutex};

use {
    reovim_driver_input::{KeyCode, KeyEvent, KeySequence, Modifiers},
    reovim_kernel::api::v1::{EventResult, ModeId, events::ModeChanged},
    reovim_protocol::v1::{InputKeysParams, InputKeysResult, RpcError},
};

use {
    super::super::dispatcher::{HandlerFuture, RpcContext},
    crate::{
        registry::KeyLookupResult,
        session::{StateSnapshot, emit_state_changes},
    },
};

/// Check if a key is a count digit in normal/visual mode.
///
/// In Vim, digits 1-9 start a count, and 0 continues an existing count.
/// '0' alone goes to beginning of line.
fn is_count_digit(key: &KeyEvent, pending_count: Option<usize>, mode_name: &str) -> bool {
    // Only parse counts in Normal or Visual modes
    if !mode_name.starts_with("normal") && !mode_name.starts_with("visual") {
        return false;
    }

    // No modifiers allowed for count digits
    if key.modifiers.contains(Modifiers::CTRL)
        || key.modifiers.contains(Modifiers::ALT)
        || key.modifiers.contains(Modifiers::META)
    {
        return false;
    }

    // Check if it's a digit
    if let KeyCode::Char(c) = key.code
        && c.is_ascii_digit()
    {
        // 1-9 can start a count, 0 can only continue
        return c != '0' || pending_count.is_some();
    }
    false
}

/// Accumulate a digit into the pending count.
fn accumulate_count_digit(key: &KeyEvent, pending_count: &mut Option<usize>) {
    if let KeyCode::Char(c) = key.code
        && let Some(digit) = c.to_digit(10)
    {
        let digit = digit as usize;
        let current = pending_count.unwrap_or(0);
        // Cap at reasonable maximum (same as event loop)
        let new_count = current.saturating_mul(10).saturating_add(digit);
        *pending_count = Some(new_count.min(10000));
    }
}

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

        // Set up mode change tracking (like event_loop does)
        // Commands emit ModeChanged events which we need to capture and apply
        let pending_mode_change: Arc<Mutex<Option<ModeId>>> = Arc::new(Mutex::new(None));
        let pending_clone = Arc::clone(&pending_mode_change);

        // Subscribe to ModeChanged events to capture mode transitions
        // IMPORTANT: Store the subscription to keep it alive during key processing
        let _mode_subscription = ctx
            .session
            .with_state(|state| {
                state.app.kernel.event_bus.subscribe::<ModeChanged, _>(
                    0, // Priority 0 (highest)
                    move |event| {
                        if let Some(mode_id) = event.target_mode()
                            && let Ok(mut guard) = pending_clone.lock()
                        {
                            *guard = Some(mode_id.clone());
                        }
                        EventResult::Handled
                    },
                )
            })
            .await;

        // Process keys one at a time, like the event loop does
        // This allows "jj" to execute 'j' twice rather than looking for a "jj" binding
        let mut pending = KeySequence::new();
        let mut pending_count: Option<usize> = None;
        let mut any_executed = false;
        let mut final_result = KeyLookupResult::NotFound;

        for key in keys.as_slice() {
            // Get current mode (may change after each command)
            let mode = ctx.session.current_mode().await;
            let mode_name = mode.name();

            tracing::debug!(?key, mode = %mode, mode_name, "Processing key");

            // Check for count prefix (digits 1-9, or 0 if already have count)
            if is_count_digit(key, pending_count, mode_name) {
                accumulate_count_digit(key, &mut pending_count);
                continue;
            }

            pending.push(*key);

            let lookup_result = ctx.session.lookup_keys(&mode, &pending).await;
            tracing::debug!(?lookup_result, pending = ?pending.to_string(), "Keymap lookup result");

            match &lookup_result {
                KeyLookupResult::Found(cmd_id) => {
                    tracing::debug!(cmd_id = %cmd_id, "Found command, executing");

                    // Build command context with count if we have one
                    let mut cmd_ctx = reovim_driver_command::CommandContext::default();
                    if let Some(count) = pending_count.take() {
                        cmd_ctx.set("count", reovim_driver_command::ArgValue::Count(count));
                    }

                    // Set current mode name in context for commands that need to
                    // adjust behavior based on mode (e.g., motions in operator-pending)
                    cmd_ctx.set_mode_name(mode_name);

                    // Execute the command
                    if let Some(cmd_result) = ctx.session.execute_command(cmd_id, &cmd_ctx).await {
                        tracing::debug!(?cmd_result, "Command executed, handling result");
                        ctx.session.handle_command_result(cmd_result).await;
                    } else {
                        tracing::warn!(cmd_id = %cmd_id, "Command not found in registry");
                    }

                    // Apply any pending mode change from ModeChanged events
                    let new_mode = pending_mode_change
                        .lock()
                        .ok()
                        .and_then(|mut guard| guard.take());
                    if let Some(ref new_mode) = new_mode {
                        tracing::debug!(new_mode = %new_mode, "Applying mode change from event");
                        ctx.session
                            .with_state_mut(|state| {
                                state.app.mode_stack.set(new_mode.clone());
                            })
                            .await;
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
                    // No binding found - try character insertion for Insert mode
                    if let KeyCode::Char(ch) = key.code {
                        // Only insert if no modifiers (except Shift for uppercase)
                        if key.modifiers.is_empty() || key.modifiers == Modifiers::SHIFT {
                            tracing::debug!(char = %ch, mode = %mode_name, "Attempting char insert");
                            let inserted = ctx.session.insert_char(ch).await;
                            tracing::debug!(inserted = %inserted, "Char insert result");
                            if inserted {
                                any_executed = true;
                            }
                        }
                    }
                    // Clear pending and count
                    pending.clear();
                    pending_count = None;
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
