//! Session struct with thread-safe state access.
//!
//! A session is a named editing context (like tmux sessions). Multiple clients
//! can attach to the same session and share editor state.

use std::sync::Arc;

use tokio::sync::RwLock;

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_display::layout::RootCompositor,
    reovim_driver_input::{KeySequence, ResolverRegistry},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{
        CommandId, KernelContext, ModeId,
        events::kernel::{BufferModified, Modification},
    },
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
        resolver_registry: ResolverRegistry,
        compositor: Option<Box<dyn RootCompositor>>,
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
                resolver_registry,
                compositor,
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
    /// # Context Population (Epic #415)
    ///
    /// VFS and `buffer_id` are now populated in `command_registry.execute()`,
    /// eliminating the double clone that was previously done here.
    pub async fn execute_command(
        &self,
        id: &CommandId,
        args: &CommandContext,
    ) -> Option<CommandResult> {
        let mut state = self.state.write().await;
        // Context enrichment (VFS, buffer_id) happens in command_registry.execute()
        state.execute_command(id, args)
    }

    /// Check if the current mode accepts character input.
    ///
    /// Acquires a read lock on the session state.
    pub async fn mode_accepts_char_input(&self) -> bool {
        self.state.read().await.mode_accepts_char_input()
    }

    /// Check if command-line mode is active.
    ///
    /// When cmdline is active, character input should go to the cmdline buffer
    /// instead of the document buffer.
    ///
    /// This checks the `CmdlineState` session extension set by commands like
    /// `enter-search-forward`, NOT the `app.cmdline` state which is synced
    /// by the event loop.
    ///
    /// Acquires a read lock on the session state.
    pub async fn is_cmdline_active(&self) -> bool {
        use reovim_driver_session::api::CmdlineState;
        self.state
            .read()
            .await
            .driver_session
            .extensions
            .get::<CmdlineState>()
            .is_some_and(CmdlineState::is_active)
    }

    /// Insert a character into the command-line buffer.
    ///
    /// Use this when cmdline is active to route characters to the cmdline
    /// instead of the document buffer.
    ///
    /// Insert a character into the cmdline buffer.
    ///
    /// Issue #452: Simplified - no sync needed. The `CmdlineState` extension
    /// is the SSOT for active state. This just inserts into the buffer.
    ///
    /// Acquires a write lock on the session state.
    pub async fn cmdline_insert_char(&self, ch: char) {
        self.with_state_mut(|state| {
            state.app.cmdline.insert_char(ch);
        })
        .await;
    }

    /// Execute the cmdline action and deactivate cmdline.
    ///
    /// Called when a command deactivates cmdline (e.g., Enter in search mode).
    /// This executes the search if the prompt was `/` or `?`, or executes Ex commands
    /// like `:set number`.
    ///
    /// Returns `StateChanges` that should be merged into the accumulated changes
    /// for notification emission. This is necessary because Ex commands like `:set`
    /// need to emit `OPTION_CHANGED` notifications.
    ///
    /// # Issue #452
    ///
    /// Simplified: reads prompt type from `CmdlineState` extension (SSOT).
    /// No more `PromptType` enum duplication.
    ///
    /// Acquires a write lock on the session state.
    #[allow(clippy::too_many_lines)]
    pub async fn execute_cmdline_and_deactivate(&self) -> reovim_driver_session::api::StateChanges {
        use {
            reovim_driver_search::{Direction, SearchKey, SearchProviderRegistry},
            reovim_driver_session::api::{CmdlinePrompt, CmdlineState, SearchState, StateChanges},
        };

        self.with_state_mut(|state| {
            let mut changes = StateChanges::new();

            // Check if cmdline was deactivated (extension is inactive)
            let ext_inactive = state
                .driver_session
                .extensions
                .get::<CmdlineState>()
                .is_none_or(|s| !s.is_active());

            // Only execute if we have input to process
            if ext_inactive && !state.app.cmdline.is_empty() {
                // Get prompt type from CmdlineState extension (SSOT)
                let prompt = state
                    .driver_session
                    .extensions
                    .get::<CmdlineState>()
                    .map_or(CmdlinePrompt::Command, CmdlineState::prompt);

                let input = state.app.cmdline.take();

                // Check if cmdline was cancelled (Escape) vs executed (Enter)
                let was_cancelled = state
                    .driver_session
                    .extensions
                    .get::<CmdlineState>()
                    .is_some_and(CmdlineState::was_cancelled);

                if !input.is_empty() && !was_cancelled {
                    match prompt {
                        CmdlinePrompt::SearchForward | CmdlinePrompt::SearchBackward => {
                            let direction = if prompt == CmdlinePrompt::SearchForward {
                                Direction::Forward
                            } else {
                                Direction::Backward
                            };

                            // Get search provider
                            if let Some(search_registry) =
                                state.app.services.get::<SearchProviderRegistry>()
                            {
                                // Get cursor position
                                let cursor_pos = state
                                    .session_active_buffer()
                                    .and_then(|id| state.app.kernel.buffers.get(id))
                                    .map(|b| b.read().position())
                                    .unwrap_or_default();

                                // Get buffer
                                let buffer_id = state.session_active_buffer();
                                let buffer_arc =
                                    buffer_id.and_then(|id| state.app.kernel.buffers.get(id));

                                if let Some(buffer_arc) = buffer_arc {
                                    // Find the match
                                    let search_result = {
                                        let buffer = buffer_arc.read();
                                        tracing::warn!(
                                            ?cursor_pos,
                                            ?direction,
                                            ?input,
                                            "Executing search"
                                        );
                                        search_registry.get(&SearchKey::Regex).and_then(
                                            |provider| {
                                                provider
                                                    .find_next(
                                                        &buffer, cursor_pos, &input, direction,
                                                        true,
                                                    )
                                                    .ok()
                                                    .flatten()
                                            },
                                        )
                                    };
                                    tracing::warn!(?search_result, "Search result");

                                    if let Some(m) = search_result {
                                        // Update buffer's cursor
                                        buffer_arc.write().set_position(m.start);

                                        // Update window cursor in driver_session
                                        if let Some(window) =
                                            state.driver_session.windows.active_mut()
                                        {
                                            window.cursor.line = m.start.line;
                                            window.cursor.column = m.start.column;
                                        }
                                    }

                                    // Store pattern for n/N repeat (#435)
                                    state
                                        .driver_session
                                        .extensions
                                        .get_or_insert::<SearchState>()
                                        .set(input, direction);
                                }
                            }
                        }
                        CmdlinePrompt::Command => {
                            // Execute Ex command (#445)
                            Self::execute_ex_command(&input, state, &mut changes);
                        }
                    }
                }
            }

            changes
        })
        .await
    }

    /// Execute an Ex command and record any state changes.
    ///
    /// Currently supports:
    /// - `:set <option>` - enable a boolean option (e.g., `:set number`)
    /// - `:set no<option>` - disable a boolean option (e.g., `:set nonumber`)
    /// - `:set <option>!` - toggle a boolean option
    /// - `:set <option>=<value>` - set option to value (integer/string)
    fn execute_ex_command(
        input: &str,
        state: &SessionState,
        changes: &mut reovim_driver_session::api::StateChanges,
    ) {
        let input = input.trim();

        // Parse :set command
        if let Some(args) = input
            .strip_prefix("set ")
            .or_else(|| input.strip_prefix("se "))
        {
            Self::execute_set_command(args.trim(), state, changes);
        } else if input == "set" || input == "se" {
            // :set without arguments - show all options (not implemented)
            tracing::debug!("Ex command ':set' without arguments (show all) not implemented");
        } else {
            tracing::debug!(?input, "Unknown Ex command");
        }
    }

    /// Execute a :set command with the given arguments.
    ///
    /// Syntax:
    /// - `set option` - enable boolean option
    /// - `set nooption` - disable boolean option
    /// - `set option!` - toggle boolean option
    /// - `set option=value` - set option value
    fn execute_set_command(
        args: &str,
        state: &SessionState,
        changes: &mut reovim_driver_session::api::StateChanges,
    ) {
        use reovim_kernel::api::v1::{OptionScopeId, OptionValue};

        // Handle multiple space-separated options
        for arg in args.split_whitespace() {
            // Parse the option syntax
            if let Some(name) = arg.strip_prefix("no") {
                // :set nooption - disable boolean
                if state
                    .app
                    .kernel
                    .options
                    .get(name, OptionScopeId::Global)
                    .is_some()
                {
                    let value = OptionValue::Bool(false);
                    if state
                        .app
                        .kernel
                        .options
                        .set(name, value.clone(), OptionScopeId::Global)
                        .is_ok()
                    {
                        changes.record_global_option_change(name, value);
                        tracing::debug!(?name, "Disabled option");
                    }
                } else {
                    tracing::debug!(?name, "Unknown option in :set no<option>");
                }
            } else if let Some(name) = arg.strip_suffix('!') {
                // :set option! - toggle boolean
                if let Some(current) = state.app.kernel.options.get(name, OptionScopeId::Global) {
                    if let OptionValue::Bool(b) = current {
                        let value = OptionValue::Bool(!b);
                        if state
                            .app
                            .kernel
                            .options
                            .set(name, value.clone(), OptionScopeId::Global)
                            .is_ok()
                        {
                            changes.record_global_option_change(name, value);
                            tracing::debug!(?name, toggled_to = !b, "Toggled option");
                        }
                    } else {
                        tracing::debug!(?name, "Cannot toggle non-boolean option");
                    }
                } else {
                    tracing::debug!(?name, "Unknown option in :set <option>!");
                }
            } else if let Some((name, value_str)) = arg.split_once('=') {
                // :set option=value
                Self::set_option_value(name, value_str, state, changes);
            } else {
                // :set option - enable boolean (or query, not implemented)
                let name = arg;
                if state
                    .app
                    .kernel
                    .options
                    .get(name, OptionScopeId::Global)
                    .is_some()
                {
                    let value = OptionValue::Bool(true);
                    if state
                        .app
                        .kernel
                        .options
                        .set(name, value.clone(), OptionScopeId::Global)
                        .is_ok()
                    {
                        changes.record_global_option_change(name, value);
                        tracing::debug!(?name, "Enabled option");
                    }
                } else {
                    tracing::debug!(?name, "Unknown option in :set <option>");
                }
            }
        }
    }

    /// Set an option to a specific value (for :set option=value syntax).
    fn set_option_value(
        name: &str,
        value_str: &str,
        state: &SessionState,
        changes: &mut reovim_driver_session::api::StateChanges,
    ) {
        use reovim_kernel::api::v1::{OptionScopeId, OptionValue};

        // Try to infer the type from current option value
        if let Some(current) = state.app.kernel.options.get(name, OptionScopeId::Global) {
            let new_value = match current {
                OptionValue::Bool(_) => {
                    // Parse boolean: true/false, yes/no, 1/0
                    match value_str.to_lowercase().as_str() {
                        "true" | "yes" | "1" => Some(OptionValue::Bool(true)),
                        "false" | "no" | "0" => Some(OptionValue::Bool(false)),
                        _ => None,
                    }
                }
                OptionValue::Integer(_) => value_str.parse::<i64>().ok().map(OptionValue::Integer),
                OptionValue::String(_) => Some(OptionValue::String(value_str.to_string())),
                OptionValue::Choice { choices, .. } => {
                    if choices.contains(&value_str.to_string()) {
                        Some(OptionValue::Choice {
                            value: value_str.to_string(),
                            choices,
                        })
                    } else {
                        tracing::debug!(?name, ?value_str, ?choices, "Invalid choice for option");
                        None
                    }
                }
            };

            if let Some(value) = new_value {
                if state
                    .app
                    .kernel
                    .options
                    .set(name, value.clone(), OptionScopeId::Global)
                    .is_ok()
                {
                    changes.record_global_option_change(name, value);
                    tracing::debug!(?name, ?value_str, "Set option value");
                }
            } else {
                tracing::debug!(?name, ?value_str, "Invalid value for option type");
            }
        } else {
            tracing::debug!(?name, "Unknown option in :set <option>=<value>");
        }
    }

    /// Insert a character at the cursor position in the active buffer.
    ///
    /// This is the fallback behavior for unmatched keys in modes that accept
    /// character input (like Insert mode). Returns `true` if the character
    /// was inserted, `false` if not (no active buffer, mode doesn't accept input).
    ///
    /// Acquires a write lock on the session state.
    /// Emits `BufferModified` event for auto-pair and other modules.
    pub async fn insert_char(&self, ch: char) -> bool {
        use reovim_driver_input::FallbackContext;

        self.with_state_mut(|state| {
            // Check if current mode accepts character input
            if !state.mode_accepts_char_input() {
                return false;
            }

            // Get active buffer (from driver_session SSOT)
            let Some(buffer_id) = state.session_active_buffer() else {
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

            // Emit BufferModified event for auto-pair and other modules (#440)
            #[allow(clippy::cast_possible_truncation)]
            {
                let handler_count = state.app.kernel.event_bus.handler_count::<BufferModified>();
                let result = state.app.kernel.event_bus.emit(BufferModified {
                    buffer_id: buffer_id.as_usize() as u64,
                    modification: Modification::Insert {
                        start: (cursor_before.line as u32, cursor_before.column as u32),
                        text: ch.to_string(),
                    },
                });
                tracing::trace!(
                    char = %ch,
                    handlers = handler_count,
                    result = ?result,
                    "BufferModified event emitted for insert_char"
                );
            }

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

    /// Resolve a key event using the resolver registry.
    ///
    /// This method uses mode resolvers to handle vim-style key resolution:
    /// - Operator interception (d, y, c → operator-pending mode)
    /// - Motion handling in operator-pending mode
    /// - Mode-specific policy application
    ///
    /// # Returns
    ///
    /// - `Some((ResolveResult, StateChanges))` - if a resolver handled the key
    /// - `None` - if no resolver is registered for the current mode
    pub async fn resolve_key(
        &self,
        key: &reovim_driver_input::KeyEvent,
    ) -> Option<(reovim_driver_input::ResolveResult, reovim_driver_session::api::StateChanges)>
    {
        self.with_state_mut(|state| state.resolve_key(key)).await
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
        }
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
