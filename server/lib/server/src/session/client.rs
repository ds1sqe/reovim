//! Per-client role and editing state (#480 Client Architecture Unification).
//!
//! Defines the unified `Client` struct that replaces the old enum model.
//! All clients now have `EditingState` for local viewport, smooth transitions,
//! and pending keys buffer. The `relation` field controls input routing.
//!
//! # Architecture
//!
//! ```text
//! Session (room with shared buffers)
//! └─ clients: HashMap<ClientId, Client>
//!     └─ Client { id, relation, state, metadata }
//!         ├─ relation: None              ← Independent (input → self)
//!         ├─ relation: Following(X)      ← Spectator (input ignored)
//!         └─ relation: Sharing(X)        ← Collaboration (input → X)
//! ```
//!
//! # Behavior Matrix
//!
//! | Relation     | My Input       | I See          | Use Case            |
//! |--------------|----------------|----------------|---------------------|
//! | None         | → my state     | my state       | Solo editing        |
//! | Following(X) | ignored        | X's state      | Spectator/present   |
//! | Sharing(X)   | → X's state    | X's state      | Pair programming    |
//!
//! # State Transitions
//!
//! ```text
//! Independent ←→ Following(B) ←→ Sharing(B) ←→ Independent
//! ```
//!
//! Transitions are validated by [`Client::try_set_relation()`] which returns
//! [`TransitionResult`] indicating success or the required precondition.
//!
//! # Validation Rules
//!
//! - Cannot follow/share with self
//! - Cannot create cycles (A → B → A)
//! - Target client must exist
//! - Following → Sharing upgrade may require cursor sync

use std::{collections::HashMap, time::SystemTime};

use {
    reovim_driver_session::{
        CursorPosition, ExtensionMap, KeySequence, Selection, SelectionMode, Viewport, Window,
        WindowLayout,
    },
    reovim_kernel::api::v1::ModeStack,
};

use super::{ClientId, ring_buffer::ClientRingBuffer};

// ============================================================================
// ClientRelation
// ============================================================================

/// Relation to another client for input routing.
///
/// Used as `Option<ClientRelation>` where `None` means independent.
///
/// # Invariants
///
/// - `Following { target }`: Input is ignored, sees target's state
/// - `Sharing { with }`: Input routes to target's state, sees target's state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientRelation {
    /// Read-only spectator. Input is ignored, sees target's state.
    Following {
        /// Client ID being followed.
        target: ClientId,
    },
    /// Bidirectional co-editing. Input goes to target's state.
    Sharing {
        /// Client ID to share input with.
        with: ClientId,
    },
}

impl ClientRelation {
    /// Get the target client ID.
    #[must_use]
    pub const fn target_id(&self) -> ClientId {
        match *self {
            Self::Following { target } => target,
            Self::Sharing { with } => with,
        }
    }

    /// Check if this is a Following relation.
    #[must_use]
    pub const fn is_following(&self) -> bool {
        matches!(self, Self::Following { .. })
    }

    /// Check if this is a Sharing relation.
    #[must_use]
    pub const fn is_sharing(&self) -> bool {
        matches!(self, Self::Sharing { .. })
    }
}

// ============================================================================
// TransitionResult
// ============================================================================

/// Result of a relation transition attempt.
///
/// Returned by [`Client::try_set_relation()`] to indicate success or
/// the precondition that must be met before the transition can proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionResult {
    /// Transition succeeded.
    Ok,
    /// Transition requires cursor sync first (for Following → Sharing upgrade).
    ///
    /// The caller should sync the cursor to the target position, then retry.
    RequiresCursorSync {
        /// Current cursor position.
        current: CursorPosition,
        /// Target cursor position to sync to.
        target: CursorPosition,
    },
    /// Target client not found.
    TargetNotFound(ClientId),
    /// Cannot create cycle (A → B → A transitively).
    WouldCreateCycle,
    /// Cannot follow/share with self.
    CannotTargetSelf,
}

impl TransitionResult {
    /// Check if the transition succeeded.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Check if the transition requires cursor sync.
    #[must_use]
    pub const fn requires_cursor_sync(&self) -> bool {
        matches!(self, Self::RequiresCursorSync { .. })
    }
}

// ============================================================================
// Cycle Detection
// ============================================================================

/// Check if setting `start` to follow/share `target` would create a cycle.
///
/// A cycle occurs if traversing from `target` eventually leads back to `start`.
fn would_create_cycle(
    start: ClientId,
    target: ClientId,
    clients: &HashMap<ClientId, Client>,
) -> bool {
    would_create_cycle_impl(start, target, clients, 10)
}

fn would_create_cycle_impl(
    start: ClientId,
    target: ClientId,
    clients: &HashMap<ClientId, Client>,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false; // Safety limit
    }
    let Some(target_client) = clients.get(&target) else {
        return false;
    };
    match target_client.relation {
        Some(
            ClientRelation::Following { target: next } | ClientRelation::Sharing { with: next },
        ) => {
            if next == start {
                return true;
            }
            would_create_cycle_impl(start, next, clients, depth - 1)
        }
        None => false,
    }
}

// ============================================================================
// ClientMetadata
// ============================================================================

/// Metadata about a client connection.
///
/// Contains identity information for display and debugging.
#[derive(Debug, Clone)]
pub struct ClientMetadata {
    /// Client type identifier ("tui", "android", "web", "cli").
    pub client_type: String,
    /// User-friendly display name ("laptop", "phone").
    pub display_name: String,
    /// When the client joined (Unix milliseconds).
    pub joined_at_ms: u64,
}

impl ClientMetadata {
    /// Create new metadata with current timestamp.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // u128 millis to u64 - safe for next 500M years
    pub fn new(client_type: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            client_type: client_type.into(),
            display_name: display_name.into(),
            joined_at_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
        }
    }

    /// Create metadata with a specific timestamp (for testing).
    #[must_use]
    pub fn with_timestamp(
        client_type: impl Into<String>,
        display_name: impl Into<String>,
        joined_at_ms: u64,
    ) -> Self {
        Self {
            client_type: client_type.into(),
            display_name: display_name.into(),
            joined_at_ms,
        }
    }
}

impl Default for ClientMetadata {
    fn default() -> Self {
        Self::new("unknown", "unknown")
    }
}

// ============================================================================
// Client
// ============================================================================

