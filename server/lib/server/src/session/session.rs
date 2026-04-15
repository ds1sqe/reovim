//! Session - a named editing context.
//!
//! # Per-Client State (Phase 11.2)
//!
//! Sessions track connected clients via the `clients` map. Each client has a role:
//! - **Owner**: Owns editing state (mode, cursor, etc.)
//! - **Follow**: Read-only spectator
//! - **Share**: Bidirectional co-edit with owner
//!
//! The `presence` service tracks display preferences and sync awareness.
//! The `clients` directory tracks editing roles and state ownership.

use std::sync::Arc;

use reovim_kernel::api::v1::ServiceRegistry;

use parking_lot::RwLock;
#[cfg(feature = "grpc")]
use {reovim_protocol::v2::Notification, tokio::sync::broadcast};

#[cfg(feature = "grpc")]
use super::CaptureTracker;
#[cfg(feature = "grpc")]
use super::PresenceService;
use {
    reovim_driver_text_session::RegisterContent,
    reovim_subsys_session::{DomainDriver, ExtensionMap},
};

use super::{Client, ClientDirectory, ClientId, SessionId, SessionState};

/// Default channel capacity for notifications.
#[cfg(feature = "grpc")]
const NOTIFICATION_CHANNEL_CAPACITY: usize = 256;

/// A session is a named editing context.
///
/// Sessions hold the kernel state (buffers, options, etc.) and can have
/// multiple clients attached. Think of it like a tmux session.
///
/// # Client Management (Phase 11.2)
///
/// The session coordinates two client-facing authorities:
/// - `clients`: role and editing-state ownership (`ClientDirectory`) — access via [`Session::clients()`]
/// - `presence`: display preferences and sync awareness (`PresenceService`) — access via [`Session::presence()`]
pub struct Session {
    /// Unique session identifier.
    id: SessionId,

    /// Session state protected by `RwLock`.
    state: RwLock<SessionState>,

    /// Per-client membership and editing-relation authority.
    clients: ClientDirectory,

    /// Domain driver for key dispatch and state queries (Phase 4A).
    ///
    /// When `Some`, `dispatch_key_for_client` delegates to the domain driver.
    /// When `None`, falls back to the inline dispatch path in `SessionState`.
    /// Set by the runner at session creation via [`Session::set_domain_driver`].
    domain_driver: RwLock<Option<Arc<dyn DomainDriver>>>,

    /// Notification broadcast channel (gRPC only).
    #[cfg(feature = "grpc")]
    notification_tx: broadcast::Sender<Notification>,

    /// Capture request tracker for CLI→Server→TUI→Server→CLI relay (gRPC only).
    #[cfg(feature = "grpc")]
    capture_tracker: CaptureTracker,

    /// Multi-client presence tracking (Phase 14, gRPC only).
    #[cfg(feature = "grpc")]
    presence: PresenceService,
}

/// Log client disconnect with crash dump path.
///
/// Dumps the client's ring buffer to a file and logs the path via `pr_info!`.
/// The `pr_info!` macro requires a global logger (`OnceLock`) which is not
/// reliably initialized in unit tests, making this path untestable.
#[cfg_attr(coverage_nightly, coverage(off))]
fn log_client_disconnect(client_id: ClientId, ring_buffer: &super::ring_buffer::ClientRingBuffer) {
    use reovim_kernel::api::v1::pr_info;

    if let Some(path) = super::crash_dump::try_dump_client_to_file(client_id, ring_buffer) {
        pr_info!("CLIENT_DISCONNECT client_id={} dump={}", client_id.as_usize(), path.display());
    }
}

impl Session {
    /// Create a new session with the given ID.
    #[must_use]
    pub fn new(id: SessionId) -> Self {
        #[cfg(feature = "grpc")]
        let (notification_tx, _) = broadcast::channel(NOTIFICATION_CHANNEL_CAPACITY);

        Self {
            id,
            state: RwLock::new(SessionState::default()),
            clients: ClientDirectory::new(),
            domain_driver: RwLock::new(None),
            #[cfg(feature = "grpc")]
            notification_tx,
            #[cfg(feature = "grpc")]
            capture_tracker: CaptureTracker::new(),
            #[cfg(feature = "grpc")]
            presence: PresenceService::new(),
        }
    }

