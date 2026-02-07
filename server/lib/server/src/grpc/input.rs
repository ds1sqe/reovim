//! `InputService` gRPC implementation.
//!
//! Provides key input processing for v2 protocol clients.
//!
//! # Key Resolution
//!
//! When modules are loaded (via `Server::with_session_factory`), keys are resolved
//! through the full resolver system:
//! 1. Parse vim notation keys (e.g., `iHello<Esc>`, `<C-w>h`)
//! 2. For each key, call `SessionState::resolve_key()` which uses:
//!    - `ResolverRegistry` to find the mode's resolver
//!    - `KeymapRegistry` for keybinding lookup
//!    - `CommandRegistry` for command execution
//! 3. Handle the `ResolveResult` (execute command, insert char, mode transition, etc.)
//! 4. Emit notifications for state changes (mode, cursor, buffer modifications)
//!
//! When modules are NOT loaded (empty registries), the service falls back to
//! basic character insertion for insert-mode-like behavior.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_driver_command_types::{ArgValue, CommandContext, CommandResult},
    reovim_driver_input::{KeySequence, ModeTransition, PopResult, ResolveContext, ResolveResult},
    reovim_driver_session::api::StateChanges,
    reovim_protocol::v2::{
        KeyStatus, SendKeysRequest, SendKeysResponse, input_service_server::InputService,
    },
    tonic::{Request, Response, Status},
};

use crate::{
    grpc::{auth::require_client_id, notification_builder},
    session::{ClientEventType, ClientId, ClientRingBuffer, Session, SessionId, SessionRegistry},
};

/// gRPC `InputService` implementation.
///
/// Bridges v2 protocol key input requests to the session system.
/// Provides basic character insertion. Full vim-style key resolution
/// (resolvers, operator-pending modes, etc.) requires modules to be
/// loaded by the runner.
pub struct InputServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
}

impl InputServiceImpl {
    /// Create a new `InputService` with access to the session registry.
    #[must_use]
    pub const fn new(sessions: Arc<SessionRegistry>, default_session_id: SessionId) -> Self {
        Self {
            sessions,
            default_session_id,
        }
    }