/// A client in a session.
///
/// All clients have `EditingState` for local viewport, smooth transitions,
/// and pending keys buffer. The `relation` field controls input routing.
///
/// # Property Invariants
///
/// 1. `relation = None` implies client is independent (input → self)
/// 2. `relation = Some(Following{target})` implies input is ignored
/// 3. `relation = Some(Sharing{with})` implies input routes to `with`
/// 4. All clients ALWAYS have `state: EditingState` (non-optional)
/// 5. No cycles allowed: if A → B, then B cannot → A (directly or transitively)
#[derive(Debug, Clone)]
pub struct Client {
    /// Unique client identifier.
    pub id: ClientId,
    /// Relation to another client. `None` = independent.
    pub relation: Option<ClientRelation>,
    /// Editing state (mode, cursor, windows, etc.). ALWAYS present.
    pub state: EditingState,
    /// Client metadata (type, display name, join time).
    pub metadata: ClientMetadata,
    /// Per-client debug ring buffer (Phase #478/#481).
    pub ring_buffer: ClientRingBuffer,
}

impl Client {
    /// Create a new independent client with default editing state.
    #[must_use]
    pub fn new(id: ClientId, metadata: ClientMetadata) -> Self {
        Self {
            id,
            relation: None,
            state: EditingState::default(),
            metadata,
            ring_buffer: ClientRingBuffer::new(),
        }
    }

    /// Create client with specific mode stack.
    #[must_use]
    pub fn with_mode_stack(id: ClientId, metadata: ClientMetadata, mode_stack: ModeStack) -> Self {
        Self {
            id,
            relation: None,
            state: EditingState::with_mode_stack(mode_stack),
            metadata,
            ring_buffer: ClientRingBuffer::new(),
        }
    }

    /// Create client with mode stack and initial window.
    #[must_use]
    pub fn with_mode_stack_and_window(
        id: ClientId,
        metadata: ClientMetadata,
        mode_stack: ModeStack,
        window: Window,
    ) -> Self {
        Self {
            id,
            relation: None,
            state: EditingState::with_mode_stack_and_window(mode_stack, window),
            metadata,
            ring_buffer: ClientRingBuffer::new(),
        }
    }

    /// Get the ring buffer for this client.
    #[must_use]
    pub const fn ring_buffer(&self) -> &ClientRingBuffer {
        &self.ring_buffer
    }

    /// Check if client is independent (no relation).
    #[must_use]
    pub const fn is_independent(&self) -> bool {
        self.relation.is_none()
    }

    /// Check if client is following another.
    #[must_use]
    pub const fn is_following(&self) -> bool {
        matches!(self.relation, Some(ClientRelation::Following { .. }))
    }

    /// Check if client is sharing with another.
    #[must_use]
    pub const fn is_sharing(&self) -> bool {
        matches!(self.relation, Some(ClientRelation::Sharing { .. }))
    }

    /// Get the target client ID (for Following/Sharing), or `None` if independent.
    #[must_use]
    pub const fn target_id(&self) -> Option<ClientId> {
        match self.relation {
            Some(ClientRelation::Following { target }) => Some(target),
            Some(ClientRelation::Sharing { with }) => Some(with),
            None => None,
        }
    }

    // ========================================================================
    // State Transitions (Phase 2)
    // ========================================================================

    /// Validate a relation change without applying it.
    ///
    /// This is a static method that checks if a relation change is valid
    /// without mutating the client. Used by `Session::set_client_relation()`.
    ///
    /// # Validation Rules
    ///
    /// 1. Cannot target self (returns `CannotTargetSelf`)
    /// 2. Target must exist (returns `TargetNotFound`)
    /// 3. Cannot create cycles (returns `WouldCreateCycle`)
    /// 4. Following → Sharing with same target may require cursor sync
    #[must_use]
    pub fn validate_relation_change(
        client: &Self,
        new_relation: Option<ClientRelation>,
        clients: &HashMap<ClientId, Self>,
    ) -> TransitionResult {
        // Check self-reference
        if let Some(
            ClientRelation::Following { target } | ClientRelation::Sharing { with: target },
        ) = new_relation
        {
            if target == client.id {
                return TransitionResult::CannotTargetSelf;
            }
            // Check target exists
            let Some(target_client) = clients.get(&target) else {
                return TransitionResult::TargetNotFound(target);
            };
            // Check for cycles
            if would_create_cycle(client.id, target, clients) {
                return TransitionResult::WouldCreateCycle;
            }

            // Edge case: Following → Sharing with same target requires cursor sync
            if let (
                Some(ClientRelation::Following { target: old_target }),
                Some(ClientRelation::Sharing { with: new_target }),
            ) = (&client.relation, &new_relation)
            {
                // Only check cursor sync when upgrading from Follow to Share with same target
                if old_target == new_target {
                    let target_cursor = target_client
                        .state
                        .windows
                        .active()
                        .map_or_else(CursorPosition::default, |w| w.cursor);
                    let my_cursor = client
                        .state
                        .windows
                        .active()
                        .map_or_else(CursorPosition::default, |w| w.cursor);
                    if my_cursor != target_cursor {
                        return TransitionResult::RequiresCursorSync {
                            current: my_cursor,
                            target: target_cursor,
                        };
                    }
                }
            }
        }

        TransitionResult::Ok
    }

    /// Attempt to change relation with validation.
    ///
    /// Returns [`TransitionResult`] indicating success or required preconditions.
    ///
    /// # Validation Rules
    ///
    /// 1. Cannot target self (returns `CannotTargetSelf`)
    /// 2. Target must exist (returns `TargetNotFound`)
    /// 3. Cannot create cycles (returns `WouldCreateCycle`)
    /// 4. Following → Sharing with same target may require cursor sync
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Independent to Following
    /// let result = client.try_set_relation(
    ///     Some(ClientRelation::Following { target: other_id }),
    ///     &clients,
    /// );
    /// assert!(result.is_ok());
    ///
    /// // Back to independent
    /// let result = client.try_set_relation(None, &clients);
    /// assert!(result.is_ok());
    /// ```
    pub fn try_set_relation(
        &mut self,
        new_relation: Option<ClientRelation>,
        clients: &HashMap<ClientId, Self>,
    ) -> TransitionResult {
        let result = Self::validate_relation_change(self, new_relation, clients);
        if result.is_ok() {
            self.relation = new_relation;
        }
        result
    }

    /// Sync cursor to target client's position.
    ///
    /// Used for Following → Sharing transitions that require cursor alignment.
    pub fn sync_cursor_to(&mut self, target: &Self) {
        if let (Some(target_window), Some(my_window)) =
            (target.state.windows.active(), self.state.windows.active_mut())
        {
            my_window.cursor = target_window.cursor;
        }
    }

    /// Force set relation without validation.
    ///
    /// **Use sparingly** - prefer `try_set_relation()` for safety.
    /// This is useful for initialization or internal operations where
    /// validation has already been performed.
    pub const fn set_relation_unchecked(&mut self, relation: Option<ClientRelation>) {
        self.relation = relation;
    }

