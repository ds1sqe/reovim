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
    reovim_driver_layout::RootCompositor,
    reovim_driver_session::{
        CursorPosition, ExtensionMap, KeySequence, Selection, SelectionMode, TabPageSet, Viewport,
        Window, WindowLayout,
    },
    reovim_kernel::api::v1::{BufferId, Jumplist, MarkBank, ModeStack},
    reovim_types_text::{HistoryRing, RegisterBank},
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

    /// Per-client compositor for window layout (#474).
    ///
    /// Each client owns their own compositor, cloned from the shared template
    /// at join time. This ensures window IDs are consistent between the
    /// compositor (geometry) and per-client windows (cursor/viewport).
    ///
    /// Before this field, the shared compositor and per-client windows used
    /// independent ID namespaces, causing cross-namespace mismatches in
    /// notifications and state queries.
    pub compositor: Option<Box<dyn RootCompositor>>,

    /// Per-client tab pages (#401).
    ///
    /// Manages tab page lifecycle. Each tab can have its own window layout
    /// and compositor. Currently starts with a single default tab.
    /// Future work will integrate with `windows` and `compositor` fields
    /// so that `active_tab().windows()` becomes the source of truth.
    pub tabs: TabPageSet,

    /// Per-client register storage (#515).
    ///
    /// Each client owns their own registers (unnamed `""`, named `a-z`/`A-Z`).
    /// System clipboard (`+`, `*`) remains shared via `ClipboardProvider`.
    /// This prevents Client A's `"ayy` from overwriting Client B's register 'a'.
    pub registers: RegisterBank,

    /// Per-client clipboard history ring (#515).
    ///
    /// Tracks yank/delete history for numbered registers `0-9`.
    /// Each client has independent history so Client A's deletes don't
    /// shift Client B's numbered registers.
    pub clipboard_history: HistoryRing,

    /// Per-client local marks (a-z, per-client special marks) (#515).
    ///
    /// Each client owns their own local marks. Global marks (A-Z) remain
    /// shared in `KernelContext.global_marks`.
    pub local_marks: MarkBank,

    /// Per-client jump list for Ctrl-O / Ctrl-I navigation (#654).
    ///
    /// Each client owns their own jump list. Jump positions are recorded
    /// on cursor movements across buffer boundaries or large jumps.
    pub jumplist: Jumplist,

    /// Per-client active buffer (#471).
    ///
    /// Each client tracks which buffer they are viewing independently.
    /// New clients are initialized with the first kernel buffer (scratch).
    pub active_buffer: Option<BufferId>,

    /// Per-client terminal dimensions (width, height) (#471).
    ///
    /// Each client has independent terminal size. Initialized to VT100
    /// default (80, 24); updated when the client sends a resize RPC.
    pub terminal_size: (u16, u16),
}

impl std::fmt::Debug for EditingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditingState")
            .field("mode_stack", &self.mode_stack)
            .field("pending_keys", &self.pending_keys)
            .field("windows", &self.windows)
            .field("viewport", &self.viewport)
            .field("selection", &self.selection)
            .field("extensions", &self.extensions)
            .field("compositor", &self.compositor.as_ref().map(|_| "..."))
            .field("tabs", &self.tabs)
            .field("registers", &self.registers)
            .field("clipboard_history", &self.clipboard_history)
            .field("local_marks", &self.local_marks)
            .field("jumplist", &self.jumplist)
            .field("active_buffer", &self.active_buffer)
            .field("terminal_size", &self.terminal_size)
            .finish()
    }
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
            compositor: self.compositor.as_ref().map(|c| c.boxed_clone()), // #474
            tabs: self.tabs.clone(),         // #401
            registers: self.registers.clone(), // #515
            clipboard_history: self.clipboard_history.clone(), // #515
            local_marks: self.local_marks.clone(), // #515
            jumplist: self.jumplist.clone(), // #654
            active_buffer: self.active_buffer, // #471
            terminal_size: self.terminal_size, // #471
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
            windows: WindowLayout::empty(),
            viewport: Viewport::default(),
            selection: None,
            extensions: ExtensionMap::new(),
            compositor: None,
            tabs: TabPageSet::new(),
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
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
            extensions: ExtensionMap::new(),
            compositor: None,
            tabs: TabPageSet::new(),
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
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
            extensions: ExtensionMap::new(),
            compositor: None,
            tabs: TabPageSet::new(),
            registers: RegisterBank::new(),
            clipboard_history: HistoryRing::new(),
            local_marks: MarkBank::new(),
            jumplist: Jumplist::new(),
            active_buffer: None,
            terminal_size: (80, 24),
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

    /// Borrow the 7 per-client mutable fields as a [`ClientContext`].
    ///
    /// This bundles the fields that `SessionRuntime` needs, avoiding
    /// 7-argument parameter lists throughout the session execution chain.
    pub fn client_context(&mut self) -> reovim_driver_session::ClientContext<'_> {
        reovim_driver_session::ClientContext {
            mode_stack: &mut self.mode_stack,
            windows: &mut self.windows,
            extensions: &mut self.extensions,
            compositor: &mut self.compositor,
            tabs: &mut self.tabs,
            registers: &mut self.registers,
            clipboard_history: &mut self.clipboard_history,
            local_marks: &mut self.local_marks,
            jumplist: &mut self.jumplist,
            active_buffer: &mut self.active_buffer,
            terminal_size: &mut self.terminal_size,
        }
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
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::similar_names)] // client1/client2/clients are clear in test context
#[path = "client_tests.rs"]
mod tests;
