//! Session struct with thread-safe state access.
//!
//! A session is a named editing context (like tmux sessions). Multiple clients
//! can attach to the same session and share editor state.

use std::sync::Arc;

use tokio::sync::RwLock;

use {
    reovim_driver_command::{CommandContext, CommandResult, EditAction, UndoAction},
    reovim_driver_input::KeySequence,
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{CommandId, Edit, KernelContext, ModeId, UndoResult},
};

use {
    super::{id::SessionId, state::SessionState},
    crate::{
        module::ModuleManager,
        registry::{CommandRegistry, KeyLookupResult, KeymapRegistry, ModeRegistry},
        server::client::ClientRegistry,
    },
};

/// A named editing session with thread-safe state access.
///
/// Sessions are the core unit of state isolation in the server. Each session
/// has its own:
/// - Kernel context (buffers, events, options)
/// - Mode/command/keymap registries
/// - Runtime state (active buffer, pending keys)
/// - Client registry (connected clients for notifications)
///
/// Multiple clients can attach to the same session and share state (like tmux).
///
/// # Thread Safety
///
/// Session uses `tokio::sync::RwLock` for state access, allowing:
/// - Multiple concurrent readers (state queries)
/// - Single writer (key processing, state mutations)
///
/// The [`ClientRegistry`] uses lock-free `ArcSwap` internally, so client
/// lookup and iteration don't block state access.
///
/// This matches the lock hierarchy in `docs/reference/concurrency.md`:
/// - Level 0 (Lock-Free): Client lookup via `ArcSwap`
/// - Level 1 (Per-Session): `RwLock<SessionState>`
///
/// # Example
///
/// ```ignore
/// use runner::session::{Session, SessionId};
/// use reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId};
///
/// let session = Session::new(
///     SessionId::new("default"),
///     KernelContext::default(),
///     ModeId::new(ModuleId::new("editor"), "normal"),
/// );
///
/// // Concurrent read access
/// let mode = session.current_mode().await;
///
/// // Broadcast to all clients (lock-free)
/// for client in session.clients().iter() {
///     client.send_line(r#"{"method":"notify"}"#).await?;
/// }
/// ```
pub struct Session {
    /// Session identifier.
    id: SessionId,

    /// Session state protected by async `RwLock`.
    ///
    /// Using `tokio::sync::RwLock` because:
    /// 1. Lock may be held across `.await` points
    /// 2. Provides async-aware fair scheduling
    /// 3. Allows concurrent read access for queries
    state: RwLock<SessionState>,

    /// Connected clients registry.
    ///
    /// Uses lock-free `ArcSwap` for client lookup and broadcast iteration.
    /// Clients are added when they connect and removed on disconnect.
    clients: ClientRegistry,
}

