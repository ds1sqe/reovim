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
use {reovim_protocol::v3::Notification, tokio::sync::broadcast};

#[cfg(feature = "grpc")]
use super::CaptureTracker;
#[cfg(feature = "grpc")]
use super::PresenceService;
use {
    reovim_subsys_input::InputEvent,
    reovim_subsys_session::{DomainDriver, ExtensionMap},
};

// active_buffer is now domain-owned; only reachable via domain driver (#753).

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

    /// Domain driver for input dispatch and state queries (#753).
    ///
    /// When `Some`, `dispatch_input_for_client` delegates to the domain driver.
    /// When `None`, input is dropped with a warning.
    /// Set by the runner at session creation via [`Session::set_domain_driver`].
    domain_driver: RwLock<Option<Arc<dyn DomainDriver>>>,

    /// Server-side projection cache (#753).
    ///
    /// Stores versioned domain projections per (ClientId, ProjectionTag).
    /// Disjoint from `state`/`clients` locks — acquired independently after dispatch.
    projection_store: RwLock<super::projection_store::ProjectionStore>,

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
            projection_store: RwLock::new(super::projection_store::ProjectionStore::new()),
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
    ///     mode_registry, command_registry, keymap_registry,
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
            projection_store: RwLock::new(super::projection_store::ProjectionStore::new()),
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
    /// After this is called, `dispatch_input_for_client` delegates to the domain
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
    // Domain Driver Queries (#753 E2)
    // =========================================================================

    /// Get the active buffer for a client via domain driver (#753 E3).
    ///
    /// active_buffer is now exclusively owned by the domain driver.
    /// Returns `None` when no domain driver is wired or driver has no buffer.
    #[must_use]
    pub fn active_buffer_for_client(
        &self,
        client_id: ClientId,
    ) -> Option<reovim_kernel::api::v1::BufferId> {
        let driver = self.domain_driver.read();
        let driver = driver.as_ref()?;
        let subsys_id = reovim_subsys_session::ClientId::new(client_id.as_usize());
        driver.active_buffer(subsys_id)
    }

    /// Get the compositor generation for a client (#753 E5).
    ///
    /// Returns the monotonic generation counter from the client's compositor.
    /// Used for poll-based layout change detection after dispatch.
    #[must_use]
    pub fn client_compositor_generation(&self, client_id: ClientId) -> u64 {
        let clients = self.clients.read();
        clients
            .get(&client_id)
            .and_then(|c| c.state.compositor.as_ref())
            .map(|c| c.generation())
            .unwrap_or(0)
    }

    // =========================================================================
    // Projection Store (#753)
    // =========================================================================

    /// Access the projection store for reading/writing.
    ///
    /// Lock ordering: acquire AFTER releasing `clients`/`state` locks.
    /// The projection store lock is disjoint — no overlap with other session locks.
    pub fn projection_store(&self) -> &RwLock<super::projection_store::ProjectionStore> {
        &self.projection_store
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
    /// Text-domain state (windows, active_buffer, etc.) is managed by the
    /// domain driver via `on_client_added` (#753 E3).
    pub fn add_client_with_metadata(&self, client_id: ClientId, metadata: super::ClientMetadata) {
        use reovim_kernel::api::v1::ModeStack;

        // Initialize with session's home mode. After this the per-client mode stack is used.
        let state = self.state.read();
        let home_mode = state.home_mode().clone();
        // #474/#753 E6: Clone shared compositor for per-client layout notifications.
        let compositor = state.compositor.as_ref().map(|c| c.boxed_clone());
        drop(state);

        tracing::debug!(
            %client_id,
            mode_module = %home_mode.module(),
            mode_name = %home_mode.name(),
            has_compositor = compositor.is_some(),
            "Initializing client with home mode"
        );

        let mode_stack = ModeStack::new(home_mode);
        let mut client = Client::with_mode_stack(client_id, metadata, mode_stack);

        // #474: Set per-client compositor for layout notification generation.
        client.state.compositor = compositor;

        self.clients.add_client_with_state(client);

        // Notify domain driver so it can initialize its per-client text-domain state.
        if let Some(ref driver) = *self.domain_driver.read() {
            let subsys_id = reovim_subsys_session::ClientId::new(client_id.as_usize());
            driver.on_client_added(subsys_id);
        }
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
    /// `dispatch_input_for_client`.
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

    // resolve_key_for_client: REMOVED (#753 E6).
    // Key resolution routes through DomainDriver via dispatch_input_for_client.

    /// Dispatch an opaque InputEvent for a client (domain-neutral path).
    ///
    /// This is the sole server-side dispatch entry point. The caller encodes its
    /// input source into `InputEvent`, then this method routes it to
    /// `driver.dispatch_input()`.
    ///
    /// # Lock ordering
    ///
    /// Acquires `clients` (write) → `state` (write). The domain driver's
    /// internal locks are disjoint from these — no deadlock risk.
    #[allow(clippy::unused_async, clippy::significant_drop_tightening)]
    pub async fn dispatch_input_for_client(
        &self,
        client_id: ClientId,
        event: &InputEvent,
    ) -> Option<reovim_subsys_session::DispatchResult> {
        let driver = self.domain_driver.read().clone();
        let Some(driver) = driver else {
            tracing::warn!(%client_id, "dispatch_input_for_client: no domain driver wired, input dropped");
            return None;
        };

        let mut clients = self.clients.write();
        let target_id = ClientDirectory::find_input_target(&clients, client_id)?;
        let target_client = clients.get_mut(&target_id)?;
        let client_ext = &mut target_client.state.extensions;

        let subsys_client_id = reovim_subsys_session::ClientId::new(target_id.as_usize());

        let mut state = self.state.write();
        let shared_ext = &mut state.app.extensions;

        Some(driver.dispatch_input(subsys_client_id, event, client_ext, shared_ext))
    }

    /// Try `on_command_complete` with per-client state (#471, #477).
    ///
    /// Like `resolve_key_for_client`, but for post-command mode transitions.
    ///
    /// Stubbed: domain driver handles mode transitions internally via
    /// `dispatch_input`. The server no longer tracks driver-tier `ModeTransition`
    /// types (#753 E3 / I.7).
    #[allow(clippy::unused_async, clippy::significant_drop_tightening)]
    pub async fn try_on_command_complete_for_client(&self, client_id: ClientId) -> Option<()> {
        tracing::warn!(%client_id, "try_on_command_complete_for_client: stubbed (#753 E3)");
        None
    }

    // execute_command_for_client: REMOVED (#753 E6).
    // Command execution routes through DomainDriver via dispatch_input_for_client.

    /// Get the current mode for a specific client (#471).
    ///
    /// Returns the mode from the routed domain driver for the effective input
    /// target (Independent/Sharing) or `None` for Following clients.
    #[must_use]
    pub fn client_current_mode(
        &self,
        client_id: ClientId,
    ) -> Option<reovim_kernel::api::v1::ModeId> {
        let clients = self.clients.read();

        // Find the target client ID based on relation
        let target_id = ClientDirectory::find_input_target(&clients, client_id)?;
        drop(clients);

        let driver = self.domain_driver.read();
        let driver = driver.as_ref()?;
        driver.current_mode(reovim_subsys_session::ClientId::new(target_id.as_usize()))
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
    /// Content is stored as opaque bytes — the domain driver interprets the format.
    #[must_use]
    pub fn get_session_register(&self, key: char) -> Option<Vec<u8>> {
        let state = self.state.read();
        state.session_registers.get(&key).cloned()
    }

    /// Set a session-shared register.
    ///
    /// The content is immediately visible to all clients in this session.
    /// Content is stored as opaque bytes — the domain driver interprets the format.
    pub fn set_session_register(&self, key: char, content: Vec<u8>) {
        let mut state = self.state.write();
        state.session_registers.insert(key, content);
    }

    /// Read another client's history ring entry (`PeerHistory`).
    ///
    /// clipboard_history is now domain-owned (#753 E3).
    /// Returns `None` — use domain driver projections instead.
    #[must_use]
    pub fn get_peer_history(&self, _client_id: ClientId, _index: u8) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