    /// Create a new session with a custom state.
    ///
    /// This allows the runner to inject module-initialized registries into sessions.
    /// The state should be created with populated registries from module initialization.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_server::{Session, SessionId, SessionState};
    ///
    /// // Create state with populated registries from modules
    /// let state = SessionState::with_registries(
    ///     kernel, initial_mode, vfs,
    ///     mode_registry, command_registry, keymap_registry, resolver_registry,
    ///     compositor,
    /// );
    ///
    /// let session = Session::from_state(SessionId::new("main"), state);
    /// ```
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Contains RwLock::new which is not const
    pub fn from_state(id: SessionId, state: SessionState) -> Self {
        #[cfg(feature = "grpc")]
        let (notification_tx, _) = broadcast::channel(NOTIFICATION_CHANNEL_CAPACITY);

        Self {
            id,
            state: RwLock::new(state),
            clients: ClientDirectory::new(),
            domain_driver: RwLock::new(None),
            #[cfg(feature = "grpc")]
            notification_tx,
            #[cfg(feature = "grpc")]
            capture_tracker: CaptureTracker::new(),
            #[cfg(feature = "grpc")]
            presence: PresenceService::new(),
        }
    }

    /// Subscribe to notifications (gRPC only).
    ///
    /// Returns a receiver for the notification broadcast channel.
    /// Used by `NotificationService` to stream updates to clients.
    #[cfg(feature = "grpc")]
    #[must_use]
    pub fn subscribe_notifications(&self) -> broadcast::Receiver<Notification> {
        self.notification_tx.subscribe()
    }

    /// Emit a notification to all subscribers (gRPC only).
    ///
    /// Sends a notification to all connected clients via the broadcast channel.
    /// If no clients are subscribed, the notification is silently dropped.
    #[cfg(feature = "grpc")]
    pub fn emit_notification(&self, notification: Notification) {
        // Ignore send errors (no subscribers)
        let _ = self.notification_tx.send(notification);
    }

    /// Get the capture tracker for CLI→Server→TUI→Server→CLI relay (gRPC only).
    #[cfg(feature = "grpc")]
    #[must_use]
    pub const fn capture_tracker(&self) -> &CaptureTracker {
        &self.capture_tracker
    }

    /// Get the presence map for multi-client tracking (Phase 14, gRPC only).
    #[cfg(feature = "grpc")]
    #[must_use]
    pub const fn presence(&self) -> &PresenceService {
        &self.presence
    }

    // =========================================================================
    // Domain Driver (Phase 4A)
    // =========================================================================

    /// Set the domain driver for this session.
    ///
    /// After this is called, `dispatch_key_for_client` delegates to the domain
    /// driver instead of using the inline dispatch path in `SessionState`.
    pub fn set_domain_driver(&self, driver: Arc<dyn DomainDriver>) {
        *self.domain_driver.write() = Some(driver);
    }

    /// Get the domain driver (if wired).
    #[must_use]
    pub fn domain_driver(&self) -> Option<Arc<dyn DomainDriver>> {
        self.domain_driver.read().clone()
    }

    // =========================================================================
    // Client Management (Phase 11.2)
    // =========================================================================

    /// Add a client to the session as independent.
    ///
    /// New clients default to independent (no relation) with their own editing state.
    /// The client's mode stack is initialized with the session's home mode.
    /// The client's windows are initialized with the session's active buffer.
    /// Call `set_client_relation()` to change to Following/Sharing.
    ///
    /// # Per-Client Windows (#471)
    ///
    /// Each client gets their own `WindowLayout` with independent cursors.
    /// If the session has an active buffer, a window is created for it.
    pub fn add_client(&self, client_id: ClientId) {
        self.add_client_with_metadata(client_id, super::ClientMetadata::default());
    }

