//! Per-client role and editing state (Phase 11.2, Epic #465).
//!
//! Defines the `Client` enum that captures the Owner/Follow/Share model
//! for multi-client collaboration.
//!
//! # Architecture
//!
//! ```text
//! Session (room with shared buffers)
//! └─ clients: HashMap<ClientId, Self>
//!     ├─ Owner { state: EditingState }   ← Owns editing state
//!     ├─ Follow { target: ClientId }     ← Read-only spectator
//!     └─ Share { owner: ClientId }       ← Bidirectional co-edit
//! ```
//!
//! # Behavior Matrix
//!
//! | Role   | My Input       | I See          | Use Case            |
//! |--------|----------------|----------------|---------------------|
//! | Owner  | → my state     | my state       | Solo editing        |
//! | Follow | ignored        | target's state | Spectator/present   |
//! | Share  | → owner state  | owner's state  | Pair programming    |
//!
//! # Design Philosophy
//!
//! - **Server owns buffers**: Shared content lives in session
//! - **Client owns state**: Each Owner has independent mode/cursor
//! - **Mechanism/Policy**: Server provides state, clients render
//!
//! # Example
//!
//! ```ignore
//! use reovim_server::session::{Client, ClientId, EditingState};
//!
//! // Create an owner client
//! let client1 = Client::new_owner();
//!
//! // Create a follower
//! let client2 = Client::follow(ClientId::new(1));
//!
//! // Create a share (pair programming)
//! let client3 = Client::share(ClientId::new(1));
//!
//! // Get effective state for rendering
//! let clients: HashMap<ClientId, Self> = ...;
//! let state = client2.effective_state(&clients);
//! // Returns client1's state (the target being followed)
//! ```

use std::collections::HashMap;

use {
    reovim_driver_session::{CursorPosition, KeySequence, Selection, SelectionMode, Viewport},
    reovim_kernel::api::v1::ModeStack,
};

use super::ClientId;

/// Per-client role within a session.
///
/// Determines how a client's input is handled and whose state they see.
#[derive(Debug, Clone)]
pub enum Client {
    /// Owns editing state. Input goes to own state.
    Owner {
        /// The editing state owned by this client.
        state: EditingState,
    },

    /// Read-only spectator. Input is ignored, sees target's state.
    Follow {
        /// Client ID being followed.
        target: ClientId,
    },

    /// Bidirectional co-editing with owner. Input goes to owner's state.
    Share {
        /// Owner client ID whose state is shared.
        owner: ClientId,
    },
}

impl Client {
    /// Create a new owner client with default editing state.
    #[must_use]
    pub fn new_owner() -> Self {
        Self::Owner {
            state: EditingState::default(),
        }
    }

    /// Create an owner client with specified mode stack.
    #[must_use]
    pub fn owner_with_mode(mode_stack: ModeStack) -> Self {
        Self::Owner {
            state: EditingState::with_mode_stack(mode_stack),
        }
    }

    /// Create a follow client (read-only spectator).
    #[must_use]
    pub const fn follow(target: ClientId) -> Self {
        Self::Follow { target }
    }

    /// Create a share client (bidirectional co-edit).
    #[must_use]
    pub const fn share(owner: ClientId) -> Self {
        Self::Share { owner }
    }

    /// Check if this client is an owner.
    #[must_use]
    pub const fn is_owner(&self) -> bool {
        matches!(self, Self::Owner { .. })
    }

    /// Check if this client is a follower.
    #[must_use]
    pub const fn is_follower(&self) -> bool {
        matches!(self, Self::Follow { .. })
    }

    /// Check if this client is sharing.
    #[must_use]
    pub const fn is_sharing(&self) -> bool {
        matches!(self, Self::Share { .. })
    }

    /// Get the effective state for this client.
    ///
    /// - Owner: Returns own state
    /// - Follow: Returns target's state (recursively)
    /// - Share: Returns owner's state (recursively)
    ///
    /// Returns `None` if the target/owner doesn't exist or if there's a cycle.
    #[must_use]
    pub fn effective_state<'a>(
        &'a self,
        clients: &'a HashMap<ClientId, Self>,
    ) -> Option<&'a EditingState> {
        match self {
            Self::Owner { state } => Some(state),
            Self::Follow { target } | Self::Share { owner: target } => {
                // Prevent infinite recursion by limiting depth
                Self::resolve_state(*target, clients, 10)
            }
        }
    }

    /// Get mutable reference to the effective state for input routing.
    ///
    /// - Owner: Returns own state
    /// - Follow: Returns `None` (input is ignored)
    /// - Share: Returns owner's state (recursively)
    pub fn effective_state_mut<'a>(
        &'a mut self,
        clients: &'a mut HashMap<ClientId, Self>,
        _self_id: ClientId,
    ) -> Option<&'a mut EditingState> {
        match self {
            Self::Owner { state } => Some(state),
            Self::Follow { .. } => None, // Input ignored for followers
            Self::Share { owner } => {
                // Route to owner's state
                let owner_id = *owner;
                // Can't borrow self and clients simultaneously, so we need to get owner directly
                clients
                    .get_mut(&owner_id)
                    .and_then(|c| c.effective_state_mut_inner())
            }
        }
    }

    /// Internal helper for getting mutable state (used for Share routing).
    #[allow(clippy::missing_const_for_fn)]
    fn effective_state_mut_inner(&mut self) -> Option<&mut EditingState> {
        match self {
            Self::Owner { state } => Some(state),
            Self::Follow { .. } | Self::Share { .. } => None,
        }
    }

    /// Get the target client ID for Follow/Share, or `None` for Owner.
    #[must_use]
    pub const fn target_id(&self) -> Option<ClientId> {
        match self {
            Self::Owner { .. } => None,
            Self::Follow { target } => Some(*target),
            Self::Share { owner } => Some(*owner),
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
        match client {
            Self::Owner { state } => Some(state),
            Self::Follow { target: next } | Self::Share { owner: next } => {
                Self::resolve_state(*next, clients, depth - 1)
            }
        }
    }
}

