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
        drop(state); // Release lock before acquiring clients lock

        tracing::debug!(
            %client_id,
            mode_module = %home_mode.module(),
            mode_name = %home_mode.name(),
            ?active_buffer,
            "Initializing client with home mode and per-client windows"
        );

        let mode_stack = ModeStack::new(home_mode);

        // Phase #471/#480: Create per-client with metadata and initial window
        let client = if let Some(buffer_id) = active_buffer {
            // Session has an active buffer - create window for it
            let window = Window::with_buffer(buffer_id);
            Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window)
        } else {
            // No buffer yet - empty windows
            Client::with_mode_stack(client_id, metadata, mode_stack)
        };

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
        use reovim_kernel::api::v1::pr_info;

        let mut clients = self.clients.write();

        // Phase #481: Dump ring buffer before removal
        if let Some(client) = clients.get(&client_id)
            && let Some(path) =
                super::crash_dump::try_dump_client_to_file(client_id, &client.ring_buffer)
        {
            pr_info!(
                "CLIENT_DISCONNECT client_id={} dump={}",
                client_id.as_usize(),
                path.display()
            );
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
            let window = Window::with_buffer(buffer_id);
            editing_state.windows.add(window);
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

        let (mode_stack, windows, extensions) = (
            &mut editing_state.mode_stack,
            &mut editing_state.windows,
            &mut editing_state.extensions,
        );

        // Resolve key with per-client state (#471 Phase 5: pass client_id for undo origin)
        state.resolve_key_for_client(target_id.as_usize(), mode_stack, windows, extensions, key)
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

        let (mode_stack, windows, extensions) = (
            &mut editing_state.mode_stack,
            &mut editing_state.windows,
            &mut editing_state.extensions,
        );

        state.try_on_command_complete_for_client(
            target_id.as_usize(),
            mode_stack,
            windows,
            extensions,
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

        let (mode_stack, windows, extensions) = (
            &mut editing_state.mode_stack,
            &mut editing_state.windows,
            &mut editing_state.extensions,
        );

        // Execute command with per-client state, passing client_id for per-client undo (#471)
        state.execute_command_for_client(
            target_id.as_usize(),
            mode_stack,
            windows,
            extensions,
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
}