    /// Get the effective state for display.
    ///
    /// - Independent: own state
    /// - Following: target's state (with depth limit)
    /// - Sharing: target's state (with depth limit)
    ///
    /// Returns `None` if the target/chain doesn't exist or if there's a cycle.
    #[must_use]
    pub fn effective_state<'a>(
        &'a self,
        clients: &'a HashMap<ClientId, Self>,
    ) -> Option<&'a EditingState> {
        match self.relation {
            None => Some(&self.state),
            Some(
                ClientRelation::Following { target } | ClientRelation::Sharing { with: target },
            ) => Self::resolve_state(target, clients, 10),
        }
    }

    /// Get mutable reference to the effective state for input routing.
    ///
    /// - Independent: Returns `None` (caller should use `self.state` directly)
    /// - Following: Returns `None` (input is ignored)
    /// - Sharing: Returns target's state (recursively)
    ///
    /// Note: For independent clients, the caller must handle `self.state` specially
    /// due to Rust borrow rules (can't borrow self and return reference to self.state).
    pub fn effective_state_mut<'a>(
        &'a self,
        clients: &'a mut HashMap<ClientId, Self>,
    ) -> Option<&'a mut EditingState> {
        match self.relation {
            None => {
                // Independent: caller should use self.state directly
                // Can't return &mut self.state here due to borrow rules
                None
            }
            Some(ClientRelation::Following { .. }) => None, // Input ignored for followers
            Some(ClientRelation::Sharing { with }) => clients.get_mut(&with).map(|c| &mut c.state),
        }
    }

    /// Resolve state through the chain with depth limit.
    fn resolve_state(
        target: ClientId,
        clients: &HashMap<ClientId, Self>,
        depth: usize,
    ) -> Option<&EditingState> {
        if depth == 0 {
            return None; // Prevent infinite loops
        }

        let client = clients.get(&target)?;
        match client.relation {
            None => Some(&client.state),
            Some(
                ClientRelation::Following { target: next } | ClientRelation::Sharing { with: next },
            ) => Self::resolve_state(next, clients, depth - 1),
        }
    }
}

// ============================================================================
// EditingState
// ============================================================================

/// Editing state for a client.
///
/// Contains all per-client state needed for editing operations.
/// All clients have this state - it's not just for "owners" anymore.
///
/// # Multi-Client Isolation (#471)
///
/// Each client owns their own `WindowLayout` with independent cursors.
/// This ensures Client A's cursor/mode doesn't affect Client B.
/// Buffers are still shared (all clients see same text content).
///
/// # Client Model Mapping (#480)
///
/// This struct maps to `ClientViewState` in the common client model.
/// Only a subset is transmitted:
///
/// | `EditingState` field | `ClientViewState` field | Transform |
/// |----------------------|-------------------------|-----------|
/// | `mode_stack` | `mode` | `.current().name()` |
/// | `windows` | `cursor: Position` | `.focused().cursor` |
/// | `windows` | `buffer_id` | `.focused().buffer_id` |
/// | `selection` | `selection` | `.to_driver_selection()` |
///
/// Per-client editing state (#471, #477).
///
/// Contains all client-specific state including mode, windows, viewport,
/// selection, and module extensions. Each client has independent state
/// to prevent cross-client interference (e.g., Client A's pending count
/// affecting Client B's motions).
#[derive(Debug)]
pub struct EditingState {
    /// Mode stack (current mode on top).
    pub mode_stack: ModeStack,

    /// Keys accumulated but not yet processed.
    pub pending_keys: KeySequence,

    /// Per-client window layout with independent cursors (#471).
    ///
    /// Each window contains its own cursor position. This replaces
    /// the old shared `session.windows` that caused multi-client bugs.
    pub windows: WindowLayout,

    /// Viewport (visible area).
    pub viewport: Viewport,

    /// Active selection (for visual mode).
    pub selection: Option<ClientSelection>,

    /// Per-client module extensions (#477).
    ///
    /// Type-erased storage for module state like `VimSessionState`,
    /// `SearchState`, `CmdlineState`. Each client has independent
    /// extensions to prevent state leakage between clients.
    ///
    /// # Why Per-Client
    ///
    /// Without isolation, Client A pressing `5` (`pending_count=5`) would
    /// cause Client B's `j` to move 5 lines instead of 1. This field
    /// ensures complete module state isolation.
    pub extensions: ExtensionMap,
}

// Manual Clone implementation (#477).
//
// ExtensionMap doesn't implement Clone (contains Box<dyn SessionExtensionDyn>).
// Cloning creates fresh extensions - intentional for relation following where
// spectators should have their own independent module state.
impl Clone for EditingState {
    fn clone(&self) -> Self {
        Self {
            mode_stack: self.mode_stack.clone(),
            pending_keys: self.pending_keys.clone(),
            windows: self.windows.clone(),
            viewport: self.viewport, // Copy type
            selection: self.selection.clone(),
            extensions: ExtensionMap::new(), // Fresh extensions for cloned state
        }
    }
}

impl Default for EditingState {
    fn default() -> Self {
        // Use a placeholder mode - the actual home mode is set during session init
        let placeholder_mode = reovim_kernel::api::v1::ModeId::new(
            reovim_kernel::api::v1::ModuleId::new("default"),
            "normal",
        );
        Self {
            mode_stack: ModeStack::new(placeholder_mode),
            pending_keys: KeySequence::new(),
            windows: WindowLayout::empty(), // Per-client windows (#471)
            viewport: Viewport::default(),
            selection: None,
            extensions: ExtensionMap::new(), // Per-client extensions (#477)
        }
    }
}

impl EditingState {
    /// Create editing state with a specific mode stack.
    #[must_use]
    pub fn with_mode_stack(mode_stack: ModeStack) -> Self {
        Self {
            mode_stack,
            pending_keys: KeySequence::new(),
            windows: WindowLayout::empty(),
            viewport: Viewport::default(),
            selection: None,
            extensions: ExtensionMap::new(), // Per-client extensions (#477)
        }
    }

    /// Create editing state with mode stack and initial window.
    ///
    /// Used when a client joins a session that already has buffers.
    #[must_use]
    pub fn with_mode_stack_and_window(mode_stack: ModeStack, window: Window) -> Self {
        let mut windows = WindowLayout::empty();
        windows.add(window);
        Self {
            mode_stack,
            pending_keys: KeySequence::new(),
            windows,
            viewport: Viewport::default(),
            selection: None,
            extensions: ExtensionMap::new(), // Per-client extensions (#477)
        }
    }

    /// Get the current mode ID.
    #[must_use]
    pub fn current_mode(&self) -> &reovim_kernel::api::v1::ModeId {
        self.mode_stack.current()
    }

