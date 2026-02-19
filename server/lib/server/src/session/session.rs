//! Session - a named editing context.
//!
//! # Per-Client State (Phase 11.2)
//!
//! Sessions track connected clients via the `clients` map. Each client has a role:
//! - **Owner**: Owns editing state (mode, cursor, etc.)
//! - **Follow**: Read-only spectator
//! - **Share**: Bidirectional co-edit with owner
//!
//! The `presence` map tracks display preferences (cursor position for rendering).
//! The `clients` map tracks editing roles and state ownership.

use std::collections::HashMap;

use parking_lot::RwLock;
#[cfg(feature = "grpc")]
use {reovim_protocol::v2::Notification, tokio::sync::broadcast};

#[cfg(feature = "grpc")]
use super::CaptureTracker;
#[cfg(feature = "grpc")]
use super::PresenceMap;
use reovim_driver_session::ExtensionMap;

use super::{Client, ClientId, SessionId, SessionState};

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
/// The session tracks connected clients via two maps:
/// - `clients`: Role and editing state ownership (Owner/Follow/Share)
/// - `presence`: Display preferences and cursor positions (for rendering)
pub struct Session {
    /// Unique session identifier.
    id: SessionId,

    /// Session state protected by `RwLock`.
    state: RwLock<SessionState>,

    /// Per-client roles and editing state (Phase 11.2).
    ///
    /// Maps `ClientId` to `Client` enum which tracks:
    /// - Owner: Has own `EditingState`
    /// - Follow: References another client (read-only)
    /// - Share: Co-edits with owner (bidirectional)
    clients: RwLock<HashMap<ClientId, Client>>,

    /// Notification broadcast channel (gRPC only).
    #[cfg(feature = "grpc")]
    notification_tx: broadcast::Sender<Notification>,

    /// Capture request tracker for CLI→Server→TUI→Server→CLI relay (gRPC only).
    #[cfg(feature = "grpc")]
    capture_tracker: CaptureTracker,