    /// Get the default session.
    fn get_session(&self) -> Result<Arc<Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

#[tonic::async_trait]
impl InputService for InputServiceImpl {
    /// Send keys to the editor.
    ///
    /// Parses vim notation keys (e.g., `iHello<Esc>`, `<C-w>h`) and processes them.
    ///
    /// # Key Resolution
    ///
    /// When modules are loaded, keys go through the full resolver system:
    /// - `SessionState::resolve_key()` finds the appropriate mode resolver
    /// - The resolver returns a `ResolveResult` (execute, insert, transition, etc.)
    /// - This method handles each result type appropriately
    /// - State changes are accumulated and emitted as notifications
    ///
    /// When modules are NOT loaded (empty registries), falls back to character insertion.
    async fn send_keys(
        &self,
        request: Request<SendKeysRequest>,
    ) -> Result<Response<SendKeysResponse>, Status> {
        // #483 Phase 5: Token-only authentication (no body fallback)
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_id = require_client_id(token_client_id)?;

        // #483: Client must exist (created via Join()); auto-join removed
        if !session.has_client(client_id) {
            return Err(Status::failed_precondition(format!(
                "Client {client_id} not found — call Join() before sending keys"
            )));
        }

        // Check client relation for input routing
        if let Some(client) = session.get_client(client_id)
            && client.is_following()
        {
            // Following: input is ignored (read-only spectator)
            tracing::debug!(%client_id, "Input ignored for Following client");
            return Ok(Response::new(SendKeysResponse {
                ok: false,
                status: KeyStatus::NotFound.into(),
            }));
        }
        // Independent/Sharing: proceed with normal input processing
        // Note: For Sharing, input goes to target's state - handled by
        // session.resolve_key_for_client() and session.execute_command_for_client()

        // Parse vim notation keys
        let keys = KeySequence::parse(&req.keys).ok_or_else(|| {
            Status::invalid_argument(format!("Invalid key notation: {}", req.keys))
        })?;

        // Process each key through the resolver system
        let mut any_handled = false;
        let mut final_status = KeyStatus::NotFound;
        let mut accumulated_changes = StateChanges::new();

        for key in keys.as_slice() {
            // Phase #478: Log key to client ring buffer
            session.with_client_ring_buffer(client_id, |rb: &ClientRingBuffer| {
                rb.log_key(&format!("{:?}", key.code));
            });

            // Debug: Log current mode before resolution
            // Per-client state (#471): Use per-client mode - client was created above
            // Phase #479: KERNEL PANIC if client not found (invariant violation)
            let current_mode = session
                .client_current_mode(client_id)
                .expect("BUG: client just created but not found - state corruption");
            tracing::debug!(
                ?current_mode,
                %client_id,
                code = ?key.code,
                modifiers = ?key.modifiers,
                "Resolving key"
            );

            // Per-client state (#471): Try to resolve with per-client mode stack first
            // This enables multi-client mode isolation (Client A in INSERT, Client B in NORMAL)
            let resolve_result = session.resolve_key_for_client(client_id, key).await;

            // Debug: Log resolution result
            tracing::debug!(
                resolver_found = resolve_result.is_some(),
                result_type = resolve_result.as_ref().map(|(r, _)| format!("{r:?}")),
                "Key resolution result"
            );

            // Per-client state (#471): Resolver MUST exist for the current mode.
            // If no resolver is found, it's a configuration bug - panic to catch it early.
            let Some((result, changes)) = resolve_result else {
                panic!(
                    "No resolver found for mode {current_mode:?} (client_id={client_id}). \
                     This is a configuration bug - ensure modules are loaded properly \
                     and the client's mode stack is initialized with the correct ModeId."
                );
            };

            // Accumulate changes from resolution
            accumulated_changes.merge(changes);

            // Key was processed by a resolver
            // Per-client state (#471): Pass client_id for per-client mode transitions
            let (handled, result_changes) =
                Self::handle_resolve_result(&session, result, key, client_id).await;
            accumulated_changes.merge(result_changes);

            if handled {
                any_handled = true;
                final_status = KeyStatus::Executed;
            }
        }

        // Phase 11.2: Record cursor movement for active buffer
        // This ensures cursor_moved notifications have affected_buffers populated.
        // Most key operations move the cursor, so record it if any key was handled.
        #[allow(clippy::redundant_closure_for_method_calls)]
        if any_handled
            && let Some(buffer_id) = session.with_state(|s| s.active_buffer()).await
            && !accumulated_changes.affected_buffers.contains(&buffer_id)
        {
            accumulated_changes.record_cursor_move(buffer_id);
            tracing::trace!(?buffer_id, "Recorded cursor move for active buffer");
        }

        // Emit notifications for accumulated state changes
        // Phase 14 (#471): Pass client_id for cursor/selection filtering
        // Phase #486: emit_notifications is now sync (uses sync per-client state access)
        if accumulated_changes.has_changes() {
            Self::emit_notifications(&session, &accumulated_changes, client_id.as_usize() as u64);
        }

        // Return result
        Ok(Response::new(SendKeysResponse {
            ok: any_handled,
            status: final_status.into(),
        }))
    }
}

impl InputServiceImpl {
    /// Emit notifications for state changes.
    ///
    /// Converts accumulated `StateChanges` to gRPC notifications and emits them
    /// to all subscribed clients.
    ///
    /// # Arguments
    ///
    /// * `session` - The session to emit to
    /// * `changes` - State changes to convert to notifications
    /// * `client_id` - Client ID that originated these changes (for multi-client filtering)
    ///
    /// Phase #486: Now passes `&Session` directly to `build_notifications()` for per-client state.
    fn emit_notifications(session: &Session, changes: &StateChanges, client_id: u64) {
        // Phase #486: Pass session directly for per-client state access
        let notifications = notification_builder::build_notifications(changes, session, client_id);

        let notification_count = notifications.len();
        for notification in notifications {
            session.emit_notification(notification);
        }

        if notification_count > 0 {
            tracing::trace!(
                count = notification_count,
                mode_changed = changes.mode_changed,
                cursor_moved = changes.cursor_moved,
                buffer_modified = changes.buffer_modified,
                selection_changed = changes.selection_changed,
                "Emitted notifications"
            );
        }
    }