impl Session {
    /// Create a new session with empty registries.
    #[must_use]
    pub fn new(
        id: SessionId,
        kernel: KernelContext,
        initial_mode: ModeId,
        vfs: Arc<dyn VfsDriver>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            state: RwLock::new(SessionState::new(kernel, initial_mode, vfs)),
            clients: ClientRegistry::new(),
        })
    }

    /// Create a new session with pre-populated registries.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn with_registries(
        id: SessionId,
        kernel: KernelContext,
        initial_mode: ModeId,
        vfs: Arc<dyn VfsDriver>,
        mode_registry: ModeRegistry,
        command_registry: CommandRegistry,
        keymap_registry: KeymapRegistry,
        module_registry: ModuleManager,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            state: RwLock::new(SessionState::with_registries(
                kernel,
                initial_mode,
                vfs,
                mode_registry,
                command_registry,
                keymap_registry,
                module_registry,
            )),
            clients: ClientRegistry::new(),
        })
    }

    /// Get the session ID.
    #[must_use]
    pub const fn id(&self) -> &SessionId {
        &self.id
    }

    /// Get the client registry.
    ///
    /// Use this to:
    /// - Add/remove clients on connect/disconnect
    /// - Iterate over clients for broadcast notifications
    /// - Look up specific clients by ID
    #[must_use]
    pub const fn clients(&self) -> &ClientRegistry {
        &self.clients
    }

    /// Get the current mode ID.
    ///
    /// Acquires a read lock on the session state.
    pub async fn current_mode(&self) -> ModeId {
        self.state.read().await.current_mode().clone()
    }

    /// Check if the session is still running.
    ///
    /// Acquires a read lock on the session state.
    pub async fn is_running(&self) -> bool {
        self.state.read().await.is_running()
    }

    /// Request the session to quit.
    ///
    /// Acquires a write lock on the session state.
    pub async fn request_quit(&self) {
        self.state.write().await.request_quit();
    }

    /// Request clients to detach (server continues running).
    ///
    /// Acquires a write lock on the session state.
    pub async fn request_detach(&self) {
        self.state.write().await.request_detach();
    }

    /// Look up a key sequence in the session's keymap.
    ///
    /// Acquires a read lock on the session state.
    pub async fn lookup_keys(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupResult {
        self.state.read().await.lookup_keys(mode, keys)
    }

    /// Execute a command in the session.
    ///
    /// Acquires a write lock on the session state.
    /// Returns `None` if the command isn't registered.
    ///
    /// The VFS is automatically populated in the command context from
    /// the session state, so commands have access to file operations.
    pub async fn execute_command(
        &self,
        id: &CommandId,
        args: &CommandContext,
    ) -> Option<CommandResult> {
        let mut state = self.state.write().await;

        // Create command context with VFS populated
        let mut args_with_vfs = args.clone();
        args_with_vfs.set_vfs(state.vfs.clone());

        state.execute_command(id, &args_with_vfs)
    }

    /// Check if the current mode accepts character input.
    ///
    /// Acquires a read lock on the session state.
    pub async fn mode_accepts_char_input(&self) -> bool {
        self.state.read().await.mode_accepts_char_input()
    }

    /// Insert a character at the cursor position in the active buffer.
    ///
    /// This is the fallback behavior for unmatched keys in modes that accept
    /// character input (like Insert mode). Returns `true` if the character
    /// was inserted, `false` if not (no active buffer, mode doesn't accept input).
    ///
    /// Acquires a write lock on the session state.
    pub async fn insert_char(&self, ch: char) -> bool {
        use reovim_driver_input::FallbackContext;

        self.with_state_mut(|state| {
            // Check if current mode accepts character input
            if !state.mode_accepts_char_input() {
                return false;
            }

            // Get active buffer
            let Some(buffer_id) = state.app.active_buffer() else {
                return false;
            };

            let Some(buffer_arc) = state.app.get_buffer(buffer_id) else {
                return false;
            };

            // Insert character
            let mut buffer = buffer_arc.write();
            let cursor_before = buffer.position();
            let edit = buffer.insert(&ch.to_string());
            let cursor_after = buffer.position();
            drop(buffer);

            // Accumulate edit for batched undo tracking
            state
                .app
                .accumulate_edit(buffer_id, edit, cursor_before, cursor_after);

            true
        })
        .await
    }

    /// Access session state with a read lock.
    ///
    /// For operations that need direct state access. Prefer the
    /// specific methods when possible.
    pub async fn with_state<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&SessionState) -> R,
    {
        let state = self.state.read().await;
        f(&state)
    }

    /// Access session state with a write lock.
    ///
    /// For operations that need to mutate state. Prefer the
    /// specific methods when possible.
    pub async fn with_state_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut SessionState) -> R,
    {
        let mut state = self.state.write().await;
        f(&mut state)
    }

    /// Handle a command result from command execution.
    ///
    /// This processes the result by:
    /// - Recording edits in the undo registry for `EditAction`
    /// - Applying undo/redo operations for `UndoAction`
    /// - Requesting quit for `Quit`/`ForceQuit`
    ///
    /// Returns an error message if undo/redo fails.
    pub async fn handle_command_result(&self, result: CommandResult) -> Option<String> {
        match result {
            CommandResult::Success | CommandResult::Error(_) => None,
            CommandResult::Quit | CommandResult::ForceQuit => {
                self.request_quit().await;
                None
            }
            CommandResult::Detach => {
                // Detach requests client disconnection but server continues.
                // Note: In session context, this triggers DETACH notification.
                self.request_detach().await;
                None
            }
            CommandResult::EditAction(action) => {
                self.record_edit(action).await;
                None
            }
            CommandResult::UndoAction(action) => self.handle_undo_action(action).await,
            CommandResult::UndotreeAction(_action) => {
                // TODO: handle undo tree action
                None
            }
            CommandResult::WaitingForChar(ctx) => {
                // Find-char command (f/F/t/T) needs a character argument.
                // Set char-wait state so next key press completes the motion.
                // This is handled by the message loop which checks char_wait before keymap lookup.
                self.set_char_wait(ctx).await;
                None
            }
            CommandResult::RepeatFindSame => {
                // Repeat last find-char in the same direction (;)
                // The actual motion execution is handled by the message loop.
                self.execute_repeat_find(false).await;
                None
            }
            CommandResult::RepeatFindReverse => {
                // Repeat last find-char in the opposite direction (,)
                // The actual motion execution is handled by the message loop.
                self.execute_repeat_find(true).await;
                None
            }
            CommandResult::SearchAction(action) => {
                // Search commands (/, ?, n, N, *, #, :noh) return search actions.
                // The runner handles input mode, pattern storage, and search execution.
                self.handle_search_action(action).await;
                None
            }
            CommandResult::RepeatAction => {
                // Repeat command (.) wants to replay the last repeatable command.
                // This is handled by the message loop which accesses repeat_state.
                // For now, just return None - full implementation comes in Phase 5.
                None
            }
            CommandResult::OperatorRange {
                start,
                end,
                is_linewise,
            } => {
                // Motion/text-object returned a range for the pending operator.
                // Execute the pending operator (d, y, c) with this range.
                tracing::debug!(?start, ?end, is_linewise, "OperatorRange received (session)");
                self.execute_pending_operator(start, end, is_linewise).await;
                None
            }
            CommandResult::ReselectVisual => {
                // Reselect-last (gv) command requests restoration of last visual selection.
                // The actual restoration is handled by the message loop which has access
                // to the last_visual_selection state.
                None
            }
            CommandResult::WindowAction(action) => {
                // Window management actions are handled by the runner event loop.
                // Full implementation in Phase 5 of #276.
                tracing::info!(?action, "Window action requested (session)");
                None
            }
            CommandResult::ModeAction(action) => {
                // Mode stack actions are handled by the runner event loop.
                tracing::info!(?action, "Mode action requested (session)");
                None
            }
            CommandResult::BlockInsertAction(action) => {
                // Block insert actions are handled by the runner event loop.
                tracing::info!(?action, "Block insert action requested (session)");
                None
            }
            CommandResult::EnterOperatorPending {
                operator_id,
                register,
            } => {
                // Enter operator-pending mode: set pending operator and push mode.
                use {crate::server::app::PendingOperator, reovim_kernel::api::v1::ModuleId};

                let pending = PendingOperator::new(operator_id)
                    .with_count(1) // Default count; RPC handler manages count separately
                    .with_register(register);

                let mut state = self.state.write().await;
                state.app.set_pending_operator(pending);

                // Push operator-pending mode onto the stack
                let mode_id = ModeId::new(ModuleId::new("editor"), "operator-pending");
                state.app.mode_stack.push(mode_id);
                drop(state);

                tracing::debug!(operator_id, ?register, "Entered operator-pending mode (session)");
                None
            }
        }
    }

    /// Handle a search action from search commands.
    async fn handle_search_action(&self, action: reovim_driver_command::SearchAction) {
        use {crate::server::app::SearchDirection, reovim_driver_command::SearchAction};

        match action {
            SearchAction::EnterSearchMode { direction } => {
                let dir = match direction {
                    reovim_driver_command::SearchDirection::Forward => SearchDirection::Forward,
                    reovim_driver_command::SearchDirection::Backward => SearchDirection::Backward,
                };
                self.state.write().await.app.search.start_input(dir);
            }
            SearchAction::Next => {
                let (pattern, direction) = {
                    let state = self.state.read().await;
                    (state.app.search.pattern.clone(), state.app.search.direction)
                };
                if let Some(pattern) = pattern {
                    self.execute_search(&pattern, direction).await;
                }
            }
            SearchAction::Previous => {
                let (pattern, direction) = {
                    let state = self.state.read().await;
                    let dir = match state.app.search.direction {
                        SearchDirection::Forward => SearchDirection::Backward,
                        SearchDirection::Backward => SearchDirection::Forward,
                    };
                    (state.app.search.pattern.clone(), dir)
                };
                if let Some(pattern) = pattern {
                    self.execute_search(&pattern, direction).await;
                }
            }
            SearchAction::WordUnderCursor { direction } => {
                self.execute_word_search(direction).await;
            }
            SearchAction::ClearHighlight => {
                self.state.write().await.app.search.clear_highlight();
            }
        }
    }

    /// Execute search with pattern and direction.
    async fn execute_search(&self, pattern: &str, direction: crate::server::app::SearchDirection) {
        use crate::search::{Direction, SearchEngine};

        let mut state = self.state.write().await;

        let Some(buffer_id) = state.app.active_buffer else {
            return;
        };

        let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id) else {
            return;
        };

        let search_dir = match direction {
            crate::server::app::SearchDirection::Forward => Direction::Forward,
            crate::server::app::SearchDirection::Backward => Direction::Backward,
        };

        let buffer = buffer_arc.read();
        let cursor_pos = buffer.cursor().position;

        if let Ok(Some(m)) = SearchEngine::find_next(&buffer, cursor_pos, pattern, search_dir, true)
        {
            drop(buffer);
            buffer_arc.write().set_position(m.start);
            state.app.search.highlight_active = true;
        }
    }

    /// Execute word search (* or #).
    async fn execute_word_search(&self, direction: reovim_driver_command::SearchDirection) {
        use crate::{search::SearchEngine, server::app::SearchDirection};

        // Compute word pattern and store state - all synchronous
        let result = {
            let mut state = self.state.write().await;

            let Some(buffer_id) = state.app.active_buffer else {
                return;
            };

            let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id) else {
                return;
            };

            // Get cursor and word pattern from buffer, then drop buffer lock
            let (cursor_pos, word_pattern) = {
                let buffer = buffer_arc.read();
                let cursor_pos = buffer.cursor().position;
                let word_pattern = SearchEngine::word_at_cursor(&buffer, cursor_pos);
                drop(buffer);
                (cursor_pos, word_pattern)
            };

            let Some(word_pattern) = word_pattern else {
                return;
            };

            // Store pattern and direction
            state.app.search.pattern = Some(word_pattern.clone());
            state.app.search.direction = match direction {
                reovim_driver_command::SearchDirection::Forward => SearchDirection::Forward,
                reovim_driver_command::SearchDirection::Backward => SearchDirection::Backward,
            };

            // cursor_pos is unused here but keeping for consistency
            let _ = cursor_pos;

            (word_pattern, state.app.search.direction)
        };
        // All locks released here

        // Execute search - can safely await
        self.execute_search(&result.0, result.1).await;
    }

    /// Execute a repeat find motion (; or ,).
    async fn execute_repeat_find(&self, reverse: bool) {
        use reovim_kernel::api::v1::MotionEngine;

        let mut state = self.state.write().await;

        let Some(last_find) = state.app.last_find else {
            // No previous find to repeat
            return;
        };

        let Some(buffer_id) = state.app.active_buffer else {
            return;
        };

        let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id) else {
            return;
        };

        // Get motion (same or reversed direction)
        let motion = if reverse {
            last_find.reverse_motion()
        } else {
            last_find.repeat_motion()
        };

        // Calculate and apply motion
        let buffer = buffer_arc.read();
        let target = MotionEngine::calculate(&buffer, buffer.cursor(), motion, 1);
        drop(buffer);

        if let Some(pos) = target {
            buffer_arc.write().set_position(pos);
        }
        // Note: ; and , do NOT update last_find (per Vim behavior)

        state.app.clear_pending_keys();
    }

    /// Execute the pending operator with the given range.
    ///
    /// Called when a motion command returns `CommandResult::OperatorRange`.
    /// This takes the pending operator state, executes the operator on the range,
    /// and returns to normal mode (or insert mode for change operator).
    async fn execute_pending_operator(
        &self,
        start: reovim_kernel::api::v1::Position,
        end: reovim_kernel::api::v1::Position,
        is_linewise: bool,
    ) {
        use {
            reovim_kernel::api::v1::ModuleId,
            reovim_module_operators::{
                ChangeOperator, DeleteOperator, Operator, OperatorContext, Range, YankOperator,
            },
        };

        let mut state = self.state.write().await;

        // Take the pending operator - this clears the state
        let Some(pending) = state.app.take_pending_operator() else {
            tracing::warn!("OperatorRange received but no pending operator");
            return;
        };

        // Get the active buffer
        let Some(buffer_id) = state.app.active_buffer else {
            tracing::warn!("No active buffer for operator execution");
            state.app.mode_stack.pop(); // Pop operator-pending mode
            return;
        };

        // Create the range (normalize it so start <= end)
        let range = if is_linewise {
            Range::linewise(start, end).normalized()
        } else {
            Range::new(start, end).normalized()
        };

        // Skip empty ranges (no-op)
        if range.is_empty() && !is_linewise {
            tracing::debug!("Empty range, skipping operator");
            state.app.mode_stack.pop(); // Pop operator-pending mode
            return;
        }

        // Get the operator
        let operator: Box<dyn Operator> = match pending.operator_id {
            "delete" => Box::new(DeleteOperator),
            "yank" => Box::new(YankOperator),
            "change" => Box::new(ChangeOperator),
            unknown => {
                tracing::warn!(operator = unknown, "Unknown operator");
                state.app.mode_stack.pop(); // Pop operator-pending mode
                return;
            }
        };

        // Execute the operator
        let operator_result = {
            let mut op_ctx = OperatorContext {
                kernel: &state.app.kernel,
                buffer_id,
                register: pending.register,
                count: pending.count,
            };
            operator.execute(&mut op_ctx, range)
        };

        match operator_result {
            Ok(()) => {
                tracing::debug!(
                    operator = pending.operator_id,
                    ?range,
                    "Operator executed successfully (session)"
                );

                // For change operator, enter insert mode
                if pending.operator_id == "change" {
                    let insert_mode = ModeId::new(ModuleId::new("editor"), "insert");
                    state.app.mode_stack.set(insert_mode);
                    tracing::debug!("Entered insert mode after change operator");
                } else {
                    // For other operators, pop back to normal mode
                    state.app.mode_stack.pop();
                    tracing::debug!(
                        mode = %state.app.current_mode(),
                        "Returned from operator-pending mode"
                    );
                }
            }
            Err(err) => {
                tracing::error!(error = %err, "Operator execution failed");
                state.app.mode_stack.pop(); // Pop operator-pending mode
            }
        }
    }

    /// Set pending char state for commands waiting for character input.
    ///
    /// Called when a command (f/F/t/T or r) returns `WaitingForChar`.
    /// The next key press will provide the character argument.
    ///
    /// # Panics
    ///
    /// Panics if a find-char operation (`FindForward`, `FindBackward`, `TillForward`, `TillBackward`)
    /// is missing its required `start_position`.
    pub async fn set_pending_char(&self, ctx: reovim_driver_command::CharWaitContext) {
        use {
            crate::server::app::{FindType, PendingCharOp},
            reovim_driver_command::CharWaitOp,
        };

        let mut state = self.state.write().await;

        let pending = match ctx.op_type {
            CharWaitOp::FindForward => {
                let start = ctx
                    .start_position
                    .expect("FindForward requires start position");
                PendingCharOp::from_find_type(FindType::FindForward, start)
            }
            CharWaitOp::FindBackward => {
                let start = ctx
                    .start_position
                    .expect("FindBackward requires start position");
                PendingCharOp::from_find_type(FindType::FindBackward, start)
            }
            CharWaitOp::TillForward => {
                let start = ctx
                    .start_position
                    .expect("TillForward requires start position");
                PendingCharOp::from_find_type(FindType::TillForward, start)
            }
            CharWaitOp::TillBackward => {
                let start = ctx
                    .start_position
                    .expect("TillBackward requires start position");
                PendingCharOp::from_find_type(FindType::TillBackward, start)
            }
            CharWaitOp::ReplaceChar => {
                let count = ctx.count.unwrap_or(1);
                PendingCharOp::replace_char(count)
            }
        };

        state.app.set_pending_char(pending);
    }

    /// Set char-wait state for find-char commands.
    ///
    /// DEPRECATED: Use `set_pending_char` instead. Kept for backward compatibility.
    pub async fn set_char_wait(&self, ctx: reovim_driver_command::CharWaitContext) {
        self.set_pending_char(ctx).await;
    }

    /// Record an edit action in the undo registry.
    ///
    /// Called after a command or fallback handler performs an edit.
    pub async fn record_edit(&self, action: EditAction) {
        let mut state = self.state.write().await;
        state.app.undo_registry.record(
            action.buffer_id,
            action.edits,
            action.cursor_before,
            action.cursor_after,
        );
    }

    /// Handle an undo/redo action.
    ///
    /// Retrieves edits from the undo registry and applies them to the buffer.
    /// Returns an error message if the operation fails.
    pub async fn handle_undo_action(&self, action: UndoAction) -> Option<String> {
        let mut state = self.state.write().await;

        let Some(buffer_id) = state.app.active_buffer else {
            return Some("No active buffer".to_string());
        };

        match action {
            UndoAction::Undo { count } => {
                for _ in 0..count {
                    match state.app.undo_registry.undo(buffer_id) {
                        Some(result) => {
                            Self::apply_undo_result(&state, buffer_id, result);
                        }
                        None => {
                            return Some("Already at oldest change".to_string());
                        }
                    }
                }
            }
            UndoAction::Redo { count } => {
                for _ in 0..count {
                    match state.app.undo_registry.redo(buffer_id) {
                        Some(result) => {
                            Self::apply_undo_result(&state, buffer_id, result);
                        }
                        None => {
                            return Some("Already at newest change".to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// Apply an undo result to a buffer.
    fn apply_undo_result(
        state: &SessionState,
        buffer_id: reovim_kernel::api::v1::BufferId,
        result: UndoResult,
    ) {
        let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id) else {
            return;
        };

        let mut buffer = buffer_arc.write();
        for edit in result.edits {
            match edit {
                Edit::Insert { position, text } => {
                    buffer.insert_at(position, &text);
                }
                Edit::Delete { position, text } => {
                    buffer.delete_at(position, text.chars().count());
                }
            }
        }
        buffer.set_position(result.cursor);
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_vfs::MockVfs, reovim_kernel::api::v1::ModuleId};

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(MockVfs::new())
    }

    #[tokio::test]
    async fn test_session_new() {
        let session = Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            test_mode_id(),
            test_vfs(),
        );

        assert_eq!(session.id().name(), "test");
        assert!(session.is_running().await);
    }

    #[tokio::test]
    async fn test_session_current_mode() {
        let session = Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            test_mode_id(),
            test_vfs(),
        );

        let mode = session.current_mode().await;
        assert_eq!(mode.name(), "normal");
    }

    #[tokio::test]
    async fn test_session_quit() {
        let session = Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            test_mode_id(),
            test_vfs(),
        );

        assert!(session.is_running().await);
        session.request_quit().await;
        assert!(!session.is_running().await);
    }

    #[tokio::test]
    async fn test_session_with_state() {
        let session = Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            test_mode_id(),
            test_vfs(),
        );

        let is_empty = session
            .with_state(|state| state.keymap_registry.is_empty())
            .await;
        assert!(is_empty);
    }
}