    /// Clear pending keys.
    pub fn clear_pending_keys(&mut self) {
        self.pending_keys.clear();
    }
}

// ============================================================================
// ClientSelection
// ============================================================================

/// Selection state for a client.
///
/// Wraps the driver's Selection with additional client-specific info.
#[derive(Debug, Clone)]
pub struct ClientSelection {
    /// Anchor position (where selection started).
    pub anchor: CursorPosition,

    /// Current cursor position (selection endpoint).
    pub cursor: CursorPosition,

    /// Selection mode (char/line/block).
    pub mode: SelectionMode,
}

impl ClientSelection {
    /// Create a new selection.
    #[must_use]
    pub const fn new(anchor: CursorPosition, cursor: CursorPosition, mode: SelectionMode) -> Self {
        Self {
            anchor,
            cursor,
            mode,
        }
    }

    /// Convert to the driver's Selection type.
    #[must_use]
    pub fn to_driver_selection(&self) -> Selection {
        Selection {
            start: self.anchor.into(),
            end: self.cursor.into(),
            mode: self.mode,
        }
    }
}

// ============================================================================
// Backward Compatibility - Old enum API
// ============================================================================

/// Old Client enum variants for backward compatibility during migration.
///
/// **DEPRECATED**: Use `Client` struct with `relation` field instead.
///
/// This module provides conversion from old `Client` enum patterns to new struct.
#[deprecated(since = "0.10.0", note = "Use Client struct with relation field")]
#[allow(dead_code)]
pub mod compat {
    use reovim_kernel::api::v1::ModeStack;

    use super::{Client, ClientId, ClientMetadata, ClientRelation, EditingState};

    /// Create an Owner-style client (independent with state).
    ///
    /// **DEPRECATED**: Use `Client::new()` or `Client::with_mode_stack()` instead.
    #[must_use]
    pub fn new_owner() -> Client {
        Client::new(ClientId::new(0), ClientMetadata::default())
    }

    /// Create an Owner-style client with mode stack.
    ///
    /// **DEPRECATED**: Use `Client::with_mode_stack()` instead.
    #[must_use]
    pub fn owner_with_mode(mode_stack: ModeStack) -> Client {
        Client::with_mode_stack(ClientId::new(0), ClientMetadata::default(), mode_stack)
    }

    /// Create a Follow-style client.
    ///
    /// **DEPRECATED**: Use `Client::new()` then set `relation = Some(ClientRelation::Following { target })`.
    #[must_use]
    pub fn follow(target: ClientId) -> Client {
        Client {
            id: ClientId::new(0),
            relation: Some(ClientRelation::Following { target }),
            state: EditingState::default(),
            metadata: ClientMetadata::default(),
            ring_buffer: super::ClientRingBuffer::new(),
        }
    }

    /// Create a Share-style client.
    ///
    /// **DEPRECATED**: Use `Client::new()` then set `relation = Some(ClientRelation::Sharing { with })`.
    #[must_use]
    pub fn share(owner: ClientId) -> Client {
        Client {
            id: ClientId::new(0),
            relation: Some(ClientRelation::Sharing { with: owner }),
            state: EditingState::default(),
            metadata: ClientMetadata::default(),
            ring_buffer: super::ClientRingBuffer::new(),
        }
    }

    /// Check if client is an "owner" (independent).
    #[must_use]
    pub const fn is_owner(client: &Client) -> bool {
        client.is_independent()
    }

    /// Check if client is a "follower".
    #[must_use]
    pub const fn is_follower(client: &Client) -> bool {
        client.is_following()
    }

    /// Check if client is "sharing".
    #[must_use]
    pub const fn is_sharing(client: &Client) -> bool {
        client.is_sharing()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::similar_names)] // client1/client2/clients are clear in test context
mod tests {
    use std::collections::HashMap;

    use {
        reovim_driver_session::{CursorPosition, SelectionMode},
        reovim_kernel::api::v1::{ModeStack, ModuleId},
    };

    use super::{
        Client, ClientId, ClientMetadata, ClientRelation, ClientSelection, EditingState,
        TransitionResult,
    };

    fn test_mode_stack() -> ModeStack {
        let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
        ModeStack::new(mode)
    }

    fn test_metadata() -> ClientMetadata {
        ClientMetadata::new("tui", "test-laptop")
    }

    #[test]
    fn test_client_new_independent() {
        let client = Client::new(ClientId::new(1), test_metadata());
        assert!(client.is_independent());
        assert!(!client.is_following());
        assert!(!client.is_sharing());
        assert!(client.target_id().is_none());
        assert_eq!(client.id, ClientId::new(1));
    }

    #[test]
    fn test_client_with_mode_stack() {
        let mode_stack = test_mode_stack();
        let client = Client::with_mode_stack(ClientId::new(1), test_metadata(), mode_stack.clone());

        assert!(client.is_independent());
        assert_eq!(client.state.mode_stack.current(), mode_stack.current());
    }

    #[test]
    fn test_client_is_following() {
        let mut client = Client::new(ClientId::new(1), test_metadata());
        client.relation = Some(ClientRelation::Following {
            target: ClientId::new(42),
        });

        assert!(client.is_following());
        assert!(!client.is_independent());
        assert!(!client.is_sharing());
        assert_eq!(client.target_id(), Some(ClientId::new(42)));
    }

    #[test]
    fn test_client_is_sharing() {
        let mut client = Client::new(ClientId::new(1), test_metadata());
        client.relation = Some(ClientRelation::Sharing {
            with: ClientId::new(42),
        });

        assert!(client.is_sharing());
        assert!(!client.is_independent());
        assert!(!client.is_following());
        assert_eq!(client.target_id(), Some(ClientId::new(42)));
    }

    #[test]
    fn test_effective_state_independent() {
        let client = Client::new(ClientId::new(1), test_metadata());
        let clients = HashMap::new();

        let state = client.effective_state(&clients);
        assert!(state.is_some());
    }

    #[test]
    fn test_effective_state_following() {
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(owner_id, Client::new(owner_id, test_metadata()));

        let mut follower = Client::new(follower_id, test_metadata());
        follower.relation = Some(ClientRelation::Following { target: owner_id });
        clients.insert(follower_id, follower);

        let follower = clients.get(&follower_id).unwrap();
        let state = follower.effective_state(&clients);

        // Should return the owner's state
        assert!(state.is_some());
    }

    #[test]
    fn test_effective_state_sharing() {
        let owner_id = ClientId::new(1);
        let sharer_id = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(owner_id, Client::new(owner_id, test_metadata()));

        let mut sharer = Client::new(sharer_id, test_metadata());
        sharer.relation = Some(ClientRelation::Sharing { with: owner_id });
        clients.insert(sharer_id, sharer);

        let sharer = clients.get(&sharer_id).unwrap();
        let state = sharer.effective_state(&clients);

        // Should return the owner's state
        assert!(state.is_some());
    }