    /// Convert `ResolveContext` to `CommandContext`.
    fn resolve_to_command_context(ctx: &ResolveContext) -> CommandContext {
        let mut cmd_ctx = CommandContext::new();
        if let Some(count) = ctx.count {
            cmd_ctx.set("count", ArgValue::Count(count));
        }
        if let Some(reg) = ctx.register {
            cmd_ctx.set("register", ArgValue::Register(reg));
        }
        // Transfer metadata (ResolveContext::ArgValue -> CommandContext::ArgValue)
        for (key, value) in &ctx.metadata {
            // Note: ResolveContext uses a different ArgValue enum than CommandContext
            // For now, we skip metadata transfer - full conversion would be complex
            tracing::trace!(key, "Metadata key in resolve context (not yet transferred)");
            let _ = value;
        }
        cmd_ctx
    }

    /// Handle a `ResolveResult` from the resolver system.
    ///
    /// Returns `(handled, changes)` where:
    /// - `handled` is `true` if the key was processed successfully
    /// - `changes` contains any state changes that need notification (e.g., buffer modifications)
    ///
    /// # Arguments
    ///
    /// * `session` - The session
    /// * `result` - The resolve result to handle
    /// * `key` - The key that was resolved
    /// * `client_id` - Client ID for per-client mode transitions (#471)
    ///
    /// Note: This function uses `Box::pin` for recursive calls to handle
    /// `InjectKeys` (macro playback) without infinite future sizes.
    #[allow(clippy::too_many_lines)] // Per-client state (#471) added per-client mode sync
    fn handle_resolve_result<'a>(
        session: &'a Session,
        result: ResolveResult,
        key: &'a reovim_driver_input::KeyEvent,
        client_id: ClientId,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = (bool, StateChanges)> + Send + 'a>>
    {
        Box::pin(async move {
            let mut changes = StateChanges::new();

            match result {
                ResolveResult::Execute(cmd_id, ctx) => {
                    // Execute the command with per-client state (Phase #471)
                    tracing::debug!(?cmd_id, ?ctx.count, "Executing command from resolver");
                    let cmd_ctx = Self::resolve_to_command_context(&ctx);

                    // Track per-client mode before execution
                    // Phase #479: KERNEL PANIC if client not found (invariant violation)
                    let mode_before = session
                        .client_current_mode(client_id)
                        .expect("BUG: client just created but not found - state corruption");

                    // Phase #471: Execute command with per-client state directly.
                    // No sync bandaids needed - command operates on per-client mode/cursor.
                    let cmd_changes =
                        session.execute_command_for_client(client_id, &cmd_id, &cmd_ctx);

                    // Call per-client on_command_complete for pending operators
                    if let Some(transition) =
                        session.try_on_command_complete_for_client(client_id).await
                    {
                        Self::apply_mode_transition_for_client(session, client_id, transition)
                            .await;
                    }

                    // Merge command changes into accumulated changes
                    if let Some(cmd_changes) = cmd_changes {
                        changes.merge(cmd_changes.1);
                    }

                    // Check if per-client mode changed
                    // Phase #479: KERNEL PANIC if client not found (invariant violation)
                    let mode_after = session
                        .client_current_mode(client_id)
                        .expect("BUG: client just created but not found - state corruption");
                    if mode_before != mode_after {
                        tracing::debug!(
                            ?mode_before,
                            ?mode_after,
                            %client_id,
                            "Mode changed during command execution"
                        );
                        changes.record_mode_change();
                    }

                    (true, changes)
                }

                ResolveResult::InsertChar { char: ch, target } => {
                    // Generic Input Target routing (#482, #477)
                    // Route character based on target specified by resolver
                    // Phase #477: Use insert_char_for_client which checks per-client extensions first
                    let modified_buffer = session.insert_char_for_client(client_id, ch, target);

                    // Record buffer modification for notification
                    if let Some(buffer_id) = modified_buffer {
                        changes.record_buffer_modified(buffer_id);
                        tracing::debug!(?buffer_id, "Recorded buffer modification for InsertChar");
                    }
                    (true, changes)
                }

                ResolveResult::ModeTransition(transition) => {
                    // Per-client state (#471): Apply mode transition to per-client mode stack
                    tracing::debug!(?transition, %client_id, "Applying mode transition");
                    Self::apply_mode_transition_for_client(session, client_id, transition).await;
                    // Record mode change for notification
                    changes.record_mode_change();
                    (true, changes)
                }

                ResolveResult::Completed => {
                    // Resolver handled everything via SessionApi
                    tracing::trace!("Resolver completed action via SessionApi");
                    (true, changes)
                }

                ResolveResult::Pending => {
                    // Key accumulated, waiting for more
                    tracing::trace!(?key.code, "Key pending, waiting for more input");
                    (true, changes)
                }

                ResolveResult::NotHandled => {
                    // Resolver didn't handle the key
                    tracing::debug!(?key.code, "Key not handled by resolver");
                    (false, changes)
                }

                ResolveResult::InjectKeys {
                    keys,
                    exit_macro_playback: _,
                } => {
                    // Macro playback - inject keys into session (Epic #465 Phase 8D)
                    tracing::debug!(key_count = keys.len(), "Injecting macro keys");

                    // Process injected keys through the resolver system
                    // Per-client state (#471): Use per-client resolution
                    for injected_key in &keys {
                        let resolve_result = session
                            .resolve_key_for_client(client_id, injected_key)
                            .await;

                        if let Some((result, resolver_changes)) = resolve_result {
                            // Accumulate changes from resolver
                            changes.merge(resolver_changes);

                            // Recursively handle the result
                            // Note: We ignore nested InjectKeys to prevent infinite loops
                            if !matches!(result, ResolveResult::InjectKeys { .. }) {
                                let (_, nested_changes) = Self::handle_resolve_result(
                                    session,
                                    result,
                                    injected_key,
                                    client_id,
                                )
                                .await;
                                changes.merge(nested_changes);
                            }
                        }
                    }

                    // Note: exit_macro_playback handling requires access to VimSessionState
                    // which is a vim module type. For now, macro depth tracking is approximate.
                    // TODO(#465): Add trait-based callback mechanism for cross-module state

                    (true, changes)
                }
            }
        })
    }

