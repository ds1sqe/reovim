//! Session struct with thread-safe state access.
//!
//! A session is a named editing context (like tmux sessions). Multiple clients
//! can attach to the same session and share editor state.

use std::sync::Arc;

use tokio::sync::RwLock;

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_display::layout::RootCompositor,
    reovim_driver_input::KeySequence,
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{CommandId, KernelContext, ModeId},
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
        resolver_registry: reovim_module_editor::ResolverRegistry,
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
