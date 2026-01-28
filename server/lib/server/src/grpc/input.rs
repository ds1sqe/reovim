//! `InputService` gRPC implementation.
//!
//! Provides key input processing for v2 protocol clients.
//!
//! # Key Resolution
//!
//! When modules are loaded (via `Server::with_session_factory`), keys are resolved
//! through the full resolver system:
//! 1. Parse vim notation keys (e.g., "iHello<Esc>", "<C-w>h")
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
    reovim_driver_command_types::{ArgValue, CommandContext},
    reovim_driver_input::{
        KeyCode, KeySequence, ModeTransition, Modifiers, PopResult, ResolveContext, ResolveResult,
    },
    reovim_driver_session::api::StateChanges,
    reovim_protocol::v2::{
        KeyStatus, SendKeysRequest, SendKeysResponse, input_service_server::InputService,
    },
    tonic::{Request, Response, Status},
};

use crate::{
    grpc::notification_builder,
    session::{Session, SessionId, SessionRegistry, SessionState},
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
    /// Parses vim notation keys (e.g., "iHello<Esc>", "<C-w>h") and processes them.
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
        let req = request.into_inner();
        let session = self.get_session()?;

        // Parse vim notation keys
        let keys = KeySequence::parse(&req.keys).ok_or_else(|| {
            Status::invalid_argument(format!("Invalid key notation: {}", req.keys))
        })?;

        // Process each key through the resolver system
        let mut any_handled = false;
        let mut final_status = KeyStatus::NotFound;
        let mut accumulated_changes = StateChanges::new();

        for key in keys.as_slice() {
            // Try to resolve the key through the full resolver system
            let resolve_result = session.with_state_mut(|state| state.resolve_key(key)).await;

            if let Some((result, changes)) = resolve_result {
                // Accumulate changes from resolution
                accumulated_changes.merge(changes);

                // Key was processed by a resolver
                let handled = Self::handle_resolve_result(&session, result, key).await;
                if handled {
                    any_handled = true;
                    final_status = KeyStatus::Executed;
                }
            } else {
                // No resolver found - fall back to legacy character insertion
                let handled = Self::fallback_char_insert(&session, key).await;
                if handled {
                    any_handled = true;
                    final_status = KeyStatus::Executed;
                } else {
                    tracing::debug!(
                        code = ?key.code,
                        modifiers = ?key.modifiers,
                        "Key not handled (no resolver, not insertable)"
                    );
                }
            }
        }

        // Emit notifications for accumulated state changes
        if accumulated_changes.has_changes() {
            Self::emit_notifications(&session, &accumulated_changes).await;
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
    async fn emit_notifications(session: &Session, changes: &StateChanges) {
        let notifications = session
            .with_state(|state| notification_builder::build_notifications(changes, state))
            .await;

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
    /// Returns `true` if the key was handled successfully.
    async fn handle_resolve_result(
        session: &Session,
        result: ResolveResult,
        key: &reovim_driver_input::KeyEvent,
    ) -> bool {
        match result {
            ResolveResult::Execute(cmd_id, ctx) => {
                // Execute the command
                tracing::debug!(?cmd_id, ?ctx.count, "Executing command from resolver");
                let cmd_ctx = Self::resolve_to_command_context(&ctx);
                session
                    .with_state_mut(|state| {
                        let _ = state.execute_command(&cmd_id, &cmd_ctx);
                    })
                    .await;
                true
            }

            ResolveResult::InsertChar(ch) => {
                // Insert character at cursor
                session
                    .with_state_mut(|state| {
                        Self::insert_char_into_state(state, ch);
                    })
                    .await;
                true
            }

            ResolveResult::ModeTransition(transition) => {
                // Apply mode transition
                session
                    .with_state_mut(|state| {
                        Self::apply_mode_transition(state, transition);
                    })
                    .await;
                true
            }

            ResolveResult::Completed => {
                // Resolver handled everything via SessionApi
                tracing::trace!("Resolver completed action via SessionApi");
                true
            }

            ResolveResult::Pending => {
                // Key accumulated, waiting for more
                tracing::trace!(?key.code, "Key pending, waiting for more input");
                true
            }

            ResolveResult::NotHandled => {
                // Resolver didn't handle the key
                tracing::debug!(?key.code, "Key not handled by resolver");
                false
            }
        }
    }

    /// Apply a mode transition to the session state.
    fn apply_mode_transition(state: &mut SessionState, transition: ModeTransition) {
        match transition {
            ModeTransition::Push { mode, context } => {
                tracing::debug!(?mode, ?context, "Pushing mode");
                state.driver_session.mode_stack.push(mode);
                // Note: TransitionContext is used by resolvers via on_mode_enter()
                // For now, resolvers get context from their own state
                let _ = context;
            }

            ModeTransition::Pop { result } => {
                if state.driver_session.mode_stack.depth() > 1 {
                    let popped = state.driver_session.mode_stack.pop();
                    tracing::debug!(?popped, "Popped mode");

                    // Handle pop result if provided
                    if let Some(pop_result) = result {
                        Self::handle_pop_result(state, pop_result);
                    }
                } else {
                    tracing::warn!("Cannot pop last mode from stack");
                }
            }

            ModeTransition::Set { mode, context } => {
                tracing::debug!(?mode, ?context, "Setting mode");
                // Pop to base then push
                while state.driver_session.mode_stack.depth() > 1 {
                    state.driver_session.mode_stack.pop();
                }
                // Replace current mode
                state.driver_session.mode_stack.set(mode);
                // Note: TransitionContext is used by resolvers via on_mode_enter()
                let _ = context;
            }
        }
    }

    /// Handle a `PopResult` from a mode transition.
    fn handle_pop_result(state: &mut SessionState, result: PopResult) {
        match result {
            PopResult::ExecuteCommand { command, args } => {
                tracing::debug!(?command, "Executing command from pop result");
                // Convert HashMap<String, ArgValue> to CommandContext
                let mut cmd_ctx = CommandContext::new();
                for (key, value) in args {
                    // Convert driver session ArgValue to command-types ArgValue
                    // For now, we just handle the common cases
                    match value {
                        reovim_driver_command_types::ArgValue::Count(c) => {
                            cmd_ctx.set(Box::leak(key.into_boxed_str()), ArgValue::Count(c));
                        }
                        reovim_driver_command_types::ArgValue::Register(r) => {
                            cmd_ctx.set(Box::leak(key.into_boxed_str()), ArgValue::Register(r));
                        }
                        reovim_driver_command_types::ArgValue::Bang(b) => {
                            cmd_ctx.set(Box::leak(key.into_boxed_str()), ArgValue::Bang(b));
                        }
                        other => {
                            tracing::trace!(key, ?other, "Skipping arg conversion");
                        }
                    }
                }
                let _ = state.execute_command(&command, &cmd_ctx);
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

    /// Insert a character into the appropriate buffer.
    fn insert_char_into_state(state: &mut SessionState, ch: char) {
        use reovim_driver_session::api::CmdlineState;

        // Try cmdline first if in command mode
        if state.mode_accepts_char_input()
            && state
                .driver_session
                .mode_stack
                .current()
                .name()
                .contains("command")
        {
            // Use CmdlineState extension (SSOT for cmdline input)
            state
                .driver_session
                .extensions
                .get_or_insert::<CmdlineState>()
                .insert_char(ch);
            return;
        }

        // Insert into active buffer
        if let Some(buffer_id) = state.active_buffer()
            && let Some(buffer_arc) = state.buffer(buffer_id)
        {
            let _ = buffer_arc.write().insert(&ch.to_string());
        }
    }

    /// Fallback character insertion for when no resolver is available.
    ///
    /// Only handles plain characters or shift+character (for uppercase).
    async fn fallback_char_insert(session: &Session, key: &reovim_driver_input::KeyEvent) -> bool {
        if let KeyCode::Char(ch) = key.code {
            // Only handle plain characters or shift+character (for uppercase)
            if key.modifiers.is_empty() || key.modifiers == Modifiers::SHIFT {
                session
                    .with_state_mut(|state| {
                        Self::insert_char_into_state(state, ch);
                    })
                    .await;
                return true;
            }
        }
        false
    }
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

    #[tokio::test]
    async fn test_send_keys_valid_notation() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SendKeysRequest {
            keys: "abc".to_string(),
        });
        let response = service.send_keys(request).await;

        // Should parse successfully, but may not execute without active buffer
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_send_keys_invalid_notation() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SendKeysRequest {
            keys: "<Ctrl".to_string(), // Unclosed angle bracket
        });
        let response = service.send_keys(request).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_send_keys_special_keys() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SendKeysRequest {
            keys: "<Esc>".to_string(),
        });
        let response = service.send_keys(request).await;

        // Should parse but not handle (non-character key)
        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.ok); // Not handled in minimal impl
    }

    #[tokio::test]
    async fn test_send_keys_with_modifiers() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SendKeysRequest {
            keys: "<C-w>".to_string(),
        });
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

        let request = Request::new(SendKeysRequest {
            keys: "a".to_string(),
        });
        let response = service.send_keys(request).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }
}