    /// Apply a mode transition to per-client mode stack (#471).
    ///
    /// This modifies the per-client mode stack instead of the shared session
    /// mode stack, enabling multi-client mode isolation.
    ///
    /// # Arguments
    ///
    /// * `session` - The session
    /// * `client_id` - Client ID whose mode stack to modify
    /// * `transition` - The mode transition to apply
    #[allow(clippy::unused_async)] // Kept async for consistency with other methods
    async fn apply_mode_transition_for_client(
        session: &Session,
        client_id: ClientId,
        transition: ModeTransition,
    ) {
        // Update per-client mode stack via session's update_client_state
        let applied = session.update_client_state(client_id, |editing_state| {
            match transition.clone() {
                ModeTransition::Push { mode, context } => {
                    tracing::debug!(?mode, ?context, %client_id, "Pushing mode (per-client)");
                    editing_state.mode_stack.push(mode);
                }

                ModeTransition::Pop { result: _ } => {
                    if editing_state.mode_stack.depth() > 1 {
                        let popped = editing_state.mode_stack.pop();
                        tracing::debug!(?popped, %client_id, "Popped mode (per-client)");
                        // Note: Pop result handling would need command execution
                        // which is done separately after mode transition
                    } else {
                        tracing::warn!(%client_id, "Cannot pop last mode from stack (per-client)");
                    }
                }

                ModeTransition::Set { mode, context } => {
                    tracing::debug!(?mode, ?context, %client_id, "Setting mode (per-client)");
                    // Pop to base then set
                    while editing_state.mode_stack.depth() > 1 {
                        editing_state.mode_stack.pop();
                    }
                    editing_state.mode_stack.set(mode);
                }
            }
        });

        if !applied {
            tracing::warn!(
                %client_id,
                "Failed to apply mode transition (client not found or is follower)"
            );
        }

        // Handle pop result if provided (Phase #471: use per-client state)
        if let ModeTransition::Pop {
            result: Some(pop_result),
        } = transition
        {
            Self::handle_pop_result_for_client(session, client_id, pop_result);
        }
    }

