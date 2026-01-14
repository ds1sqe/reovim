//! Session struct with thread-safe state access.
//!
//! A session is a named editing context (like tmux sessions). Multiple clients
//! can attach to the same session and share editor state.

use std::sync::Arc;

use tokio::sync::RwLock;

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_input::KeySequence,
    reovim_kernel::api::v1::{CommandId, KernelContext, ModeId},
};

use {
    super::{id::SessionId, state::SessionState},
    crate::{
        client::ClientRegistry,
        registry::{CommandRegistry, KeyLookupResult, KeymapRegistry, ModeRegistry},
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
    pub fn new(id: SessionId, kernel: KernelContext, initial_mode: ModeId) -> Arc<Self> {
        Arc::new(Self {
            id,
            state: RwLock::new(SessionState::new(kernel, initial_mode)),
            clients: ClientRegistry::new(),
        })
    }

    /// Create a new session with pre-populated registries.
    #[must_use]
    pub fn with_registries(
        id: SessionId,
        kernel: KernelContext,
        initial_mode: ModeId,
        mode_registry: ModeRegistry,
        command_registry: CommandRegistry,
        keymap_registry: KeymapRegistry,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            state: RwLock::new(SessionState::with_registries(
                kernel,
                initial_mode,
                mode_registry,
                command_registry,
                keymap_registry,
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
    pub async fn execute_command(
        &self,
        id: &CommandId,
        args: &CommandContext,
    ) -> Option<CommandResult> {
        self.state.write().await.execute_command(id, args)
    }

    /// Check if the current mode accepts character input.
    ///
    /// Acquires a read lock on the session state.
    pub async fn mode_accepts_char_input(&self) -> bool {
        self.state.read().await.mode_accepts_char_input()
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
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    #[tokio::test]
    async fn test_session_new() {
        let session =
            Session::new(SessionId::new("test"), KernelContext::default(), test_mode_id());

        assert_eq!(session.id().name(), "test");
        assert!(session.is_running().await);
    }

    #[tokio::test]
    async fn test_session_current_mode() {
        let session =
            Session::new(SessionId::new("test"), KernelContext::default(), test_mode_id());

        let mode = session.current_mode().await;
        assert_eq!(mode.name(), "normal");
    }

    #[tokio::test]
    async fn test_session_quit() {
        let session =
            Session::new(SessionId::new("test"), KernelContext::default(), test_mode_id());

        assert!(session.is_running().await);
        session.request_quit().await;
        assert!(!session.is_running().await);
    }

    #[tokio::test]
    async fn test_session_with_state() {
        let session =
            Session::new(SessionId::new("test"), KernelContext::default(), test_mode_id());

        let is_empty = session
            .with_state(|state| state.keymap_registry.is_empty())
            .await;
        assert!(is_empty);
    }
}