    /// Add a client with metadata.
    ///
    /// Creates an independent client with the given metadata.
    /// This is the preferred method for gRPC handlers that have client info.
    pub fn add_client_with_metadata(&self, client_id: ClientId, metadata: super::ClientMetadata) {
        use {reovim_driver_text_session::Window, reovim_kernel::api::v1::ModeStack};

        // Per-client state (#471, #491): Initialize new clients with session's home mode
        // stored in SessionShared. After this, the client's per-client mode stack is used.
        // active_buffer: new clients get the first kernel buffer (scratch buffer).
        let state = self.state.read();
        let home_mode = state.home_mode().clone();
        let active_buffer = state.app.kernel.buffers.list().first().copied();
        // #474: Clone shared compositor for per-client ownership
        let compositor = state
            .driver_session
            .shared
            .compositor
            .as_ref()
            .map(|c| c.boxed_clone());
        drop(state); // Release lock before acquiring clients lock

        tracing::debug!(
            %client_id,
            mode_module = %home_mode.module(),
            mode_name = %home_mode.name(),
            ?active_buffer,
            has_compositor = compositor.is_some(),
            "Initializing client with home mode and per-client windows"
        );

        let mode_stack = ModeStack::new(home_mode);
        let mut client = Client::with_mode_stack(client_id, metadata, mode_stack);

        // Per-client active_buffer: initialize with first kernel buffer
        client.state.active_buffer = active_buffer;

        // #474: Set per-client compositor and create windows with matching IDs.
        // The compositor's window IDs must match the per-client WindowLayout IDs
        // so that cursor notifications (which use WindowLayout IDs) align with
        // layout notifications (which use compositor IDs).
        if let Some(compositor) = compositor {
            if let Some(buffer_id) = active_buffer {
                let (tw, th) = client.state.terminal_size;
                let screen = reovim_subsys_layout::Rect::new(0, 0, tw, th);
                let result = compositor.composite(screen);
                for p in &result.placements {
                    let window = Window::with_id_and_buffer(p.window_id, buffer_id);
                    client.state.windows.add(window);
                }
                if let Some(focused) = result.focused {
                    client.state.windows.set_active(focused);
                }
            }
            client.state.compositor = Some(compositor);
        } else if let Some(buffer_id) = active_buffer {
            // No compositor — create window with new ID (fallback)
            let window = Window::with_buffer(buffer_id);
            client.state.windows.add(window);
        }

        self.clients.add_client_with_state(client);
    }

    /// Remove a client from the session.
    ///
    /// Returns the removed client if found.
    ///
    /// # Debug Infrastructure (#481)
    ///
    /// Before removing a client, this method dumps the client's ring buffer
    /// to a file at `~/.local/share/reovim/crash/client-{id}-{timestamp}.log` for
    /// post-mortem analysis. A `CLIENT_DISCONNECT` entry is logged to the server
    /// ring buffer with the dump file path.
    pub fn remove_client(&self, client_id: ClientId) -> Option<Client> {
        self.clients.remove_client_with(client_id, |client| {
            log_client_disconnect(client_id, &client.ring_buffer);
        })
    }

    /// Execute a tick closure with mutable access to client + shared extensions (#546).
    ///
    /// Lock order: clients (write) → state (write). Same order as
    /// [`resolve_key_for_client`](Self::resolve_key_for_client).
    /// Returns `None` if client not connected or input is ignored (Following).
    ///
    /// Used by `TokioTickScheduler` for periodic state advancement.
    #[cfg(feature = "grpc")]
    pub fn with_tick_mut<F, R>(&self, client_id: ClientId, f: F) -> Option<R>
    where
        F: FnOnce(&mut ExtensionMap, &mut ExtensionMap, &ServiceRegistry) -> R,
    {
        let mut clients = self.clients.write();
        let target_id = ClientDirectory::find_input_target(&clients, client_id)?;
        let target_client = clients.get_mut(&target_id)?;

        let mut state = self.state.write();
        // Clone the Arc before taking mutable borrows on extensions (#555).
        let services = std::sync::Arc::clone(&state.app.services);
        let result = f(&mut target_client.state.extensions, &mut state.app.extensions, &services);
        drop(state);
        drop(clients);
        Some(result)
    }

    /// Get the session ID.
    #[must_use]
    pub const fn id(&self) -> &SessionId {
        &self.id
    }

    /// Get the client directory (membership and editing-relation authority).
    #[must_use]
    pub const fn clients(&self) -> &ClientDirectory {
        &self.clients
    }