    #[test]
    fn test_effective_state_missing_target() {
        let mut client = Client::new(ClientId::new(1), test_metadata());
        client.relation = Some(ClientRelation::Following {
            target: ClientId::new(999),
        }); // Non-existent
        let clients = HashMap::new();

        let state = client.effective_state(&clients);
        assert!(state.is_none());
    }

    #[test]
    fn test_effective_state_chain() {
        // Test: Client 3 follows Client 2 who follows Client 1 (independent)
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);
        let id3 = ClientId::new(3);

        let mut clients = HashMap::new();
        clients.insert(id1, Client::new(id1, test_metadata()));

        let mut mid = Client::new(id2, test_metadata());
        mid.relation = Some(ClientRelation::Following { target: id1 });
        clients.insert(id2, mid);

        let mut end = Client::new(id3, test_metadata());
        end.relation = Some(ClientRelation::Following { target: id2 });
        clients.insert(id3, end);

        let end_client = clients.get(&id3).unwrap();
        let state = end_client.effective_state(&clients);

        // Should resolve through the chain to owner's state
        assert!(state.is_some());
    }

    #[test]
    fn test_effective_state_cycle_protection() {
        // Create a cycle: 1 → 2 → 3 → 1
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);
        let id3 = ClientId::new(3);

        let mut clients = HashMap::new();

        let mut c1 = Client::new(id1, test_metadata());
        c1.relation = Some(ClientRelation::Following { target: id3 });
        clients.insert(id1, c1);

        let mut c2 = Client::new(id2, test_metadata());
        c2.relation = Some(ClientRelation::Following { target: id1 });
        clients.insert(id2, c2);

        let mut c3 = Client::new(id3, test_metadata());
        c3.relation = Some(ClientRelation::Following { target: id2 });
        clients.insert(id3, c3);

        let client = clients.get(&id1).unwrap();
        let state = client.effective_state(&clients);

        // Should return None due to cycle (depth limit exceeded)
        assert!(state.is_none());
    }

    #[test]
    fn test_effective_state_mut_following_ignored() {
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(owner_id, Client::new(owner_id, test_metadata()));

        let mut follower = Client::new(follower_id, test_metadata());
        follower.relation = Some(ClientRelation::Following { target: owner_id });

        // Following clients should get None (input ignored)
        let state = follower.effective_state_mut(&mut clients);
        assert!(state.is_none());
    }

    #[test]
    fn test_editing_state_default() {
        let state = EditingState::default();

        assert!(state.pending_keys.is_empty());
        assert!(state.windows.is_empty()); // Per-client windows (#471)
        assert!(state.selection.is_none());
    }

    #[test]
    fn test_editing_state_with_mode_stack() {
        let mode_stack = test_mode_stack();
        let state = EditingState::with_mode_stack(mode_stack.clone());

        assert_eq!(state.mode_stack.current(), mode_stack.current());
    }

    #[test]
    fn test_editing_state_clear_pending_keys() {
        let mut state = EditingState::default();
        state.pending_keys.push("d".to_string());
        state.pending_keys.push("w".to_string());

        assert!(!state.pending_keys.is_empty());
        state.clear_pending_keys();
        assert!(state.pending_keys.is_empty());
    }

    #[test]
    fn test_client_selection() {
        let anchor = CursorPosition::new(0, 0);
        let cursor = CursorPosition::new(5, 10);
        let selection = ClientSelection::new(anchor, cursor, SelectionMode::Character);

        assert_eq!(selection.anchor, anchor);
        assert_eq!(selection.cursor, cursor);
        assert_eq!(selection.mode, SelectionMode::Character);

        let driver_sel = selection.to_driver_selection();
        assert_eq!(driver_sel.mode, SelectionMode::Character);
    }

    #[test]
    fn test_client_metadata_new() {
        let metadata = ClientMetadata::new("tui", "my-laptop");
        assert_eq!(metadata.client_type, "tui");
        assert_eq!(metadata.display_name, "my-laptop");
        assert!(metadata.joined_at_ms > 0);
    }

    #[test]
    fn test_client_relation_following() {
        let relation = ClientRelation::Following {
            target: ClientId::new(42),
        };
        assert!(relation.is_following());
        assert!(!relation.is_sharing());
        assert_eq!(relation.target_id(), ClientId::new(42));
    }

    #[test]
    fn test_client_relation_sharing() {
        let relation = ClientRelation::Sharing {
            with: ClientId::new(42),
        };
        assert!(relation.is_sharing());
        assert!(!relation.is_following());
        assert_eq!(relation.target_id(), ClientId::new(42));
    }

    // =========================================================================
    // Phase 2: State Machine Tests
    // =========================================================================

    #[test]
    fn test_transition_independent_to_following() {
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(id1, Client::new(id1, test_metadata()));
        clients.insert(id2, Client::new(id2, test_metadata()));

        let mut client1 = clients.remove(&id1).unwrap();
        assert!(client1.is_independent());

        let result =
            client1.try_set_relation(Some(ClientRelation::Following { target: id2 }), &clients);
        assert!(result.is_ok());
        assert!(client1.is_following());
        assert_eq!(client1.target_id(), Some(id2));
    }

    #[test]
    fn test_transition_independent_to_sharing() {
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(id1, Client::new(id1, test_metadata()));
        clients.insert(id2, Client::new(id2, test_metadata()));

        let mut client1 = clients.remove(&id1).unwrap();
        assert!(client1.is_independent());

        let result =
            client1.try_set_relation(Some(ClientRelation::Sharing { with: id2 }), &clients);
        assert!(result.is_ok());
        assert!(client1.is_sharing());
        assert_eq!(client1.target_id(), Some(id2));
    }

    #[test]
    fn test_transition_following_to_independent() {
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(id2, Client::new(id2, test_metadata()));

        let mut client1 = Client::new(id1, test_metadata());
        client1.relation = Some(ClientRelation::Following { target: id2 });
        assert!(client1.is_following());

        let result = client1.try_set_relation(None, &clients);
        assert!(result.is_ok());
        assert!(client1.is_independent());
    }

    #[test]
    fn test_transition_sharing_to_independent() {
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(id2, Client::new(id2, test_metadata()));

        let mut client1 = Client::new(id1, test_metadata());
        client1.relation = Some(ClientRelation::Sharing { with: id2 });
        assert!(client1.is_sharing());

        let result = client1.try_set_relation(None, &clients);
        assert!(result.is_ok());
        assert!(client1.is_independent());
    }

    #[test]
    fn test_transition_sharing_to_following() {
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(id2, Client::new(id2, test_metadata()));

        let mut client1 = Client::new(id1, test_metadata());
        client1.relation = Some(ClientRelation::Sharing { with: id2 });

        let result =
            client1.try_set_relation(Some(ClientRelation::Following { target: id2 }), &clients);
        assert!(result.is_ok());
        assert!(client1.is_following());
    }

    #[test]
    fn test_transition_cannot_follow_self() {
        let id1 = ClientId::new(1);
        let clients = HashMap::new();

        let mut client1 = Client::new(id1, test_metadata());
        let result =
            client1.try_set_relation(Some(ClientRelation::Following { target: id1 }), &clients);
        assert_eq!(result, TransitionResult::CannotTargetSelf);
        assert!(client1.is_independent()); // Relation unchanged
    }

    #[test]
    fn test_transition_cannot_share_with_self() {
        let id1 = ClientId::new(1);
        let clients = HashMap::new();

        let mut client1 = Client::new(id1, test_metadata());
        let result =
            client1.try_set_relation(Some(ClientRelation::Sharing { with: id1 }), &clients);
        assert_eq!(result, TransitionResult::CannotTargetSelf);
        assert!(client1.is_independent()); // Relation unchanged
    }

    #[test]
    fn test_transition_target_not_found() {
        let id1 = ClientId::new(1);
        let id999 = ClientId::new(999);
        let clients = HashMap::new();

        let mut client1 = Client::new(id1, test_metadata());
        let result =
            client1.try_set_relation(Some(ClientRelation::Following { target: id999 }), &clients);
        assert_eq!(result, TransitionResult::TargetNotFound(id999));
        assert!(client1.is_independent()); // Relation unchanged
    }

    #[test]
    fn test_transition_prevents_cycle() {
        // Setup: id2 follows id1 (independent)
        // Attempt: id1 follows id2 → should fail (would create cycle)
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut clients = HashMap::new();

        let mut client2 = Client::new(id2, test_metadata());
        client2.relation = Some(ClientRelation::Following { target: id1 });
        clients.insert(id2, client2);

        let mut client1 = Client::new(id1, test_metadata());
        let result =
            client1.try_set_relation(Some(ClientRelation::Following { target: id2 }), &clients);
        assert_eq!(result, TransitionResult::WouldCreateCycle);
        assert!(client1.is_independent()); // Relation unchanged
    }

    #[test]
    fn test_transition_prevents_longer_cycle() {
        // Setup: id2 → id3 → id1 (chain)
        // Attempt: id1 → id2 → should fail (would create cycle)
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);
        let id3 = ClientId::new(3);

        let mut clients = HashMap::new();

        let mut client3 = Client::new(id3, test_metadata());
        client3.relation = Some(ClientRelation::Following { target: id1 });
        clients.insert(id3, client3);

        let mut client2 = Client::new(id2, test_metadata());
        client2.relation = Some(ClientRelation::Following { target: id3 });
        clients.insert(id2, client2);

        let mut client1 = Client::new(id1, test_metadata());
        let result =
            client1.try_set_relation(Some(ClientRelation::Following { target: id2 }), &clients);
        assert_eq!(result, TransitionResult::WouldCreateCycle);
    }

    #[test]
    fn test_transition_result_is_ok() {
        assert!(TransitionResult::Ok.is_ok());
        assert!(!TransitionResult::CannotTargetSelf.is_ok());
        assert!(!TransitionResult::WouldCreateCycle.is_ok());
    }

    #[test]
    fn test_transition_result_requires_cursor_sync() {
        let result = TransitionResult::RequiresCursorSync {
            current: CursorPosition::default(),
            target: CursorPosition::new(5, 10),
        };
        assert!(result.requires_cursor_sync());
        assert!(!TransitionResult::Ok.requires_cursor_sync());
    }

    #[test]
    fn test_sync_cursor_to() {
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        // Create target client with a window and cursor
        let mut target = Client::with_mode_stack_and_window(
            id2,
            test_metadata(),
            test_mode_stack(),
            reovim_driver_session::Window::new(),
        );
        // Set target's cursor position
        if let Some(w) = target.state.windows.active_mut() {
            w.cursor = CursorPosition::new(10, 20);
        }

        // Create source client with a window
        let mut source = Client::with_mode_stack_and_window(
            id1,
            test_metadata(),
            test_mode_stack(),
            reovim_driver_session::Window::new(),
        );
        // Verify initial cursor is at default
        assert_eq!(source.state.windows.active().unwrap().cursor, CursorPosition::default());

        // Sync cursor
        source.sync_cursor_to(&target);

        // Verify cursor was synced
        assert_eq!(source.state.windows.active().unwrap().cursor, CursorPosition::new(10, 20));
    }

    #[test]
    fn test_set_relation_unchecked() {
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut client = Client::new(id1, test_metadata());
        assert!(client.is_independent());

        // Set relation unchecked (no validation)
        client.set_relation_unchecked(Some(ClientRelation::Following { target: id2 }));
        assert!(client.is_following());
        assert_eq!(client.target_id(), Some(id2));

        // Back to independent
        client.set_relation_unchecked(None);
        assert!(client.is_independent());
    }

    // =========================================================================
    // Coverage: RequiresCursorSync path
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_validate_following_to_sharing_requires_cursor_sync() {
        // Setup: client1 follows client2, then tries to upgrade to Sharing
        // with mismatched cursors -> RequiresCursorSync
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut clients = HashMap::new();

        // Client 2 (target) with a window and cursor at (5, 10)
        let mut target = Client::with_mode_stack_and_window(
            id2,
            test_metadata(),
            test_mode_stack(),
            reovim_driver_session::Window::new(),
        );
        if let Some(w) = target.state.windows.active_mut() {
            w.cursor = CursorPosition::new(5, 10);
        }
        clients.insert(id2, target);

        // Client 1 (follower) with a window and cursor at default (0, 0)
        let mut follower = Client::with_mode_stack_and_window(
            id1,
            test_metadata(),
            test_mode_stack(),
            reovim_driver_session::Window::new(),
        );
        follower.relation = Some(ClientRelation::Following { target: id2 });

        // Attempt upgrade from Following(id2) to Sharing(id2) with cursor mismatch
        let result = Client::validate_relation_change(
            &follower,
            Some(ClientRelation::Sharing { with: id2 }),
            &clients,
        );

        assert!(result.requires_cursor_sync());
        match result {
            TransitionResult::RequiresCursorSync { current, target } => {
                assert_eq!(current, CursorPosition::default());
                assert_eq!(target, CursorPosition::new(5, 10));
            }
            other => panic!("Expected RequiresCursorSync, got {other:?}"),
        }
    }

    #[test]
    fn test_validate_following_to_sharing_same_cursor_ok() {
        // Same target, cursors already match -> Ok
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut clients = HashMap::new();

        let target = Client::with_mode_stack_and_window(
            id2,
            test_metadata(),
            test_mode_stack(),
            reovim_driver_session::Window::new(),
        );
        clients.insert(id2, target);

        let mut follower = Client::with_mode_stack_and_window(
            id1,
            test_metadata(),
            test_mode_stack(),
            reovim_driver_session::Window::new(),
        );
        follower.relation = Some(ClientRelation::Following { target: id2 });

        // Cursors both at default (0,0) -> should succeed
        let result = Client::validate_relation_change(
            &follower,
            Some(ClientRelation::Sharing { with: id2 }),
            &clients,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_following_to_sharing_different_target_no_sync() {
        // Following target A, trying to Share with target B -> no cursor sync check
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);
        let id3 = ClientId::new(3);

        let mut clients = HashMap::new();
        clients.insert(id2, Client::new(id2, test_metadata()));
        clients.insert(id3, Client::new(id3, test_metadata()));

        let mut follower = Client::new(id1, test_metadata());
        follower.relation = Some(ClientRelation::Following { target: id2 });

        // Following id2, but sharing with id3 -> different targets, no sync check
        let result = Client::validate_relation_change(
            &follower,
            Some(ClientRelation::Sharing { with: id3 }),
            &clients,
        );
        assert!(result.is_ok());
    }

    // =========================================================================
    // Coverage: effective_state_mut Sharing case
    // =========================================================================

    #[test]
    fn test_effective_state_mut_sharing_returns_target_state() {
        let owner_id = ClientId::new(1);
        let sharer_id = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(owner_id, Client::new(owner_id, test_metadata()));

        let mut sharer = Client::new(sharer_id, test_metadata());
        sharer.relation = Some(ClientRelation::Sharing { with: owner_id });

        // Sharing clients should get mutable access to owner's state
        let state = sharer.effective_state_mut(&mut clients);
        assert!(state.is_some());
    }

    #[test]
    fn test_effective_state_mut_independent_returns_none() {
        let id = ClientId::new(1);
        let client = Client::new(id, test_metadata());
        let mut clients = HashMap::new();

        // Independent returns None (caller should use client.state directly)
        let state = client.effective_state_mut(&mut clients);
        assert!(state.is_none());
    }

    #[test]
    fn test_effective_state_mut_sharing_missing_target() {
        let sharer_id = ClientId::new(1);
        let missing_id = ClientId::new(999);

        let mut sharer = Client::new(sharer_id, test_metadata());
        sharer.relation = Some(ClientRelation::Sharing { with: missing_id });

        let mut clients = HashMap::new();
        let state = sharer.effective_state_mut(&mut clients);
        assert!(state.is_none());
    }

    // =========================================================================
    // Coverage: EditingState::clone
    // =========================================================================

    #[test]
    fn test_editing_state_clone() {
        let mut state = EditingState::default();
        state.pending_keys.push("d".to_string());
        state.selection = Some(ClientSelection::new(
            CursorPosition::new(0, 0),
            CursorPosition::new(1, 5),
            SelectionMode::Character,
        ));

        let cloned = state.clone();

        // Cloned should have same mode stack
        assert_eq!(cloned.mode_stack.current().name(), state.mode_stack.current().name());
        // Cloned should have same pending keys
        assert!(!cloned.pending_keys.is_empty());
        // Cloned should have same selection
        assert!(cloned.selection.is_some());
        // Cloned should have FRESH extensions (not copied)
        // Extensions are intentionally not cloned for isolation
        let _ = cloned.extensions;
    }

    // =========================================================================
    // Coverage: ClientMetadata::with_timestamp
    // =========================================================================

    #[test]
    fn test_client_metadata_with_timestamp() {
        let metadata = super::ClientMetadata::with_timestamp("android", "phone", 1_700_000_000_000);
        assert_eq!(metadata.client_type, "android");
        assert_eq!(metadata.display_name, "phone");
        assert_eq!(metadata.joined_at_ms, 1_700_000_000_000);
    }

    // =========================================================================
    // Coverage: compat module
    // =========================================================================

    #[test]
    #[allow(deprecated)]
    fn test_compat_new_owner() {
        let client = super::compat::new_owner();
        assert!(super::compat::is_owner(&client));
        assert!(!super::compat::is_follower(&client));
        assert!(!super::compat::is_sharing(&client));
        assert!(client.is_independent());
    }

    #[test]
    #[allow(deprecated)]
    fn test_compat_owner_with_mode() {
        let mode_stack = test_mode_stack();
        let client = super::compat::owner_with_mode(mode_stack.clone());
        assert!(super::compat::is_owner(&client));
        assert_eq!(client.state.mode_stack.current().name(), mode_stack.current().name());
    }

    #[test]
    #[allow(deprecated)]
    fn test_compat_follow() {
        let target = ClientId::new(42);
        let client = super::compat::follow(target);
        assert!(super::compat::is_follower(&client));
        assert!(!super::compat::is_owner(&client));
        assert!(client.is_following());
        assert_eq!(client.target_id(), Some(target));
    }

    #[test]
    #[allow(deprecated)]
    fn test_compat_share() {
        let owner = ClientId::new(10);
        let client = super::compat::share(owner);
        assert!(super::compat::is_sharing(&client));
        assert!(!super::compat::is_owner(&client));
        assert!(client.is_sharing());
        assert_eq!(client.target_id(), Some(owner));
    }

    // =========================================================================
    // Coverage: ClientRelation target_id for both variants
    // =========================================================================

    #[test]
    fn test_client_ring_buffer_accessor() {
        let client = Client::new(ClientId::new(1), test_metadata());
        let rb = client.ring_buffer();
        // Just verify accessor works
        let _ = rb;
    }

    // =========================================================================
    // Coverage: EditingState::with_mode_stack_and_window
    // =========================================================================

    #[test]
    fn test_editing_state_with_mode_stack_and_window() {
        let mode_stack = test_mode_stack();
        let window = reovim_driver_session::Window::new();
        let state = EditingState::with_mode_stack_and_window(mode_stack.clone(), window);

        assert_eq!(state.mode_stack.current().name(), mode_stack.current().name());
        assert!(!state.windows.is_empty());
        assert!(state.selection.is_none());
        assert!(state.pending_keys.is_empty());
    }

    #[test]
    fn test_editing_state_current_mode() {
        let state = EditingState::default();
        let mode = state.current_mode();
        assert_eq!(mode.name(), "normal");
    }

    // =========================================================================
    // Coverage: ClientSelection::to_driver_selection all modes
    // =========================================================================

    #[test]
    fn test_client_selection_line_mode() {
        let selection = ClientSelection::new(
            CursorPosition::new(0, 0),
            CursorPosition::new(3, 0),
            SelectionMode::Line,
        );
        let driver_sel = selection.to_driver_selection();
        assert_eq!(driver_sel.mode, SelectionMode::Line);
    }

    #[test]
    fn test_client_selection_block_mode() {
        let selection = ClientSelection::new(
            CursorPosition::new(1, 2),
            CursorPosition::new(3, 4),
            SelectionMode::Block,
        );
        let driver_sel = selection.to_driver_selection();
        assert_eq!(driver_sel.mode, SelectionMode::Block);
    }

    // =========================================================================
    // Coverage: cycle detection depth limit
    // =========================================================================

    #[test]
    fn test_cycle_detection_depth_limit() {
        // Create a very long chain to test depth limit (10 hops)
        let mut clients = HashMap::new();

        // Chain: id0 → id1 → id2 → ... → id10 → id11
        for i in 0..12 {
            let id = ClientId::new(i);
            let mut client = Client::new(id, test_metadata());
            if i > 0 {
                client.relation = Some(ClientRelation::Following {
                    target: ClientId::new(i + 1),
                });
            }
            clients.insert(id, client);
        }

        // Try to make id11 point to id0 (would create cycle if depth wasn't limited)
        let mut last_client = clients.remove(&ClientId::new(11)).unwrap();
        let result = last_client.try_set_relation(
            Some(ClientRelation::Following {
                target: ClientId::new(0),
            }),
            &clients,
        );

        // Should succeed because depth limit prevents cycle detection at depth 10
        assert!(result.is_ok());
    }

    #[test]
    fn test_cycle_detection_missing_intermediate() {
        // Chain with missing intermediate: id1 → id2 (missing) → ...
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut clients = HashMap::new();

        let mut client1 = Client::new(id1, test_metadata());
        client1.relation = Some(ClientRelation::Following { target: id2 });
        clients.insert(id1, client1);

        // id2 doesn't exist, so cycle check should return false (no cycle)
        let mut test_client = Client::new(ClientId::new(99), test_metadata());
        let result =
            test_client.try_set_relation(Some(ClientRelation::Following { target: id1 }), &clients);

        // Should succeed - can't detect cycle through missing client
        assert!(result.is_ok());
    }

    #[test]
    fn test_effective_state_mut_independent_borrow_semantics() {
        // Test that independent clients return None from effective_state_mut
        // (because of Rust borrow rules - can't return &mut self.state)
        let id = ClientId::new(1);
        let client = Client::new(id, test_metadata());
        let mut clients = HashMap::new();

        let state = client.effective_state_mut(&mut clients);
        assert!(state.is_none());

        // Caller should use client.state directly for independent clients
        assert!(client.state.pending_keys.is_empty());
    }

    #[test]
    fn test_sync_cursor_to_no_windows() {
        // Test sync_cursor_to when one or both clients have no windows
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);

        let mut source = Client::new(id1, test_metadata());
        let target = Client::new(id2, test_metadata());

        // Both have no windows - should not crash
        source.sync_cursor_to(&target);

        // Verify nothing changed (no windows to sync)
        assert!(source.state.windows.active().is_none());
    }

    #[test]
    fn test_client_metadata_default() {
        let metadata = ClientMetadata::default();
        assert_eq!(metadata.client_type, "unknown");
        assert_eq!(metadata.display_name, "unknown");
        assert!(metadata.joined_at_ms > 0);
    }

    #[test]
    fn test_validate_relation_change_independent_to_none() {
        // Changing from independent (None) to independent (None) should succeed
        let id = ClientId::new(1);
        let client = Client::new(id, test_metadata());
        let clients = HashMap::new();

        let result = Client::validate_relation_change(&client, None, &clients);
        assert!(result.is_ok());
    }

    #[test]
    fn test_effective_state_sharing_chain() {
        // Test: Client 3 shares with Client 2 who shares with Client 1
        let id1 = ClientId::new(1);
        let id2 = ClientId::new(2);
        let id3 = ClientId::new(3);

        let mut clients = HashMap::new();
        clients.insert(id1, Client::new(id1, test_metadata()));

        let mut mid = Client::new(id2, test_metadata());
        mid.relation = Some(ClientRelation::Sharing { with: id1 });
        clients.insert(id2, mid);

        let mut end = Client::new(id3, test_metadata());
        end.relation = Some(ClientRelation::Sharing { with: id2 });
        clients.insert(id3, end);

        let end_client = clients.get(&id3).unwrap();
        let state = end_client.effective_state(&clients);

        // Should resolve through the chain to owner's state
        assert!(state.is_some());
    }

    #[test]
    fn test_client_with_mode_stack_and_window() {
        let id = ClientId::new(1);
        let mode_stack = test_mode_stack();
        let window = reovim_driver_session::Window::new();

        let client =
            Client::with_mode_stack_and_window(id, test_metadata(), mode_stack.clone(), window);

        assert!(client.is_independent());
        assert_eq!(client.state.mode_stack.current().name(), mode_stack.current().name());
        assert!(!client.state.windows.is_empty());
        assert_eq!(client.id, id);
    }

    #[test]
    fn test_would_create_cycle_impl_depth_limit() {
        use super::would_create_cycle;

        // Build a long chain: A -> B -> C -> ... that exceeds depth 10
        let mut clients = HashMap::new();
        for i in 0..12 {
            let mut client = Client::new(ClientId::new(i), test_metadata());
            if i < 11 {
                client.relation = Some(ClientRelation::Following {
                    target: ClientId::new(i + 1),
                });
            }
            clients.insert(ClientId::new(i), client);
        }
        // No actual cycle, but chain length > 10.
        // would_create_cycle starts traversing from target (1) looking for start (0).
        // Chain: 1->2->3->...->11 (no client 12). Should return false (no cycle found).
        assert!(!would_create_cycle(ClientId::new(0), ClientId::new(1), &clients));

        // Test with actual cycle at depth > 10
        // Make client 11 point back to client 1 (creates cycle 1->2->...->11->1)
        clients.get_mut(&ClientId::new(11)).unwrap().relation = Some(ClientRelation::Following {
            target: ClientId::new(1),
        });
        // would_create_cycle(0, 1, ...) traverses 1->2->...->10 (depth exhausted at 10)
        // It should return false because the safety limit prevents further traversal
        assert!(!would_create_cycle(ClientId::new(0), ClientId::new(1), &clients));
    }
}