    /// Handle a `PopResult` from a mode transition with per-client state (Phase #471).
    ///
    /// This ensures commands executed from pop results (like change operators)
    /// use per-client state for proper multi-client isolation.
    fn handle_pop_result_for_client(session: &Session, client_id: ClientId, result: PopResult) {
        match result {
            PopResult::ExecuteCommand { command, args } => {
                tracing::debug!(?command, %client_id, "Executing command from pop result (per-client)");
                let mut cmd_ctx = CommandContext::new();

                // Transfer all arguments directly (same ArgValue type on both sides)
                for (key, value) in args {
                    cmd_ctx.set(Box::leak(key.into_boxed_str()), value);
                }

                // Set active buffer ID (required for operators like delete/yank)
                #[allow(clippy::redundant_closure_for_method_calls)]
                if let Some(buffer_id) = session.with_state_sync(|state| state.active_buffer()) {
                    cmd_ctx.set_buffer_id(buffer_id);
                }

                // Phase #471/#479: Execute with per-client state, log errors to ring buffer
                if let Some((CommandResult::Error(ref e), _)) =
                    session.execute_command_for_client(client_id, &command, &cmd_ctx)
                {
                    // Phase #479: Log command failure to ring buffer (visible, not silent)
                    session.with_client_ring_buffer(client_id, |rb| {
                        rb.log_event(
                            ClientEventType::Error,
                            format!("COMMAND_FAILED: cmd={command:?} error={e}"),
                        );
                    });
                    tracing::warn!(?command, %client_id, error = %e, "Command execution failed");
                }
                // Success/Quit/ForceQuit/Detach: handled elsewhere
                // None (client not found or following): already logged in execute_command_for_client
            }

            PopResult::Cancelled => {
                tracing::trace!("Mode transition cancelled");
            }

            PopResult::Data { values } => {
                // Data result from mode - modules handle this via extensions
                tracing::trace!(?values, "Mode returned data");
            }
        }
    }

    // NOTE (#471): `fallback_char_insert()` was REMOVED.
    // NOTE (#477): `insert_char_by_target()` moved to Session::insert_char_for_client().
    //
    // The fallback was a design mistake that masked configuration bugs by silently
    // inserting characters when no resolver was found. This caused:
    // 1. Characters appearing in Normal mode (should be commands)
    // 2. ModeId mismatches going undetected (e.g., "default/normal" vs "vim/normal")
    //
    // Now, if no resolver is found for a mode, `send_keys()` panics with a clear
    // error message. This is the correct behavior - a missing resolver is a
    // configuration bug that should be fixed, not worked around.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> Arc<SessionRegistry> {
        let registry = Arc::new(SessionRegistry::new());
        let session = Arc::new(Session::new(SessionId::new("test")));
        registry.insert(&session);
        registry
    }

    /// Helper: build a request with token-authenticated `ClientId` in extensions.
    fn authed_request<T>(body: T, client_id: ClientId) -> Request<T> {
        let mut request = Request::new(body);
        request.extensions_mut().insert(client_id);
        request
    }

    #[tokio::test]
    #[ignore = "Per-client state (#471): Requires resolver registration; panics without modules"]
    async fn test_send_keys_valid_notation() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = authed_request(
            SendKeysRequest {
                keys: "abc".to_string(),
            },
            ClientId::new(1),
        );
        let response = service.send_keys(request).await;

        // Should parse successfully, but may not execute without active buffer
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_send_keys_invalid_notation() {
        let registry = test_registry();
        // Client must exist (created via Join() in production)
        registry
            .get(&SessionId::new("test"))
            .unwrap()
            .add_client(ClientId::new(1));
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = authed_request(
            SendKeysRequest {
                keys: "<Ctrl".to_string(),
            },
            ClientId::new(1),
        );
        let response = service.send_keys(request).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    #[ignore = "Per-client state (#471): Requires resolver registration; panics without modules"]
    async fn test_send_keys_special_keys() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = authed_request(
            SendKeysRequest {
                keys: "<Esc>".to_string(),
            },
            ClientId::new(1),
        );
        let response = service.send_keys(request).await;

        // Should parse but not handle (non-character key)
        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.ok); // Not handled in minimal impl
    }

    #[tokio::test]
    #[ignore = "Per-client state (#471): Requires resolver registration; panics without modules"]
    async fn test_send_keys_with_modifiers() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = authed_request(
            SendKeysRequest {
                keys: "<C-w>".to_string(),
            },
            ClientId::new(1),
        );
        let response = service.send_keys(request).await;

        // Should parse but not handle (modifier key)
        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.ok); // Not handled in minimal impl
    }

    #[tokio::test]
    async fn test_send_keys_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        // No session inserted
        let service = InputServiceImpl::new(registry, SessionId::new("nonexistent"));

        let request = authed_request(
            SendKeysRequest {
                keys: "a".to_string(),
            },
            ClientId::new(1),
        );
        let response = service.send_keys(request).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_send_keys_rejects_unauthenticated() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        // No ClientId in extensions — should be rejected
        let request = Request::new(SendKeysRequest {
            keys: "a".to_string(),
        });
        let response = service.send_keys(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
    }
}