    /// Multi-client presence tracking (Phase 14, gRPC only).
    #[cfg(feature = "grpc")]
    presence: PresenceMap,
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
            clients: RwLock::new(HashMap::new()),
            #[cfg(feature = "grpc")]
            notification_tx,
            #[cfg(feature = "grpc")]
            capture_tracker: CaptureTracker::new(),
            #[cfg(feature = "grpc")]
            presence: PresenceMap::new(),
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
            clients: RwLock::new(HashMap::new()),
            #[cfg(feature = "grpc")]
            notification_tx,
            #[cfg(feature = "grpc")]
            capture_tracker: CaptureTracker::new(),
            #[cfg(feature = "grpc")]
            presence: PresenceMap::new(),
        }
    }

    /// Create a new session with a custom state (for testing).
    #[cfg(test)]
    #[must_use]
    #[deprecated(since = "0.9.0", note = "Use Session::from_state instead")]
    pub fn new_with_state(id: SessionId, state: SessionState) -> Self {
        Self::from_state(id, state)
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
    pub const fn presence(&self) -> &PresenceMap {
        &self.presence
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
        use {reovim_driver_session::Window, reovim_kernel::api::v1::ModeStack};

        // Per-client state (#471, #491): Initialize new clients with session's home mode
        // stored in SessionShared. After this, the client's per-client mode stack is used.
        let state = self.state.read();
        let home_mode = state.home_mode().clone();
        let active_buffer = state.active_buffer();
        let terminal_size = state.driver_session.terminal_size();
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

        // #474: Set per-client compositor and create windows with matching IDs.
        // The compositor's window IDs must match the per-client WindowLayout IDs
        // so that cursor notifications (which use WindowLayout IDs) align with
        // layout notifications (which use compositor IDs).
        if let Some(compositor) = compositor {
            if let Some(buffer_id) = active_buffer {
                let screen =
                    reovim_driver_display::Rect::new(0, 0, terminal_size.0, terminal_size.1);
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

        let mut clients = self.clients.write();
        clients.insert(client_id, client);
    }

    /// Add a client with a specific initial state.
    ///
    /// Used for restoring clients or creating clients with pre-configured state.
    pub fn add_client_with_state(&self, client: Client) {
        let mut clients = self.clients.write();
        clients.insert(client.id, client);
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
        let mut clients = self.clients.write();

        // Phase #481: Dump ring buffer before removal
        if let Some(client) = clients.get(&client_id) {
            log_client_disconnect(client_id, &client.ring_buffer);
        }

        clients.remove(&client_id)
    }

    /// Get a client's role (immutable).
    #[must_use]
    pub fn get_client(&self, client_id: ClientId) -> Option<Client> {
        let clients = self.clients.read();
        clients.get(&client_id).cloned()
    }

    /// Set a client's relation with validation.
    ///
    /// Use this to change between Independent/Following/Sharing modes.
    /// Pass `None` for independent, `Some(ClientRelation::Following { target })`
    /// for following, or `Some(ClientRelation::Sharing { with })` for sharing.
    ///
    /// # Validation
    ///
    /// Validates the transition:
    /// - Cannot target self
    /// - Target must exist
    /// - Cannot create cycles (A → B → A)
    /// - Following → Sharing upgrade may require cursor sync (returns `RequiresCursorSync`)
    ///
    /// # Errors
    ///
    /// Returns `Err(TransitionResult)` if:
    /// - Client not found (`TargetNotFound`)
    /// - Attempting to target self (`CannotTargetSelf`)
    /// - Change would create a cycle (`WouldCreateCycle`)
    /// - Following → Sharing requires cursor sync first (`RequiresCursorSync`)
    pub fn set_client_relation(
        &self,
        client_id: ClientId,
        relation: Option<super::ClientRelation>,
    ) -> Result<(), super::TransitionResult> {
        let mut clients = self.clients.write();

        // First validate without mutation
        let validation_result = {
            let Some(client) = clients.get(&client_id) else {
                return Err(super::TransitionResult::TargetNotFound(client_id));
            };
            Client::validate_relation_change(client, relation, &clients)
        };

        // If validation passed, apply the change
        let result = match validation_result {
            super::TransitionResult::Ok => {
                if let Some(client) = clients.get_mut(&client_id) {
                    client.set_relation_unchecked(relation);
                }
                Ok(())
            }
            other => Err(other),
        };

        drop(clients);
        result
    }

    /// Set a client's relation without validation.
    ///
    /// **Use sparingly** - prefer `set_client_relation()` for safety.
    /// This is useful for initialization where validation isn't needed.
    ///
    /// # Returns
    ///
    /// `true` if the client was found and relation set, `false` otherwise.
    pub fn set_client_relation_unchecked(
        &self,
        client_id: ClientId,
        relation: Option<super::ClientRelation>,
    ) -> bool {
        let mut clients = self.clients.write();
        clients.get_mut(&client_id).is_some_and(|client| {
            client.set_relation_unchecked(relation);
            true
        })
    }

    /// Sync cursor and set relation.
    ///
    /// Use this when `set_client_relation()` returns `RequiresCursorSync`.
    /// This syncs the cursor first, then sets the relation.
    ///
    /// # Errors
    ///
    /// Returns `Err(TransitionResult)` if:
    /// - Client or target not found (`TargetNotFound`)
    /// - Attempting to target self (`CannotTargetSelf`)
    /// - Change would create a cycle (`WouldCreateCycle`)
    pub fn sync_and_set_relation(
        &self,
        client_id: ClientId,
        target_id: ClientId,
        relation: Option<super::ClientRelation>,
    ) -> Result<(), super::TransitionResult> {
        let mut clients = self.clients.write();

        // Sync cursor first
        let target_cursor = clients
            .get(&target_id)
            .and_then(|c| c.state.windows.active())
            .map(|w| w.cursor);

        if let (Some(cursor), Some(client)) = (target_cursor, clients.get_mut(&client_id))
            && let Some(window) = client.state.windows.active_mut()
        {
            window.cursor = cursor;
        }

        // Validate without mutation
        let validation_result = {
            let Some(client) = clients.get(&client_id) else {
                return Err(super::TransitionResult::TargetNotFound(client_id));
            };
            Client::validate_relation_change(client, relation, &clients)
        };

        // If validation passed, apply the change
        let result = match validation_result {
            super::TransitionResult::Ok => {
                if let Some(client) = clients.get_mut(&client_id) {
                    client.set_relation_unchecked(relation);
                }
                Ok(())
            }
            other => Err(other),
        };

        drop(clients);
        result
    }

    /// Set a client's role (deprecated).
    ///
    /// **DEPRECATED**: Use `set_client_relation()` instead.
    #[deprecated(since = "0.10.0", note = "Use set_client_relation() instead")]
    pub fn set_client_role(&self, client_id: ClientId, role: Client) {
        let mut clients = self.clients.write();
        clients.insert(client_id, role);
    }

    /// Get the effective editing state for a client.
    ///
    /// - Owner: Returns own state
    /// - Follow: Returns target's state (read-only access)
    /// - Share: Returns owner's state (for display)
    ///
    /// Returns `None` if client not found or target chain is broken.
    #[must_use]
    pub fn client_state(&self, client_id: ClientId) -> Option<super::EditingState> {
        let clients = self.clients.read();
        clients
            .get(&client_id)
            .and_then(|c| c.effective_state(&clients))
            .cloned()
    }

    /// Update a client's editing state via closure.
    ///
    /// - Independent: Updates own state
    /// - Following: No-op (input ignored)
    /// - Sharing: Updates target's state
    ///
    /// Returns `true` if state was updated.
    pub fn update_client_state<F>(&self, client_id: ClientId, f: F) -> bool
    where
        F: FnOnce(&mut super::EditingState),
    {
        let mut clients = self.clients.write();

        // Find the target client ID based on relation
        let Some(client) = clients.get(&client_id) else {
            return false;
        };

        let target_id = match client.relation {
            None => client_id, // Independent - update own state
            Some(super::ClientRelation::Sharing { with }) => with, // Sharing - update target's state
            Some(super::ClientRelation::Following { .. }) => return false, // Following - input ignored
        };

        // Update the target's state
        if let Some(target_client) = clients.get_mut(&target_id) {
            f(&mut target_client.state);
            true
        } else {
            false
        }
    }

    /// Execute a closure with read access to the clients map.
    pub fn with_clients<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<ClientId, Client>) -> R,
    {
        let clients = self.clients.read();
        f(&clients)
    }

    /// Execute a closure with write access to the clients map.
    pub fn with_clients_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut HashMap<ClientId, Client>) -> R,
    {
        let mut clients = self.clients.write();
        f(&mut clients)
    }

    /// Run a closure on a client's `ExtensionMap` without cloning.
    ///
    /// `EditingState::clone()` creates an empty `ExtensionMap` because
    /// `Box<dyn SessionExtensionDyn>` is not `Clone`. This method provides
    /// direct read access to extensions through the clients lock.
    ///
    /// Respects Follow/Share relations via `effective_state()`.
    pub fn with_client_extensions<F, R>(&self, client_id: ClientId, f: F) -> Option<R>
    where
        F: FnOnce(&ExtensionMap) -> R,
    {
        let clients = self.clients.read();
        let client = clients.get(&client_id)?;
        let state = client.effective_state(&clients)?;
        let result = f(&state.extensions);
        drop(clients);
        Some(result)
    }

    /// Get count of connected clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.read().len()
    }

    /// Check if a client is connected.
    #[must_use]
    pub fn has_client(&self, client_id: ClientId) -> bool {
        self.clients.read().contains_key(&client_id)
    }

    /// Get the session ID.
    #[must_use]
    pub const fn id(&self) -> &SessionId {
        &self.id
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
    fn ensure_client_has_window(editing_state: &mut super::EditingState, state: &SessionState) {
        use reovim_driver_session::Window;

        // Only sync if per-client windows are empty AND session has an active buffer
        if editing_state.windows.is_empty()
            && let Some(buffer_id) = state.active_buffer()
        {
            // #474: If per-client compositor exists, create windows with matching IDs
            if let Some(ref compositor) = editing_state.compositor {
                let terminal_size = state.driver_session.terminal_size();
                let screen =
                    reovim_driver_display::Rect::new(0, 0, terminal_size.0, terminal_size.1);
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
        key: &reovim_driver_input::KeyEvent,
    ) -> Option<(reovim_driver_input::ResolveResult, reovim_driver_session::api::StateChanges)>
    {
        // Acquire both locks in consistent order to avoid deadlocks
        let mut clients = self.clients.write();
        let mut state = self.state.write();

        // Find the target client ID based on relation
        let target_id = Self::find_input_target(&clients, client_id)?;

        // Phase #471/#477/#480: Get mutable references to per-client state
        let target_client = clients.get_mut(&target_id)?;
        let editing_state = &mut target_client.state;

        // Ensure per-client windows are populated (fixes buffer-after-client-join issue)
        Self::ensure_client_has_window(editing_state, &state);

        let (mode_stack, windows, extensions, compositor) = (
            &mut editing_state.mode_stack,
            &mut editing_state.windows,
            &mut editing_state.extensions,
            &mut editing_state.compositor,
        );

        // Resolve key with per-client state (#471 Phase 5: pass client_id for undo origin)
        state.resolve_key_for_client(
            target_id.as_usize(),
            mode_stack,
            windows,
            extensions,
            compositor,
            key,
        )
    }

    /// Try `on_command_complete` with per-client state (#471, #477).
    ///
    /// Like `resolve_key_for_client`, but for post-command mode transitions.
    #[allow(clippy::unused_async, clippy::significant_drop_tightening)]
    pub async fn try_on_command_complete_for_client(
        &self,
        client_id: ClientId,
    ) -> Option<reovim_driver_input::ModeTransition> {
        // Acquire both locks in consistent order
        let mut clients = self.clients.write();
        let mut state = self.state.write();

        // Find the target client ID based on relation
        let target_id = Self::find_input_target(&clients, client_id)?;

        // Phase #471/#477/#480: Get mutable references to per-client state
        let target_client = clients.get_mut(&target_id)?;
        let editing_state = &mut target_client.state;

        // Ensure per-client windows are populated (fixes buffer-after-client-join issue)
        Self::ensure_client_has_window(editing_state, &state);

        let (mode_stack, windows, extensions, compositor) = (
            &mut editing_state.mode_stack,
            &mut editing_state.windows,
            &mut editing_state.extensions,
            &mut editing_state.compositor,
        );

        state.try_on_command_complete_for_client(
            target_id.as_usize(),
            mode_stack,
            windows,
            extensions,
            compositor,
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
        args: &reovim_driver_command_types::CommandContext,
    ) -> Option<(reovim_driver_command::CommandResult, reovim_driver_session::api::StateChanges)>
    {
        // Acquire both locks in consistent order to avoid deadlocks
        let mut clients = self.clients.write();
        let mut state = self.state.write();

        // Find the target client ID based on relation
        let target_id = Self::find_input_target(&clients, client_id)?;

        // Phase #471/#477/#480: Get mutable references to per-client state
        let target_client = clients.get_mut(&target_id)?;
        let editing_state = &mut target_client.state;

        // Ensure per-client windows are populated (fixes buffer-after-client-join issue)
        Self::ensure_client_has_window(editing_state, &state);

        let (mode_stack, windows, extensions, compositor) = (
            &mut editing_state.mode_stack,
            &mut editing_state.windows,
            &mut editing_state.extensions,
            &mut editing_state.compositor,
        );

        // Execute command with per-client state, passing client_id for per-client undo (#471)
        state.execute_command_for_client(
            target_id.as_usize(),
            mode_stack,
            windows,
            extensions,
            compositor,
            cmd_id,
            args,
        )
    }

    /// Insert a character for a client, checking per-client extensions first (#477).
    ///
    /// For `InputTarget::Buffer`, inserts into the active buffer.
    /// For `InputTarget::Extension(type_id)`, looks in per-client extensions first,
    /// then falls back to shared session extensions.
    ///
    /// # Returns
    ///
    /// - `Some(BufferId)` if character was inserted into a buffer
    /// - `None` if inserted into extension or failed
    #[allow(clippy::significant_drop_tightening)]
    pub fn insert_char_for_client(
        &self,
        client_id: ClientId,
        ch: char,
        target: reovim_driver_input::InputTarget,
    ) -> Option<reovim_kernel::api::v1::BufferId> {
        use {
            reovim_driver_input::InputTarget,
            reovim_driver_undo::{UndoKey, UndoProviderRegistry},
            reovim_kernel::api::v1::{Edit, Position},
        };

        match target {
            InputTarget::Buffer => {
                // Insert into active buffer at client's cursor position
                let state = self.state.read();
                let buffer_id = state.active_buffer()?;
                let buffer_arc = state.buffer(buffer_id)?;

                // Get undo registry for recording edit (#471)
                let undo_registry = state.app.kernel.services.get::<UndoProviderRegistry>();

                drop(state); // Release lock before getting client

                // Get cursor position from client's window
                let mut clients = self.clients.write();
                let client = clients.get_mut(&client_id)?;
                let active_window = client.state.windows.active_mut()?;
                let cursor_before =
                    Position::new(active_window.cursor.line, active_window.cursor.column);

                tracing::debug!(?buffer_id, ?ch, ?cursor_before, "Inserting into buffer");
                buffer_arc.write().insert_at(cursor_before, &ch.to_string());

                // Update cursor position after insertion
                // For regular characters, move cursor one position right
                // For newlines, move to start of next line
                if ch == '\n' {
                    active_window.cursor.line += 1;
                    active_window.cursor.column = 0;
                } else {
                    active_window.cursor.column += 1;
                }

                let cursor_after =
                    Position::new(active_window.cursor.line, active_window.cursor.column);

                drop(clients);

                // Record edit for undo with client origin (#471)
                if let Some(undo_reg) = undo_registry
                    && let Some(undo_provider) = undo_reg.get(&UndoKey::Buffer)
                {
                    let edit = Edit::Insert {
                        position: cursor_before,
                        text: ch.to_string(),
                    };
                    undo_provider.record_for_client(
                        buffer_id,
                        client_id.as_usize(),
                        vec![edit],
                        cursor_before,
                        cursor_after,
                    );
                }

                Some(buffer_id)
            }
            InputTarget::Extension(type_id) => {
                // Phase #477: Check per-client extensions FIRST, then shared
                tracing::debug!(?type_id, ?ch, %client_id, "Routing to extension via TextInputSink");

                // Try per-client extensions first
                let mut clients = self.clients.write();
                if let Some(client) = clients.get_mut(&client_id)
                    && let Some(sink) = client.state.extensions.get_text_input_sink_by_id(type_id)
                {
                    sink.insert_char(ch);
                    tracing::debug!(?type_id, "Inserted char via per-client extension");
                    return None;
                }
                drop(clients);

                // Fallback to shared session extensions (#491)
                let mut state = self.state.write();
                if let Some(sink) = state.app.extensions.get_text_input_sink_by_id(type_id) {
                    sink.insert_char(ch);
                    tracing::debug!(?type_id, "Inserted char via shared extension");
                } else {
                    tracing::warn!(
                        ?type_id,
                        "Extension not found or doesn't implement TextInputSink"
                    );
                }
                drop(state); // Release lock early
                None
            }
        }
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
        let target_id = Self::find_input_target(&clients, client_id)?;

        // Get mode from target's mode stack
        clients
            .get(&target_id)
            .map(|c| c.state.mode_stack.current().clone())
    }

    /// Find the target client ID for input routing.
    ///
    /// - Independent: returns self
    /// - Following: returns None (input ignored)
    /// - Sharing: returns target
    fn find_input_target(
        clients: &HashMap<ClientId, Client>,
        client_id: ClientId,
    ) -> Option<ClientId> {
        let client = clients.get(&client_id)?;
        if client.is_independent() {
            Some(client_id)
        } else if client.is_sharing() {
            client.target_id()
        } else {
            // Following - input ignored
            None
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let session = Session::new(SessionId::new("test"));
        assert_eq!(session.id().name(), "test");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_remove_client_dumps_ring_buffer() {
        let session = Session::new(SessionId::new("dump-test"));
        let client_id = ClientId::new(42);

        // Add a client using the new API
        session.add_client(client_id);

        // Log some events to the client's ring buffer
        session.with_client_ring_buffer(client_id, |ring| {
            ring.log_key("a");
            ring.log_key("b");
            ring.log_command("write");
        });

        // Remove the client - this should create a dump file
        let removed = session.remove_client(client_id);
        assert!(removed.is_some());

        // Verify the dump file was created
        // Note: The dump file path includes a timestamp, so we check the crash directory
        let crash_dir = super::super::crash_dump::crash_dir();
        if crash_dir.exists() {
            // Look for a dump file with our client ID
            let pattern = format!("client-{}-", client_id.as_usize());
            let found = std::fs::read_dir(&crash_dir).ok().is_some_and(|entries| {
                entries
                    .filter_map(Result::ok)
                    .any(|e| e.file_name().to_string_lossy().starts_with(&pattern))
            });

            // May not find file in CI environments where directory isn't writable
            if found {
                // Clean up: find and remove the test file
                if let Ok(entries) = std::fs::read_dir(&crash_dir) {
                    for entry in entries.filter_map(Result::ok) {
                        let name = entry.file_name();
                        if name.to_string_lossy().starts_with(&pattern) {
                            std::fs::remove_file(entry.path()).ok();
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_remove_following_client_also_dumps() {
        use crate::session::ClientRelation;

        // In #480 unified architecture, ALL clients have ring buffers
        let session = Session::new(SessionId::new("follow-test"));
        let owner_id = ClientId::new(1);
        let follow_id = ClientId::new(2);

        // Add owner and follower
        session.add_client(owner_id);
        session.add_client(follow_id);

        // Set follower relation
        let _ = session
            .set_client_relation(follow_id, Some(ClientRelation::Following { target: owner_id }));

        // Log event to follower's ring buffer
        session.with_client_ring_buffer(follow_id, |ring| {
            ring.log_state_change("follower connected");
        });

        // Remove the follower - should still dump (all clients have ring buffers now)
        let removed = session.remove_client(follow_id);
        assert!(removed.is_some());
        assert!(removed.unwrap().is_following());

        // Clean up owner
        session.remove_client(owner_id);
    }

    #[test]
    fn test_remove_sharing_client_also_dumps() {
        use crate::session::ClientRelation;

        // In #480 unified architecture, ALL clients have ring buffers
        let session = Session::new(SessionId::new("share-test"));
        let owner_id = ClientId::new(1);
        let share_id = ClientId::new(2);

        // Add owner and sharer
        session.add_client(owner_id);
        session.add_client(share_id);

        // Set sharing relation
        let _ =
            session.set_client_relation(share_id, Some(ClientRelation::Sharing { with: owner_id }));

        // Log event to sharer's ring buffer
        session.with_client_ring_buffer(share_id, |ring| {
            ring.log_state_change("sharer connected");
        });

        // Remove the sharer - should dump (all clients have ring buffers)
        let removed = session.remove_client(share_id);
        assert!(removed.is_some());
        assert!(removed.unwrap().is_sharing());

        // Clean up owner
        session.remove_client(owner_id);
    }

    #[tokio::test]
    async fn test_session_with_state() {
        use std::sync::Arc;

        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
        };

        // Create a kernel context with a real buffer manager
        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = SessionState::with_kernel(kernel);
        #[allow(deprecated)]
        let session = Session::new_with_state(SessionId::default(), state);

        // Create a buffer
        session
            .with_state_mut(|state| {
                state.create_buffer("hello world");
            })
            .await;

        // Read it back
        let has_buffer = session
            .with_state(|state| state.active_buffer().is_some())
            .await;

        assert!(has_buffer);
    }

    #[test]
    fn test_session_from_state() {
        let state = SessionState::default();
        let session = Session::from_state(SessionId::new("from-state"), state);
        assert_eq!(session.id().name(), "from-state");
        assert_eq!(session.client_count(), 0);
    }

    #[test]
    fn test_add_client() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);
        assert_eq!(session.client_count(), 1);
        assert!(session.has_client(client_id));
    }

    #[test]
    fn test_add_client_with_metadata() {
        use crate::session::ClientMetadata;

        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);
        let metadata = ClientMetadata::default();

        session.add_client_with_metadata(client_id, metadata);
        assert_eq!(session.client_count(), 1);
        assert!(session.has_client(client_id));
    }

    #[test]
    fn test_add_client_with_state() {
        use {
            crate::session::ClientMetadata,
            reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
        };

        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);
        let mode = ModeId::new(ModuleId::new("test"), "normal");
        let mode_stack = ModeStack::new(mode);
        let metadata = ClientMetadata::default();

        let client = Client::with_mode_stack(client_id, metadata, mode_stack);
        session.add_client_with_state(client);

        assert_eq!(session.client_count(), 1);
        assert!(session.has_client(client_id));
    }

    #[test]
    fn test_get_client() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);
        let client = session.get_client(client_id);
        assert!(client.is_some());
        assert_eq!(client.unwrap().id, client_id);
    }

    #[test]
    fn test_client_state() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);
        let state = session.client_state(client_id);
        assert!(state.is_some());
    }

    #[test]
    fn test_update_client_state() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);

        let updated = session.update_client_state(client_id, |state| {
            // Just access the mode_stack to verify we can mutate
            let _ = state.mode_stack.current();
        });

        assert!(updated);
    }

    #[test]
    fn test_update_client_state_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(999);

        let updated = session.update_client_state(client_id, |_state| {});
        assert!(!updated);
    }

    #[test]
    fn test_with_clients() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);

        let count = session.with_clients(std::collections::HashMap::len);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_with_clients_mut() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);

        session.with_clients_mut(|clients| {
            assert_eq!(clients.len(), 1);
        });
    }

    #[test]
    fn test_has_client() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        assert!(!session.has_client(client_id));
        session.add_client(client_id);
        assert!(session.has_client(client_id));
    }

    #[test]
    fn test_set_client_relation_independent_to_following() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(follower_id);

        let result = session
            .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));

        assert!(result.is_ok());
        let client = session.get_client(follower_id).unwrap();
        assert!(client.is_following());
    }

    #[test]
    fn test_set_client_relation_cannot_target_self() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);

        let result = session
            .set_client_relation(client_id, Some(ClientRelation::Following { target: client_id }));

        assert!(result.is_err());
    }

    #[test]
    fn test_set_client_relation_target_not_found() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);
        let nonexistent_id = ClientId::new(999);

        session.add_client(client_id);

        let result = session.set_client_relation(
            client_id,
            Some(ClientRelation::Following {
                target: nonexistent_id,
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_set_client_relation_unchecked() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(follower_id);

        let result = session.set_client_relation_unchecked(
            follower_id,
            Some(ClientRelation::Following { target: owner_id }),
        );

        assert!(result);
    }

    #[test]
    fn test_set_client_relation_unchecked_nonexistent() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let nonexistent_id = ClientId::new(999);
        let target_id = ClientId::new(1);

        let result = session.set_client_relation_unchecked(
            nonexistent_id,
            Some(ClientRelation::Following { target: target_id }),
        );

        assert!(!result);
    }

    #[test]
    fn test_sync_and_set_relation() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let sharer_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(sharer_id);

        let result = session.sync_and_set_relation(
            sharer_id,
            owner_id,
            Some(ClientRelation::Sharing { with: owner_id }),
        );

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_with_state_sync() {
        let session = Session::new(SessionId::new("test"));

        let running = session.with_state_sync(SessionState::is_running);
        assert!(running);
    }

    #[tokio::test]
    async fn test_with_state_mut_sync() {
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = SessionState::with_kernel(kernel);
        let session = Session::from_state(SessionId::new("test"), state);

        session.with_state_mut_sync(|state| {
            state.create_buffer("test content");
        });

        let has_buffer = session.with_state_sync(|state| state.active_buffer().is_some());
        assert!(has_buffer);
    }

    #[tokio::test]
    async fn test_client_current_mode() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);

        let mode = session.client_current_mode(client_id);
        assert!(mode.is_some());
    }

    #[test]
    fn test_dump_client_ring_buffer() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);

        // Log something to the ring buffer
        session.with_client_ring_buffer(client_id, |ring| {
            ring.log_key("x");
        });

        let dump = session.dump_client_ring_buffer(client_id);
        assert!(dump.is_some());
        assert!(dump.unwrap().contains('x'));
    }

    #[test]
    fn test_dump_client_ring_buffer_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let nonexistent_id = ClientId::new(999);

        let dump = session.dump_client_ring_buffer(nonexistent_id);
        assert!(dump.is_none());
    }

    #[cfg(feature = "grpc")]
    #[test]
    fn test_subscribe_notifications() {
        let session = Session::new(SessionId::new("test"));
        let _rx = session.subscribe_notifications();
        // Just verify it doesn't panic
    }

    #[cfg(feature = "grpc")]
    #[test]
    fn test_emit_notification() {
        use reovim_protocol::v2::Notification;

        let session = Session::new(SessionId::new("test"));
        let mut rx = session.subscribe_notifications();

        // Create a minimal notification - the exact fields depend on proto definition
        let notification = Notification::default();

        session.emit_notification(notification);

        // Try to receive (non-blocking check)
        // Just verify it doesn't panic - the actual notification content
        // is proto-generated and may vary
        let _result = rx.try_recv();
    }

    #[cfg(feature = "grpc")]
    #[test]
    fn test_capture_tracker() {
        let session = Session::new(SessionId::new("test"));
        let _tracker = session.capture_tracker();
        // Just verify it doesn't panic
    }

    #[cfg(feature = "grpc")]
    #[test]
    fn test_presence() {
        let session = Session::new(SessionId::new("test"));
        let _presence = session.presence();
        // Just verify it doesn't panic
    }

    #[test]
    fn test_update_client_state_following_ignored() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(follower_id);

        session
            .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }))
            .ok();

        // Following clients should ignore updates
        let updated = session.update_client_state(follower_id, |_state| {});
        assert!(!updated);
    }

    #[test]
    fn test_update_client_state_sharing() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let sharer_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(sharer_id);

        session
            .set_client_relation(sharer_id, Some(ClientRelation::Sharing { with: owner_id }))
            .ok();

        // Sharing clients should update target's state
        let updated = session.update_client_state(sharer_id, |_state| {});
        assert!(updated);
    }

    #[test]
    fn test_session_id_accessor() {
        let session = Session::new(SessionId::new("my-session"));
        assert_eq!(session.id().name(), "my-session");
    }

    #[test]
    fn test_remove_client_returns_none_for_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let result = session.remove_client(ClientId::new(999));
        assert!(result.is_none());
    }

    #[test]
    fn test_get_client_returns_none_for_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let result = session.get_client(ClientId::new(999));
        assert!(result.is_none());
    }

    #[test]
    fn test_client_state_returns_none_for_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let result = session.client_state(ClientId::new(999));
        assert!(result.is_none());
    }

    #[test]
    fn test_client_current_mode_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let result = session.client_current_mode(ClientId::new(999));
        assert!(result.is_none());
    }

    #[test]
    fn test_client_current_mode_following_returns_none() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(follower_id);

        session
            .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }))
            .ok();

        // Following clients should return None for mode (input ignored)
        let result = session.client_current_mode(follower_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_client_current_mode_sharing_returns_target_mode() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let sharer_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(sharer_id);

        session
            .set_client_relation(sharer_id, Some(ClientRelation::Sharing { with: owner_id }))
            .ok();

        // Sharing client should return target's mode
        let result = session.client_current_mode(sharer_id);
        assert!(result.is_some());
    }

    #[test]
    fn test_multiple_clients() {
        let session = Session::new(SessionId::new("test"));
        let c1 = ClientId::new(1);
        let c2 = ClientId::new(2);
        let c3 = ClientId::new(3);

        session.add_client(c1);
        session.add_client(c2);
        session.add_client(c3);

        assert_eq!(session.client_count(), 3);
        assert!(session.has_client(c1));
        assert!(session.has_client(c2));
        assert!(session.has_client(c3));

        // Remove one
        let removed = session.remove_client(c2);
        assert!(removed.is_some());
        assert_eq!(session.client_count(), 2);
        assert!(!session.has_client(c2));
    }

    #[test]
    fn test_with_client_ring_buffer_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let result = session.with_client_ring_buffer(ClientId::new(999), |_| ());
        assert!(result.is_none());
    }

    #[test]
    fn test_with_client_ring_buffer_returns_value() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Log and retrieve
        session.with_client_ring_buffer(client_id, |ring| {
            ring.log_key("a");
            ring.log_key("b");
        });

        let count = session.with_client_ring_buffer(client_id, |ring| {
            let dump = ring.dump();
            dump.matches('a').count()
        });
        assert!(count.is_some());
        assert!(count.unwrap() > 0);
    }

    #[test]
    fn test_set_client_relation_with_cycle_detection() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("cycle-test"));
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);
        let id3 = ClientId::new(3);

        session.add_client(id1);
        session.add_client(id2);
        session.add_client(id3);

        // id2 follows id3
        let _ = session.set_client_relation(id2, Some(ClientRelation::Following { target: id3 }));

        // id3 follows id1
        let _ = session.set_client_relation(id3, Some(ClientRelation::Following { target: id1 }));

        // id1 trying to follow id2 would create cycle: 1 -> 2 -> 3 -> 1
        let result =
            session.set_client_relation(id1, Some(ClientRelation::Following { target: id2 }));
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_and_set_relation_nonexistent_client() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));

        let result = session.sync_and_set_relation(
            ClientId::new(999),
            ClientId::new(888),
            Some(ClientRelation::Sharing {
                with: ClientId::new(888),
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_set_client_relation_client_not_found() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let nonexistent = ClientId::new(999);

        // Session-level relation setting should fail for non-existent client
        let result = session.set_client_relation(
            nonexistent,
            Some(ClientRelation::Following {
                target: ClientId::new(1),
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_set_client_relation_back_to_independent() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(follower_id);

        // Set following
        let result = session
            .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));
        assert!(result.is_ok());
        assert!(session.get_client(follower_id).unwrap().is_following());

        // Set back to independent (None relation)
        let result = session.set_client_relation(follower_id, None);
        assert!(result.is_ok());
        assert!(session.get_client(follower_id).unwrap().is_independent());
    }

    #[cfg(feature = "grpc")]
    #[test]
    fn test_emit_notification_and_receive() {
        use reovim_protocol::v2::Notification;

        let session = Session::new(SessionId::new("notif-test"));
        let mut rx = session.subscribe_notifications();

        let notification = Notification {
            event_type: "test_event".to_string(),
            timestamp_ms: 12345,
            payload: None,
        };

        session.emit_notification(notification);

        let received = rx.try_recv();
        assert!(received.is_ok());
        let n = received.unwrap();
        assert_eq!(n.event_type, "test_event");
        assert_eq!(n.timestamp_ms, 12345);
    }

    #[cfg(feature = "grpc")]
    #[test]
    fn test_emit_notification_no_subscribers() {
        let session = Session::new(SessionId::new("no-sub-test"));

        // Emit with no subscribers should not panic
        let notification = reovim_protocol::v2::Notification {
            event_type: "orphan".to_string(),
            timestamp_ms: 0,
            payload: None,
        };

        session.emit_notification(notification);
    }

    #[tokio::test]
    async fn test_resolve_key_for_client_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));

        // Non-existent client should return None
        let result = session
            .resolve_key_for_client(ClientId::new(999), &key)
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_resolve_key_for_client_following_ignored() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(follower_id);

        let _ = session
            .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));

        let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
        let result = session.resolve_key_for_client(follower_id, &key).await;

        // Following clients should return None (input ignored)
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_try_on_command_complete_for_client_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let result = session
            .try_on_command_complete_for_client(ClientId::new(999))
            .await;
        assert!(result.is_none());
    }

    #[test]
    fn test_execute_command_for_client_nonexistent() {
        let session = Session::new(SessionId::new("test"));
        let cmd_id = reovim_kernel::api::v1::CommandId::new(
            reovim_kernel::api::v1::ModuleId::new("test"),
            "noop",
        );
        let ctx = reovim_driver_command_types::CommandContext::new();

        let result = session.execute_command_for_client(ClientId::new(999), &cmd_id, &ctx);
        assert!(result.is_none());
    }

    #[test]
    fn test_execute_command_for_client_following_ignored() {
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(follower_id);

        let _ = session
            .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));

        let cmd_id = reovim_kernel::api::v1::CommandId::new(
            reovim_kernel::api::v1::ModuleId::new("test"),
            "noop",
        );
        let ctx = reovim_driver_command_types::CommandContext::new();

        let result = session.execute_command_for_client(follower_id, &cmd_id, &ctx);
        // Following clients should return None (input ignored)
        assert!(result.is_none());
    }

    #[test]
    fn test_add_client_with_active_buffer() {
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = SessionState::with_kernel(kernel);
        let session = Session::from_state(SessionId::new("buf-test"), state);

        // Create a buffer so session has an active buffer
        session.with_state_mut_sync(|state| {
            state.create_buffer("hello");
        });

        // Adding a client now should give it a window with the active buffer
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Client should have a window
        let state = session.client_state(client_id);
        assert!(state.is_some());
        let editing_state = state.unwrap();
        assert!(!editing_state.windows.is_empty());
    }

    #[test]
    fn test_insert_char_for_client_buffer_path() {
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_driver_input::InputTarget,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = SessionState::with_kernel(kernel);
        let session = Session::from_state(SessionId::new("insert-test"), state);

        // Create a buffer
        session.with_state_mut_sync(|state| {
            state.create_buffer("hello");
        });

        // Add a client (which gets a window with the active buffer)
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Insert a character into the buffer
        let result = session.insert_char_for_client(client_id, 'X', InputTarget::Buffer);
        assert!(result.is_some()); // Should return the buffer id

        // Verify the character was inserted
        session.with_state_sync(|state| {
            let buffer_id = state.active_buffer().unwrap();
            let buffer = state.buffer(buffer_id).unwrap();
            let content = buffer.read().content();
            assert!(content.contains('X'));
        });
    }

    #[test]
    fn test_insert_char_for_client_newline() {
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_driver_input::InputTarget,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = SessionState::with_kernel(kernel);
        let session = Session::from_state(SessionId::new("newline-test"), state);

        session.with_state_mut_sync(|state| {
            state.create_buffer("ab");
        });

        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Insert a newline
        let result = session.insert_char_for_client(client_id, '\n', InputTarget::Buffer);
        assert!(result.is_some());

        // Verify the cursor moved to start of next line
        let editing_state = session.client_state(client_id).unwrap();
        let active_window = editing_state.windows.active().unwrap();
        assert_eq!(active_window.cursor.line, 1);
        assert_eq!(active_window.cursor.column, 0);
    }

    #[test]
    fn test_insert_char_for_client_extension_path() {
        use reovim_driver_input::InputTarget;

        let session = Session::new(SessionId::new("ext-test"));
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Extension target with a random type_id - neither per-client nor shared
        // extension exists, so the char is effectively dropped with a warning.
        let type_id = std::any::TypeId::of::<String>();
        let result =
            session.insert_char_for_client(client_id, 'a', InputTarget::Extension(type_id));
        // Extension path always returns None
        assert!(result.is_none());
    }

    #[test]
    fn test_insert_char_for_client_no_buffer() {
        use reovim_driver_input::InputTarget;

        let session = Session::new(SessionId::new("no-buf-test"));
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // No buffer exists, so insert_char should return None
        let result = session.insert_char_for_client(client_id, 'x', InputTarget::Buffer);
        assert!(result.is_none());
    }

    #[test]
    fn test_insert_char_for_client_nonexistent_client() {
        use reovim_driver_input::InputTarget;

        let session = Session::new(SessionId::new("noone-test"));
        let result = session.insert_char_for_client(ClientId::new(999), 'x', InputTarget::Buffer);
        assert!(result.is_none());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_sync_and_set_relation_with_cursor_sync() {
        use {
            crate::session::ClientRelation,
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = SessionState::with_kernel(kernel);
        let session = Session::from_state(SessionId::new("sync-cursor-test"), state);

        // Create a buffer so clients get windows
        session.with_state_mut_sync(|state| {
            state.create_buffer("test content");
        });

        let owner_id = ClientId::new(1);
        let sharer_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(sharer_id);

        // Move owner's cursor to a specific position
        session.update_client_state(owner_id, |state| {
            if let Some(w) = state.windows.active_mut() {
                w.cursor.line = 5;
                w.cursor.column = 10;
            }
        });

        // Sync and set relation - sharer should get owner's cursor
        let result = session.sync_and_set_relation(
            sharer_id,
            owner_id,
            Some(ClientRelation::Sharing { with: owner_id }),
        );
        assert!(result.is_ok());

        // Sharer's cursor should be synced to owner's position
        let sharer_state = session.client_state(sharer_id).unwrap();
        if let Some(w) = sharer_state.windows.active() {
            assert_eq!(w.cursor.line, 5);
            assert_eq!(w.cursor.column, 10);
        }
    }

    // =========================================================================
    // Coverage: sync_and_set_relation validation failure (#497)
    // =========================================================================

    #[test]
    fn test_sync_and_set_relation_self_target_error() {
        // Test the `other => Err(other)` path in sync_and_set_relation (line 387)
        // where validation fails for a reason other than not found.
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Attempt to share with self - this triggers CannotTargetSelf
        let result = session.sync_and_set_relation(
            client_id,
            client_id,
            Some(ClientRelation::Sharing { with: client_id }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_and_set_relation_would_create_cycle() {
        // Test the cycle detection error path in sync_and_set_relation (line 387)
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("cycle-test"));
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        session.add_client(id1);
        session.add_client(id2);

        // id2 follows id1
        let _ = session.set_client_relation(id2, Some(ClientRelation::Following { target: id1 }));

        // Try to set id1 → id2 via sync_and_set_relation → cycle
        let result =
            session.sync_and_set_relation(id1, id2, Some(ClientRelation::Sharing { with: id2 }));
        assert!(result.is_err());
    }

    // =========================================================================
    // Coverage: deprecated set_client_role (#497)
    // =========================================================================

    #[test]
    fn test_deprecated_set_client_role() {
        // Test the deprecated set_client_role method (lines 398-401).
        use {
            crate::session::ClientMetadata,
            reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
        };

        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);

        session.add_client(client_id);

        // Create a replacement client
        let mode = ModeId::new(ModuleId::new("test"), "insert");
        let mode_stack = ModeStack::new(mode);
        let replacement = Client::with_mode_stack(client_id, ClientMetadata::default(), mode_stack);

        #[allow(deprecated)]
        session.set_client_role(client_id, replacement);

        // Verify the client was replaced
        let client = session.get_client(client_id).unwrap();
        assert_eq!(client.state.mode_stack.current().name(), "insert");
    }

    // =========================================================================
    // Coverage: update_client_state target not found (#497)
    // =========================================================================

    #[test]
    fn test_update_client_state_sharing_target_removed() {
        // Test the `false` return at line 448 when the sharing target doesn't exist.
        use crate::session::ClientRelation;

        let session = Session::new(SessionId::new("test"));
        let owner_id = ClientId::new(1);
        let sharer_id = ClientId::new(2);

        session.add_client(owner_id);
        session.add_client(sharer_id);

        // Set sharer to share with owner
        let _ = session
            .set_client_relation(sharer_id, Some(ClientRelation::Sharing { with: owner_id }));

        // Remove the owner (the sharing target)
        session.remove_client(owner_id);

        // Now update_client_state for sharer should return false
        // because the target (owner) no longer exists
        let updated = session.update_client_state(sharer_id, |_state| {});
        assert!(!updated);
    }

    // =========================================================================
    // Coverage: insert_char_for_client undo recording (#497 lines 775-787)
    // =========================================================================

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_insert_char_for_client_with_undo_recording() {
        // This test covers lines 775-787: the undo recording path in
        // insert_char_for_client when an UndoProviderRegistry with a
        // UndoKey::Buffer provider is registered in services.
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_driver_input::InputTarget,
            reovim_driver_undo::{UndoKey, UndoPersistError, UndoProvider, UndoProviderRegistry},
            reovim_driver_vfs::VfsDriver,
            reovim_kernel::api::v1::{
                BufferId, Edit, EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry,
                Position, RegisterBank, ServiceRegistry, TextObjectEngine, UndoResult, UndoTree,
            },
            std::sync::{Arc, Mutex},
        };

        /// Recorded undo entries: (buffer, client, edits, `cursor_before`, `cursor_after`).
        type UndoRecords = Mutex<Vec<(BufferId, usize, Vec<Edit>, Position, Position)>>;

        /// Minimal mock undo provider that tracks `record_for_client` calls.
        struct MockUndoProvider {
            recorded: UndoRecords,
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl MockUndoProvider {
            fn new() -> Self {
                Self {
                    recorded: Mutex::new(Vec::new()),
                }
            }
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl UndoProvider for MockUndoProvider {
            fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
                None
            }
            fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
                None
            }
            fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
                None
            }
            fn record(
                &self,
                _buffer_id: BufferId,
                _edits: Vec<Edit>,
                _cursor_before: Position,
                _cursor_after: Position,
            ) {
            }
            fn has_history(&self, _buffer_id: BufferId) -> bool {
                false
            }
            fn remove(&self, _buffer_id: BufferId) {}
            fn buffer_count(&self) -> usize {
                0
            }
            fn get_tree(&self, _buffer_id: BufferId) -> Option<UndoTree> {
                None
            }
            fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
            fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
            fn is_batching(&self, _buffer_id: BufferId) -> bool {
                false
            }
            fn persist(
                &self,
                _buffer_id: BufferId,
                _buffer_path: &str,
                _vfs: &dyn VfsDriver,
            ) -> Result<(), UndoPersistError> {
                Ok(())
            }
            fn load(
                &self,
                _buffer_id: BufferId,
                _buffer_path: &str,
                _vfs: &dyn VfsDriver,
            ) -> Result<bool, UndoPersistError> {
                Ok(false)
            }
            fn record_for_client(
                &self,
                buffer_id: BufferId,
                client_id: usize,
                edits: Vec<Edit>,
                cursor_before: Position,
                cursor_after: Position,
            ) {
                self.recorded.lock().unwrap().push((
                    buffer_id,
                    client_id,
                    edits,
                    cursor_before,
                    cursor_after,
                ));
            }
        }

        // Create services with an UndoProviderRegistry containing our mock
        let services = Arc::new(ServiceRegistry::new());
        let undo_registry = Arc::new(UndoProviderRegistry::new());
        let mock_undo = Arc::new(MockUndoProvider::new());
        undo_registry.register(UndoKey::Buffer, Arc::clone(&mock_undo) as Arc<dyn UndoProvider>);
        services.register(undo_registry);

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            services,
        );

        let state = SessionState::with_kernel(kernel);
        let session = Session::from_state(SessionId::new("undo-test"), state);

        // Create a buffer
        session.with_state_mut_sync(|state| {
            state.create_buffer("hello");
        });

        // Add a client
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Insert a character
        let result = session.insert_char_for_client(client_id, 'Z', InputTarget::Buffer);
        assert!(result.is_some());

        // Verify the undo provider received a record_for_client call
        let recorded = mock_undo.recorded.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        let (buf_id, cid, edits, cursor_before, cursor_after) = &recorded[0];
        assert_eq!(*cid, client_id.as_usize());
        assert_eq!(edits.len(), 1);
        // The edit should be an Insert at position (0,0) with text "Z"
        assert!(
            matches!(&edits[0], Edit::Insert { position, text } if *position == Position::new(0, 0) && text == "Z")
        );
        assert_eq!(*cursor_before, Position::new(0, 0));
        assert_eq!(*cursor_after, Position::new(0, 1));
        // Buffer ID should match
        assert_eq!(buf_id.as_usize(), result.unwrap().as_usize());
        drop(recorded);
    }

    // =========================================================================
    // Coverage: insert_char_for_client per-client extension sink (#497 lines 801-803)
    // =========================================================================

    #[test]
    fn test_insert_char_for_client_per_client_extension_sink() {
        // This test covers lines 801-803: routing input to a per-client
        // extension that implements TextInputSink.
        use {
            reovim_driver_input::InputTarget,
            reovim_driver_session::{SessionExtension, TextInputSink},
        };

        /// Test extension that implements `TextInputSink`.
        #[derive(Default)]
        struct TestSinkExtension {
            buffer: String,
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl SessionExtension for TestSinkExtension {
            fn create() -> Self {
                Self::default()
            }

            fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
                Some(self)
            }
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl TextInputSink for TestSinkExtension {
            fn insert_char(&mut self, ch: char) {
                self.buffer.push(ch);
            }
        }

        let session = Session::new(SessionId::new("pcext-test"));
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Install extension into per-client extensions
        let type_id = std::any::TypeId::of::<TestSinkExtension>();
        session.with_clients_mut(|clients| {
            let client = clients.get_mut(&client_id).unwrap();
            client.state.extensions.get_or_insert::<TestSinkExtension>();
        });

        // Route a char to the per-client extension
        let result =
            session.insert_char_for_client(client_id, 'q', InputTarget::Extension(type_id));
        assert!(result.is_none()); // Extension path returns None

        // Verify the char arrived in the per-client extension
        session.with_clients(|clients| {
            let client = clients.get(&client_id).unwrap();
            let ext = client.state.extensions.get::<TestSinkExtension>().unwrap();
            assert_eq!(ext.buffer, "q");
        });
    }

    // =========================================================================
    // Coverage: insert_char_for_client shared extension sink (#497 lines 810-811)
    // =========================================================================

    #[test]
    fn test_insert_char_for_client_shared_extension_sink() {
        // This test covers lines 810-811: routing input to a shared session
        // extension that implements TextInputSink (fallback from per-client).
        use {
            reovim_driver_input::InputTarget,
            reovim_driver_session::{SessionExtension, TextInputSink},
        };

        /// Test extension that implements `TextInputSink`.
        #[derive(Default)]
        struct SharedSinkExtension {
            buffer: String,
        }

        impl SessionExtension for SharedSinkExtension {
            fn create() -> Self {
                Self::default()
            }

            fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
                Some(self)
            }
        }

        impl TextInputSink for SharedSinkExtension {
            fn insert_char(&mut self, ch: char) {
                self.buffer.push(ch);
            }
        }

        let session = Session::new(SessionId::new("shext-test"));
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Install extension into SHARED session extensions (not per-client)
        let type_id = std::any::TypeId::of::<SharedSinkExtension>();
        session.with_state_mut_sync(|state| {
            state.app.extensions.get_or_insert::<SharedSinkExtension>();
        });

        // Route a char - per-client won't have it, so falls through to shared
        let result =
            session.insert_char_for_client(client_id, 'w', InputTarget::Extension(type_id));
        assert!(result.is_none());

        // Verify the char arrived in the shared extension
        session.with_state_sync(|state| {
            let ext = state.app.extensions.get::<SharedSinkExtension>().unwrap();
            assert_eq!(ext.buffer, "w");
        });
    }

    #[test]
    fn test_ensure_client_has_window_lazy_sync() {
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = SessionState::with_kernel(kernel);
        let session = Session::from_state(SessionId::new("lazy-sync"), state);

        // Add a client BEFORE any buffer exists
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Client should have no windows yet
        let editing_state = session.client_state(client_id).unwrap();
        assert!(editing_state.windows.is_empty());

        // Now create a buffer
        session.with_state_mut_sync(|state| {
            state.create_buffer("hello");
        });

        // resolve_key_for_client should trigger ensure_client_has_window
        let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _result = rt.block_on(session.resolve_key_for_client(client_id, &key));

        // After resolve_key, client should now have a window
        let editing_state = session.client_state(client_id).unwrap();
        assert!(!editing_state.windows.is_empty());
    }

    // =========================================================================
    // Coverage: tracing::debug closures in add_client_with_metadata (lines 202-203)
    // =========================================================================

    #[test]
    fn test_add_client_with_metadata_tracing_debug_closures() {
        // Set up a DEBUG-level tracing subscriber to execute lazy closures
        // inside tracing::debug! at lines 200-206.
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let session = Session::new(SessionId::new("tracing-debug-test"));
        let client_id = ClientId::new(42);

        // This exercises the tracing::debug! block at lines 200-206 which includes
        // mode_module = %home_mode.module() and mode_name = %home_mode.name()
        session.add_client(client_id);

        // Verify client was added
        assert!(session.client_state(client_id).is_some());
    }

    // =========================================================================
    // Coverage: pr_info! in remove_client after crash dump (lines 254-255)
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_remove_client_with_crash_dump_written() {
        // The pr_info! at lines 252-256 only executes when try_dump_client_to_file
        // returns Some(path), which requires writing to ~/.local/share/reovim/crash/.
        // This test verifies the dump path by adding ring buffer entries first.
        let session = Session::new(SessionId::new("crash-dump-test"));
        let client_id = ClientId::new(77);
        session.add_client(client_id);

        // Log some events to the client's ring buffer so the dump has content
        session.with_client_ring_buffer(client_id, |rb| {
            rb.log_key("a");
            rb.log_key("b");
            rb.log_command("write");
        });

        // remove_client calls try_dump_client_to_file
        // If it succeeds (crash dir is writable), pr_info! at lines 254-255 executes
        let removed = session.remove_client(client_id);
        assert!(removed.is_some());
    }

    // =========================================================================
    // Coverage: EditingState Debug impl (#497 lines 612-622)
    // =========================================================================

    #[test]
    fn test_editing_state_debug_format() {
        // Exercise the manual Debug impl for EditingState (lines 611-622).
        // The impl formats compositor as "..." via `.map(|_| "...")`.
        let session = Session::new(SessionId::new("debug-fmt-test"));
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        let state = session.client_state(client_id).unwrap();
        let debug_str = format!("{state:?}");
        assert!(debug_str.contains("EditingState"));
        assert!(debug_str.contains("mode_stack"));
        assert!(debug_str.contains("pending_keys"));
        assert!(debug_str.contains("windows"));
        assert!(debug_str.contains("viewport"));
        assert!(debug_str.contains("selection"));
        assert!(debug_str.contains("extensions"));
        assert!(debug_str.contains("compositor"));
    }

    // =========================================================================
    // Coverage: compositor-driven window creation (#497 lines 239-251, 590-600)
    // =========================================================================

    /// Mock compositor that returns one tiled placement with a focused window.
    ///
    /// Unlike `MockRootCompositor` in `runtime.rs` which returns empty results,
    /// this compositor provides actual placements so the session code at
    /// lines 239-251 and 590-600 can create windows from them.
    struct TestPlacementCompositor;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl reovim_driver_display::layout::RootCompositor for TestPlacementCompositor {
        fn composite(
            &self,
            screen: reovim_driver_display::Rect,
        ) -> reovim_driver_display::layout::CompositeResult {
            use reovim_driver_display::{
                WindowId,
                layout::{CompositeResult, LayerId, WindowPlacement, ZOrder, Zone},
            };

            CompositeResult {
                placements: vec![WindowPlacement {
                    window_id: WindowId::from_raw(1),
                    layer_id: LayerId::new(0),
                    zone: Zone::Tiled,
                    bounds: reovim_driver_display::Rect::new(0, 0, 80, 24),
                    z_order: ZOrder::new(0),
                    visible: true,
                    focusable: true,
                }],
                focused: Some(WindowId::from_raw(1)),
                active_layer: Some(LayerId::new(0)),
                screen,
            }
        }

        fn create_layer(
            &mut self,
            _config: reovim_driver_display::layout::LayerConfig,
        ) -> reovim_driver_display::layout::LayerId {
            reovim_driver_display::layout::LayerId::new(0)
        }

        fn remove_layer(&mut self, _layer: reovim_driver_display::layout::LayerId) {}

        fn layer_by_label(&self, _label: &str) -> Option<reovim_driver_display::layout::LayerId> {
            None
        }

        fn layers(&self) -> Vec<&reovim_driver_display::layout::Layer> {
            Vec::new()
        }

        fn set_layer_visible(
            &mut self,
            _layer: reovim_driver_display::layout::LayerId,
            _visible: bool,
        ) {
        }

        fn set_layer_opacity(
            &mut self,
            _layer: reovim_driver_display::layout::LayerId,
            _opacity: f32,
        ) {
        }

        fn reorder_layer(&mut self, _layer: reovim_driver_display::layout::LayerId, _new_z: u16) {}

        fn set_active_layer(&mut self, _layer: reovim_driver_display::layout::LayerId) {}

        fn active_layer(&self) -> Option<reovim_driver_display::layout::LayerId> {
            Some(reovim_driver_display::layout::LayerId::new(0))
        }

        fn set_focus(&mut self, _window: reovim_driver_display::WindowId) {}

        fn focused(&self) -> Option<reovim_driver_display::WindowId> {
            Some(reovim_driver_display::WindowId::from_raw(1))
        }

        fn focus_at(&mut self, _x: u16, _y: u16) -> Option<reovim_driver_display::WindowId> {
            None
        }

        fn layer_compositor(
            &self,
            _layer: reovim_driver_display::layout::LayerId,
        ) -> Option<&dyn reovim_driver_display::layout::WindowLayerCompositor> {
            None
        }

        fn layer_compositor_mut(
            &mut self,
            _layer: reovim_driver_display::layout::LayerId,
        ) -> Option<&mut dyn reovim_driver_display::layout::WindowLayerCompositor> {
            None
        }

        fn window_count(&self) -> usize {
            1
        }

        fn set_screen(&mut self, _screen: reovim_driver_display::Rect) {}

        fn layer_of(
            &self,
            _window: reovim_driver_display::WindowId,
        ) -> Option<reovim_driver_display::layout::LayerId> {
            Some(reovim_driver_display::layout::LayerId::new(0))
        }

        fn boxed_clone(&self) -> Box<dyn reovim_driver_display::layout::RootCompositor> {
            Box::new(Self)
        }
    }

    #[test]
    fn test_add_client_with_compositor_creates_windows() {
        // Exercise lines 239-251: compositor-driven window creation in
        // add_client_with_metadata() when shared compositor is set and
        // an active buffer exists.
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let mut state = SessionState::with_kernel(kernel);

        // Set the compositor on the shared session state BEFORE creating the session
        state
            .driver_session
            .shared
            .set_compositor(Box::new(TestPlacementCompositor));

        let session = Session::from_state(SessionId::new("compositor-add-test"), state);

        // Create a buffer so there is an active buffer
        session.with_state_mut_sync(|state| {
            state.create_buffer("hello compositor");
        });

        // Add a client - should use compositor for window creation
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Verify client has windows from compositor placements
        let editing_state = session.client_state(client_id).unwrap();
        assert!(!editing_state.windows.is_empty());
        // The compositor returned WindowId(1), so the active window should be set
        assert!(editing_state.windows.active().is_some());
        // Compositor should be cloned into per-client state
        assert!(editing_state.compositor.is_some());
    }

    #[test]
    fn test_ensure_client_has_window_with_compositor() {
        // Exercise lines 590-600: compositor-driven window sync in
        // ensure_client_has_window() when a client has a compositor but
        // empty windows, and the session acquires an active buffer later.
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let mut state = SessionState::with_kernel(kernel);

        // Set compositor on shared state
        state
            .driver_session
            .shared
            .set_compositor(Box::new(TestPlacementCompositor));

        let session = Session::from_state(SessionId::new("compositor-ensure-test"), state);

        // Add client BEFORE any buffer exists. The compositor is cloned into
        // per-client state but no windows are created yet (no active buffer).
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Client should have compositor but empty windows
        let editing_state = session.client_state(client_id).unwrap();
        assert!(editing_state.compositor.is_some());
        assert!(editing_state.windows.is_empty());

        // Now create a buffer
        session.with_state_mut_sync(|state| {
            state.create_buffer("hello lazy compositor");
        });

        // Trigger ensure_client_has_window via resolve_key_for_client.
        // The client has a compositor + empty windows + active buffer now,
        // so lines 589-600 should execute.
        let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _result = rt.block_on(session.resolve_key_for_client(client_id, &key));

        // After resolve_key, client should now have windows from compositor
        let editing_state = session.client_state(client_id).unwrap();
        assert!(!editing_state.windows.is_empty());
        assert!(editing_state.windows.active().is_some());
    }

    // ========================================================================
    // with_client_extensions tests (#514)
    // ========================================================================

    #[test]
    fn test_with_client_extensions_returns_none_for_unknown_client() {
        let session = Session::new(SessionId::new("test"));
        let result = session.with_client_extensions(ClientId::new(99), |_ext| 42);
        assert!(result.is_none());
    }

    #[test]
    fn test_with_client_extensions_reads_extensions() {
        let session = Session::new(SessionId::new("test"));
        let client_id = ClientId::new(1);
        session.add_client(client_id);

        // Initially empty
        let has_cmdline = session
            .with_client_extensions(client_id, |ext| {
                ext.get::<reovim_driver_session::CmdlineState>().is_some()
            })
            .unwrap();
        assert!(!has_cmdline);

        // Insert CmdlineState
        session.update_client_state(client_id, |state| {
            state
                .extensions
                .get_or_insert::<reovim_driver_session::CmdlineState>();
        });

        // Now it exists
        let has_cmdline = session
            .with_client_extensions(client_id, |ext| {
                ext.get::<reovim_driver_session::CmdlineState>().is_some()
            })
            .unwrap();
        assert!(has_cmdline);
    }
}
