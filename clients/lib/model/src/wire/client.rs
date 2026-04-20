//! Unified client types for wire format (#480 Client Architecture Unification).
//!
//! These types mirror the server's `Client` model but contain only
//! the minimal state needed for rendering peer presence.
//!
//! # Server Model Mapping
//!
//! | Wire Type | Server Type | Purpose |
//! |-----------|-------------|---------|
//! | `ClientRelation` | `session::ClientRelation` | Input routing |
//! | `ClientViewState` | `session::EditingState` (projection) | Render state |
//! | `ClientMetadata` | `session::ClientMetadata` | Identity |
//! | `ClientInfo` | `session::Client` | Full client |

use serde::{Deserialize, Serialize};

/// Relation to another client.
///
/// Used as `Option<ClientRelation>` where `None` = independent.
///
/// # Behavior Matrix
///
/// | Relation | My Input | I See | Use Case |
/// |----------|----------|-------|----------|
/// | None | → my state | my state | Solo editing |
/// | Following | ignored | target's state | Spectator |
/// | Sharing | → target's state | target's state | Pair programming |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ClientRelation {
    /// Read-only spectator. Input ignored, sees target's state.
    Following {
        /// ID of the client being followed.
        target: u64,
    },
    /// Bidirectional collaboration. Input goes to target's state.
    Sharing {
        /// ID of the client to share input with.
        with: u64,
    },
}

impl ClientRelation {
    /// Create a Following relation.
    #[must_use]
    pub const fn following(target: u64) -> Self {
        Self::Following { target }
    }

    /// Create a Sharing relation.
    #[must_use]
    pub const fn sharing(with: u64) -> Self {
        Self::Sharing { with }
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

    /// Get target client ID.
    #[must_use]
    pub const fn target_id(&self) -> u64 {
        match *self {
            Self::Following { target } => target,
            Self::Sharing { with } => with,
        }
    }
}

/// Minimal view state for rendering peer presence.
///
/// This is a projection of the server's `EditingState`. Clients receive
/// this via notifications to render peer cursors, selections, etc.
///
/// # Server Model Mapping
///
/// | Field | Source (EditingState) |
/// |-------|----------------------|
/// | `mode` | `mode_stack.current().name()` |
/// | `cursor` | `windows.active().cursor` |
/// | `buffer_id` | `windows.active().buffer_id` |
/// | `selection` | `selection.to_driver_selection()` |
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ClientViewState {
    /// Current mode name ("NORMAL", "INSERT", etc.).
    pub mode: String,
    /// Cursor position (line, column).
    pub cursor: (u32, u32),
    /// Which buffer this client is viewing.
    pub buffer_id: Option<u64>,
    /// Selection range for visual mode (start, end as (line, col)).
    pub selection: Option<((u32, u32), (u32, u32))>,
}

impl ClientViewState {
    /// Create a view state with just mode.
    #[must_use]
    pub fn new(mode: impl Into<String>) -> Self {
        Self {
            mode: mode.into(),
            cursor: (0, 0),
            buffer_id: None,
            selection: None,
        }
    }

    /// Set cursor position.
    #[must_use]
    pub const fn with_cursor(mut self, line: u32, column: u32) -> Self {
        self.cursor = (line, column);
        self
    }

    /// Set buffer ID.
    #[must_use]
    pub const fn with_buffer(mut self, buffer_id: u64) -> Self {
        self.buffer_id = Some(buffer_id);
        self
    }

    /// Set selection.
    #[must_use]
    pub const fn with_selection(mut self, start: (u32, u32), end: (u32, u32)) -> Self {
        self.selection = Some((start, end));
        self
    }
}

/// Client metadata.
///
/// Contains identity information for display and debugging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ClientMetadata {
    /// Client type ("tui", "web", "android", "cli").
    pub client_type: String,
    /// Display name ("laptop", "phone").
    pub display_name: String,
    /// When client joined (Unix ms).
    pub joined_at_ms: u64,
}

impl ClientMetadata {
    /// Create new metadata.
    #[must_use]
    pub fn new(client_type: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            client_type: client_type.into(),
            display_name: display_name.into(),
            joined_at_ms: 0,
        }
    }

    /// Set join timestamp.
    #[must_use]
    pub const fn with_joined_at(mut self, joined_at_ms: u64) -> Self {
        self.joined_at_ms = joined_at_ms;
        self
    }
}

impl Default for ClientMetadata {
    fn default() -> Self {
        Self::new("unknown", "unknown")
    }
}

/// A client in a session.
///
/// Unified type that combines relation, view state, and metadata.
/// This is the wire format for `peers_v2` and `clients_v2` in the protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ClientInfo {
    /// Unique client ID.
    pub id: u64,
    /// Relation to another client. `None` = independent.
    pub relation: Option<ClientRelation>,
    /// View state for rendering.
    pub view: ClientViewState,
    /// Client metadata.
    pub metadata: ClientMetadata,
}

impl ClientInfo {
    /// Create a new independent client.
    #[must_use]
    pub fn new(id: u64, metadata: ClientMetadata) -> Self {
        Self {
            id,
            relation: None,
            view: ClientViewState::default(),
            metadata,
        }
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

    /// Get the target client ID, if any.
    #[must_use]
    pub const fn target_id(&self) -> Option<u64> {
        match self.relation {
            Some(ClientRelation::Following { target }) => Some(target),
            Some(ClientRelation::Sharing { with }) => Some(with),
            None => None,
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