/// Editing state for an Owner client.
///
/// Contains all per-client state needed for editing operations.
/// This is the "source of truth" for Owner clients; Follow/Share
/// clients reference their target's state.
#[derive(Debug, Clone)]
pub struct EditingState {
    /// Mode stack (current mode on top).
    pub mode_stack: ModeStack,

    /// Keys accumulated but not yet processed.
    pub pending_keys: KeySequence,

    /// Cursor position in the active buffer.
    pub cursor: CursorPosition,

    /// Viewport (visible area).
    pub viewport: Viewport,

    /// Active selection (for visual mode).
    pub selection: Option<ClientSelection>,
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
            cursor: CursorPosition::origin(),
            viewport: Viewport::default(),
            selection: None,
        }
    }
}

impl EditingState {
    /// Create editing state with a specific mode stack.
    #[must_use]
    pub fn with_mode_stack(mode_stack: ModeStack) -> Self {
        Self {
            mode_stack,
            ..Default::default()
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

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_mode_stack() -> ModeStack {
        let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
        ModeStack::new(mode)
    }

    #[test]
    fn test_client_new_owner() {
        let client = Client::new_owner();
        assert!(client.is_owner());
        assert!(!client.is_follower());
        assert!(!client.is_sharing());
        assert!(client.target_id().is_none());
    }

    #[test]
    fn test_client_owner_with_mode() {
        let mode_stack = test_mode_stack();
        let client = Client::owner_with_mode(mode_stack.clone());

        if let Client::Owner { state } = client {
            assert_eq!(state.mode_stack.current(), mode_stack.current());
        } else {
            panic!("Expected Owner variant");
        }
    }

    #[test]
    fn test_client_follow() {
        let target = ClientId::new(42);
        let client = Client::follow(target);

        assert!(client.is_follower());
        assert!(!client.is_owner());
        assert!(!client.is_sharing());
        assert_eq!(client.target_id(), Some(target));
    }

    #[test]
    fn test_client_share() {
        let owner = ClientId::new(1);
        let client = Client::share(owner);

        assert!(client.is_sharing());
        assert!(!client.is_owner());
        assert!(!client.is_follower());
        assert_eq!(client.target_id(), Some(owner));
    }

    #[test]
    fn test_effective_state_owner() {
        let client = Client::new_owner();
        let clients = HashMap::new();

        let state = client.effective_state(&clients);
        assert!(state.is_some());
    }

    #[test]
    fn test_effective_state_follow() {
        let owner_id = ClientId::new(1);
        let follower_id = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(owner_id, Client::new_owner());
        clients.insert(follower_id, Client::follow(owner_id));

        let follower = clients.get(&follower_id).unwrap();
        let state = follower.effective_state(&clients);

        // Should return the owner's state
        assert!(state.is_some());
    }

    #[test]
    fn test_effective_state_share() {
        let owner_id = ClientId::new(1);
        let sharer_id = ClientId::new(2);

        let mut clients = HashMap::new();
        clients.insert(owner_id, Client::new_owner());
        clients.insert(sharer_id, Client::share(owner_id));

        let sharer = clients.get(&sharer_id).unwrap();
        let state = sharer.effective_state(&clients);

        // Should return the owner's state
        assert!(state.is_some());
    }

    #[test]
    fn test_effective_state_missing_target() {
        let follower = Client::follow(ClientId::new(999)); // Non-existent target
        let clients = HashMap::new();

        let state = follower.effective_state(&clients);
        assert!(state.is_none());
    }

    #[test]
    fn test_effective_state_chain() {
        // Test: Client 3 follows Client 2 who follows Client 1 (owner)
        let owner_id = ClientId::new(1);
        let mid_id = ClientId::new(2);
        let end_id = ClientId::new(3);

        let mut clients = HashMap::new();
        clients.insert(owner_id, Client::new_owner());
        clients.insert(mid_id, Client::follow(owner_id));
        clients.insert(end_id, Client::follow(mid_id));

        let end_client = clients.get(&end_id).unwrap();
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
        clients.insert(id1, Client::follow(id3));
        clients.insert(id2, Client::follow(id1));
        clients.insert(id3, Client::follow(id2));

        let client = clients.get(&id1).unwrap();
        let state = client.effective_state(&clients);

        // Should return None due to cycle (depth limit exceeded)
        assert!(state.is_none());
    }

    #[test]
    fn test_editing_state_default() {
        let state = EditingState::default();

        assert!(state.pending_keys.is_empty());
        assert_eq!(state.cursor, CursorPosition::origin());
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
}