    /// Execute a closure with read access to the session state.
    ///
    /// This is the primary way to query session data.
    ///
    /// Note: Currently synchronous but kept async for future I/O operations.
    #[allow(clippy::unused_async)]
    pub async fn with_state<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&SessionState) -> R,
    {
        let state = self.state.read();
        f(&state)
    }

    /// Execute a closure with write access to the session state.
    ///
    /// Use this for mutations like inserting text, moving cursor, etc.
    ///
    /// Note: Currently synchronous but kept async for future I/O operations.
    #[allow(clippy::unused_async)]
    pub async fn with_state_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut SessionState) -> R,
    {
        let mut state = self.state.write();
        f(&mut state)
    }

    /// Synchronous read access (for contexts where async isn't needed).
    pub fn with_state_sync<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&SessionState) -> R,
    {
        let state = self.state.read();
        f(&state)
    }

    /// Synchronous write access.
    pub fn with_state_mut_sync<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut SessionState) -> R,
    {
        let mut state = self.state.write();
        f(&mut state)
    }

    /// Execute a closure with combined read access to a client's extensions,
    /// shared extensions, and pre-collected opponent extension maps (#543).
    ///
    /// Acquires locks in established order: `clients` (read) first, then `state`
    /// (read). Resolves `effective_state()` for all clients to collect opponent
    /// data as driver-layer `ClientId` + `&ExtensionMap` pairs.
    ///
    /// Returns `None` if `client_id` is not connected or has no effective state.
    pub fn with_bridge_context<F, R>(&self, client_id: ClientId, f: F) -> Option<R>
    where
        F: FnOnce(
            &ExtensionMap,
            &ExtensionMap,
            &[(reovim_subsys_session::ClientId, &ExtensionMap)],
        ) -> R,
    {
        let clients = self.clients.read();
        let client = clients.get(&client_id)?;
        let own_ext = &client.effective_state(&clients)?.extensions;

        // Pre-collect opponent extension maps with driver-layer ClientId.
        // The driver crate cannot see `Client`, so we resolve here.
        let opponents: Vec<(reovim_subsys_session::ClientId, &ExtensionMap)> = clients
            .iter()
            .filter(|&(&id, _)| id != client_id)
            .filter_map(|(&id, c)| {
                c.effective_state(&clients).map(|state| {
                    (reovim_subsys_session::ClientId::new(id.as_usize()), &state.extensions)
                })
            })
            .collect();

        let state = self.state.read();
        let shared_ext = &state.app.extensions;
        let result = f(own_ext, shared_ext, &opponents);
        drop(state);
        drop(clients);
        Some(result)
    }

    // =========================================================================
    // Per-Client Key Resolution (#471)
    // =========================================================================

    /// Ensure a client's per-client windows are populated.
    ///
    /// When a client joins before any buffers exist, their windows are empty.
    /// Later, when a buffer is created (e.g., via `:e`), only the shared session
    /// windows are updated. This helper syncs per-client windows with the session's
    /// active buffer when needed.
    ///
    /// # When this matters
    ///
    /// 1. Client connects (no buffers yet) → empty windows
    /// 2. `:e filename` creates buffer → shared windows updated
    /// 3. Client tries to move cursor → per-client windows still empty!
    ///
    /// This helper fixes step 3 by creating a window for the active buffer.
    fn ensure_client_has_window(editing_state: &mut super::EditingState) {
        use reovim_driver_text_session::Window;

        // Only sync if per-client windows are empty AND client has an active buffer
        if editing_state.windows.is_empty()
            && let Some(buffer_id) = editing_state.active_buffer
        {
            // #474: If per-client compositor exists, create windows with matching IDs
            if let Some(ref compositor) = editing_state.compositor {
                let (tw, th) = editing_state.terminal_size;
                let screen = reovim_subsys_layout::Rect::new(0, 0, tw, th);
                let result = compositor.composite(screen);
                for p in &result.placements {
                    let window = Window::with_id_and_buffer(p.window_id, buffer_id);
                    editing_state.windows.add(window);
                }
                if let Some(focused) = result.focused {
                    editing_state.windows.set_active(focused);
                }
            } else {
                let window = Window::with_buffer(buffer_id);
                editing_state.windows.add(window);
            }
            tracing::debug!(?buffer_id, "Synced per-client windows with active buffer");
        }
    }

    /// Resolve a key with per-client mode stack (#471).
    ///
    /// This method provides access to both session state AND per-client mode stack,
    /// enabling multi-client mode isolation. The key is resolved using the client's
    /// mode stack instead of the shared session mode stack.
    ///
    /// # Arguments
    ///
    /// * `client_id` - Client ID to resolve for
    /// * `key` - Key event to resolve
    ///
    /// # Returns
    ///
    /// - `Some((ResolveResult, StateChanges))` - if key was resolved
    /// - `None` - if client not found, client is Following, or no resolver
    ///
    /// # Relation Behavior
    ///
    /// - **Independent**: Uses own mode stack and windows
    /// - **Following**: Returns `None` (input ignored)
    /// - **Sharing**: Uses target's mode stack and windows
    #[allow(clippy::unused_async, clippy::significant_drop_tightening)]
    pub async fn resolve_key_for_client(
        &self,
        client_id: ClientId,
        key: &reovim_subsys_input::KeyEvent,
    ) -> Option<(
        reovim_driver_text_input::ResolveResult,
        reovim_driver_text_session::api::StateChanges,
    )> {
        // Acquire both locks in consistent order to avoid deadlocks
        let mut clients = self.clients.write();
        let mut state = self.state.write();

        // Find the target client ID based on relation
        let target_id = ClientDirectory::find_input_target(&clients, client_id)?;

        // Phase #471/#477/#480: Get mutable references to per-client state
        let target_client = clients.get_mut(&target_id)?;
        let editing_state = &mut target_client.state;

        // Ensure per-client windows are populated (fixes buffer-after-client-join issue)
        Self::ensure_client_has_window(editing_state);

        // Resolve key with per-client state (#471 Phase 5: pass client_id for undo origin)
        state.resolve_key_for_client(target_id.as_usize(), editing_state.client_context(), key)
    }

    /// Dispatch a key through the full pipeline (sub-plan 05 Phase 4A).
    ///
    /// Delegates to the domain driver when wired. The domain driver internally
    /// handles resolver lookup, command execution, mode transitions, pending
    /// bindings, and `on_command_complete`. The server never sees `ResolveResult`.
    ///
    /// Falls back to the inline `SessionState::dispatch_key_for_client` path
    /// when no domain driver is wired (log a warning in that case).
    ///
    /// # Lock ordering
    ///
    /// Acquires `clients` (write) → `state` (write). The domain driver's
    /// internal locks are disjoint from these — no deadlock risk.
    ///
    /// # Returns
    ///
    /// `None` if the client doesn't exist or is a follower.
    /// `Some((handled, changes))` where `handled` indicates if the key was processed.
    #[allow(clippy::unused_async, clippy::significant_drop_tightening)]
    pub async fn dispatch_key_for_client(
        &self,
        client_id: ClientId,
        key: &reovim_subsys_input::KeyEvent,
    ) -> Option<(bool, reovim_subsys_session::ChangeSet)> {
        // Check for domain driver first (no lock needed — read is cheap).
        let driver = self.domain_driver.read().clone();

        if let Some(ref driver) = driver {
            // Domain driver path: delegate entirely to the driver.
            // Lock ordering: clients (write) → state (write). Domain driver's
            // internal locks are disjoint — no deadlock risk.
            let mut clients = self.clients.write();
            let target_id = ClientDirectory::find_input_target(&clients, client_id)?;

            let target_client = clients.get_mut(&target_id)?;
            let editing_state = &mut target_client.state;

            // Ensure per-client windows are populated (fixes buffer-after-client-join).
            Self::ensure_client_has_window(editing_state);

            let subsys_client_id = reovim_subsys_session::ClientId::new(target_id.as_usize());
            let client_ext = &mut editing_state.extensions;

            // Acquire state lock for shared extensions.
            let mut state = self.state.write();
            let shared_ext = &mut state.app.extensions;

            let cs =
                driver.dispatch_key_with_extensions(subsys_client_id, key, client_ext, shared_ext);

            // Pending text/byte edits for syntax/codec bridge: currently not
            // extracted from the domain driver path. The `ChangeSet` is domain-
            // neutral and does not carry text edits. Phase 5 moves syntax/codec
            // updates into the domain driver entirely, making this unnecessary.
            // Until Phase 5, the runner does not call `set_domain_driver`, so
            // the fallback path below handles all actual dispatch.

            return Some((true, cs));
        }

        // Fallback: no domain driver wired — use inline SessionState dispatch.
        // Expected during Phase 4A (runner does not call set_domain_driver yet).
        tracing::debug!("dispatch_key_for_client: no domain driver wired, using fallback path");
        let mut clients = self.clients.write();
        let mut state = self.state.write();

        let target_id = ClientDirectory::find_input_target(&clients, client_id)?;
        let target_client = clients.get_mut(&target_id)?;
        let editing_state = &mut target_client.state;

        Self::ensure_client_has_window(editing_state);

        state.dispatch_key_for_client(target_id.as_usize(), editing_state.client_context(), key)
    }

    /// Dispatch an opaque InputEvent for a client (domain-neutral path).
    ///
    /// This is the A4 entry point — encodes PlatformEvent → InputEvent and calls
    /// `driver.dispatch_input()`. Returns `DispatchResult` instead of `ChangeSet`.
    pub async fn dispatch_input_for_client(
        &self,
        client_id: ClientId,
        event: &reovim_subsys_input::InputEvent,
    ) -> Option<reovim_subsys_session::DispatchResult> {
        let driver = self.domain_driver.read().clone();
        let driver = driver.as_ref()?;

        let mut clients = self.clients.write();
        let target_id = ClientDirectory::find_input_target(&clients, client_id)?;
        let target_client = clients.get_mut(&target_id)?;
        let editing_state = &mut target_client.state;

        Self::ensure_client_has_window(editing_state);

        let subsys_client_id = reovim_subsys_session::ClientId::new(target_id.as_usize());
        let client_ext = &mut editing_state.extensions;

        let mut state = self.state.write();
        let shared_ext = &mut state.app.extensions;

        Some(driver.dispatch_input(subsys_client_id, event, client_ext, shared_ext))
    }

    /// Take pending text edits from the last dispatch batch (for syntax — Phase 5 stub).
    pub fn take_pending_text_edits(&self) -> Vec<reovim_driver_codec::TextBufferModified> {
        self.state.write().take_pending_text_edits()
    }

    /// Take pending byte edits from the last dispatch batch (for codec — Phase 5 stub).
    pub fn take_pending_byte_edits(
        &self,
    ) -> Vec<(reovim_kernel::api::v1::BufferId, reovim_kernel::api::v1::ByteEdit)> {
        self.state.write().take_pending_byte_edits()
    }

    /// Try `on_command_complete` with per-client state (#471, #477).
    ///
    /// Like `resolve_key_for_client`, but for post-command mode transitions.
    #[allow(clippy::unused_async, clippy::significant_drop_tightening)]
    pub async fn try_on_command_complete_for_client(
        &self,
        client_id: ClientId,
    ) -> Option<reovim_subsys_input::ModeTransition> {
        // Acquire both locks in consistent order
        let mut clients = self.clients.write();
        let mut state = self.state.write();

        // Find the target client ID based on relation
        let target_id = ClientDirectory::find_input_target(&clients, client_id)?;

        // Phase #471/#477/#480: Get mutable references to per-client state
        let target_client = clients.get_mut(&target_id)?;
        let editing_state = &mut target_client.state;

        // Ensure per-client windows are populated (fixes buffer-after-client-join issue)
        Self::ensure_client_has_window(editing_state);

        state.try_on_command_complete_for_client(
            target_id.as_usize(),
            editing_state.client_context(),
        )
    }

    /// Execute a command with per-client state (Phase #471).
    ///
    /// This enables multi-client mode isolation by operating on per-client
    /// mode and cursor state instead of shared session state.
    ///
    /// # Arguments
    ///
    /// * `client_id` - Client ID to execute for
    /// * `cmd_id` - Command ID to execute
    /// * `args` - Command arguments (count, register, etc.)
    ///
    /// # Returns
    ///
    /// - `Some((CommandResult, StateChanges))` - if command executed
    /// - `None` - if client not found, client is Following, or command not registered
    ///
    /// # Relation Behavior
    ///
    /// - **Independent**: Uses own mode stack, windows, and extensions
    /// - **Following**: Returns `None` (input ignored)
    /// - **Sharing**: Uses target's state
    #[allow(clippy::significant_drop_tightening)]
    pub fn execute_command_for_client(
        &self,
        client_id: ClientId,
        cmd_id: &reovim_kernel::api::v1::CommandId,
        args: &reovim_subsys_command_types::CommandContext,
    ) -> Option<(
        reovim_driver_command::CommandResult,
        reovim_driver_text_session::api::StateChanges,
        Vec<reovim_subsys_command_types::RuntimeSignal>,
    )> {
        // Acquire both locks in consistent order to avoid deadlocks
        let mut clients = self.clients.write();
        let mut state = self.state.write();

        // Find the target client ID based on relation
        let target_id = ClientDirectory::find_input_target(&clients, client_id)?;

        // Phase #471/#477/#480: Get mutable references to per-client state
        let target_client = clients.get_mut(&target_id)?;
        let editing_state = &mut target_client.state;

        // Ensure per-client windows are populated (fixes buffer-after-client-join issue)
        Self::ensure_client_has_window(editing_state);

        // Execute command with per-client state, passing client_id for per-client undo (#471, #515)
        state.execute_command_for_client(
            target_id.as_usize(),
            editing_state.client_context(),
            cmd_id,
            args,
        )
    }

    /// Get the current mode for a specific client (#471).
    ///
    /// Returns the mode from the client's per-client mode stack (if Independent/Sharing)
    /// or `None` for Following clients.
    #[must_use]
    pub fn client_current_mode(
        &self,
        client_id: ClientId,
    ) -> Option<reovim_kernel::api::v1::ModeId> {
        let clients = self.clients.read();

        // Find the target client ID based on relation
        let target_id = ClientDirectory::find_input_target(&clients, client_id)?;

        // Get mode from target's mode stack
        clients
            .get(&target_id)
            .map(|c| c.state.mode_stack.current().clone())
    }

    /// Get access to a client's ring buffer.
    ///
    /// Returns `None` if the client doesn't exist.
    pub fn with_client_ring_buffer<F, R>(&self, client_id: ClientId, f: F) -> Option<R>
    where
        F: FnOnce(&super::ring_buffer::ClientRingBuffer) -> R,
    {
        let clients = self.clients.read();
        clients.get(&client_id).map(|c| f(&c.ring_buffer))
    }

    /// Dump a client's ring buffer for debugging.
    ///
    /// Returns `None` if the client doesn't exist.
    #[must_use]
    pub fn dump_client_ring_buffer(&self, client_id: ClientId) -> Option<String> {
        self.with_client_ring_buffer(client_id, super::ring_buffer::ClientRingBuffer::dump)
    }

    // ========================================================================
    // Session-scoped registers (#515 Phase 5)
    // ========================================================================

    /// Get a session-shared register.
    ///
    /// Returns `None` if the register has not been set. Session registers
    /// are shared across all clients in this session.
    #[must_use]
    pub fn get_session_register(&self, key: char) -> Option<RegisterContent> {
        let state = self.state.read();
        state.session_registers.get(&key).cloned()
    }

    /// Set a session-shared register.
    ///
    /// The content is immediately visible to all clients in this session.
    pub fn set_session_register(&self, key: char, content: RegisterContent) {
        let mut state = self.state.write();
        state.session_registers.insert(key, content);
    }

    /// Read another client's history ring entry (`PeerHistory`).
    ///
    /// Returns `None` if the client doesn't exist or the index is out of range.
    /// This is a read-only operation - you cannot modify another client's history.
    #[must_use]
    pub fn get_peer_history(&self, client_id: ClientId, index: u8) -> Option<RegisterContent> {
        let clients = self.clients.read();
        clients
            .get(&client_id)
            .and_then(|client| client.state.clipboard_history.get_by_index(index))
            .cloned()
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
