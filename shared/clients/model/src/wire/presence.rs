//! Client presence and synchronization modes.
//!
//! These types support multi-client scenarios where multiple clients
//! are attached to the same session.

use serde::{Deserialize, Serialize};

/// Synchronization mode for multi-client scenarios.
///
/// Determines how a client's state relates to other clients
/// in the same session.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SyncMode {
    /// Client operates independently.
    ///
    /// Each client has its own viewport, cursor, and selection.
    /// Changes to buffer content are shared, but view state is not.
    #[default]
    Independent,

    /// Client broadcasts its state to followers.
    ///
    /// Other clients in `Follow` mode will mirror this client's
    /// viewport and cursor position.
    Broadcast,

    /// Client follows a broadcaster.
    ///
    /// The client's viewport and cursor track the broadcaster's position.
    /// Local edits are still possible but cursor will snap back.
    Follow {
        /// ID of the client being followed.
        target: String,
    },

    /// Client accepts incoming state from any broadcaster.
    ///
    /// Similar to `Follow` but accepts state from any broadcasting client
    /// rather than a specific one.
    Accept,
}

impl SyncMode {
    /// Create a Follow mode targeting a specific client.
    #[must_use]
    pub fn follow(target: impl Into<String>) -> Self {
        Self::Follow {
            target: target.into(),
        }
    }

    /// Check if this mode broadcasts state.
    #[must_use]
    pub const fn is_broadcasting(&self) -> bool {
        matches!(self, Self::Broadcast)
    }

    /// Check if this mode receives state from others.
    #[must_use]
    pub const fn is_receiving(&self) -> bool {
        matches!(self, Self::Follow { .. } | Self::Accept)
    }

    /// Check if this mode is independent.
    #[must_use]
    pub const fn is_independent(&self) -> bool {
        matches!(self, Self::Independent)
    }
}

/// Presence information for a connected client.
///
/// Used for collaborative features and multi-client awareness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ClientPresence {
    /// Client identifier.
    pub client_id: String,
    /// Human-readable name (optional).
    pub name: Option<String>,
    /// Current synchronization mode.
    pub sync_mode: SyncMode,
    /// Currently focused buffer (if any).
    pub active_buffer: Option<u64>,
    /// Cursor position in active buffer.
    pub cursor: Option<(u32, u32)>,
    /// Whether the client is actively editing.
    pub is_active: bool,
}

impl ClientPresence {
    /// Create a new client presence.
    #[must_use]
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            name: None,
            sync_mode: SyncMode::Independent,
            active_buffer: None,
            cursor: None,
            is_active: true,
        }
    }

    /// Set the display name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the sync mode.
    #[must_use]
    pub fn with_sync_mode(mut self, mode: SyncMode) -> Self {
        self.sync_mode = mode;
        self
    }

    /// Set the active buffer and cursor.
    #[must_use]
    pub const fn with_position(mut self, buffer_id: u64, line: u32, col: u32) -> Self {
        self.active_buffer = Some(buffer_id);
        self.cursor = Some((line, col));
        self
    }

    /// Get the display name, falling back to client ID.
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.client_id)
    }
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod tests;
