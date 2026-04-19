//! Core session types extracted from the session driver.
//!
//! This module provides:
//! - [`ClientId`] — unique client connection identifier
//! - [`CursorSnapshot`] — per-client opaque cursor identity snapshot for bridge tick consumption
//! - [`KeySequence`] — pending key sequence accumulator

use crate::SessionExtension;

pub use reovim_subsys_input_contracts::KeySequence;

/// Unique client connection identifier.
///
/// Each terminal/TUI that connects to the server gets a unique `ClientId`.
/// IDs are monotonically increasing and not reused after disconnect.
///
/// # Semantics
///
/// - **Client**: Individual connection to the server (like tmux clients)
/// - **Session**: Named editing context (defined in runner layer)
/// - Multiple clients can attach to the same session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub usize);

impl ClientId {
    /// Create a new client ID.
    #[must_use]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client-{}", self.0)
    }
}

/// Per-client cursor identity snapshot for bridge consumption.
///
/// Updated by the active domain driver after input dispatch. Bridges read this
/// opaque token to detect cursor identity changes without direct access to
/// domain-specific cursor state.
///
/// This is a domain-neutral mechanism-level type: any bridge can read it, but
/// only the active driver owns how the 8-byte body is produced. A sentinel
/// value of all-zeros indicates "no cursor seen yet".
///
/// No text-domain fields (line, col, buffer_id) are exposed here.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CursorSnapshot(pub [u8; 8]);

impl CursorSnapshot {
    /// The sentinel value meaning "no cursor position recorded yet".
    pub const SENTINEL: Self = Self([0u8; 8]);

    /// Create a snapshot from raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// Return the raw 8-byte representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
}

impl std::fmt::Debug for CursorSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CursorSnapshot").field(&self.0).finish()
    }
}

impl SessionExtension for CursorSnapshot {
    fn create() -> Self {
        Self::SENTINEL
    }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
