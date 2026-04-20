//! Layout synchronization mode.
//!
//! Controls how window layout is synchronized between clients.

use serde::{Deserialize, Serialize};

/// Synchronization mode for window layout.
///
/// Determines whether a client's window arrangement is independent
/// or synchronized with other clients.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LayoutSyncMode {
    /// Client has independent window layout.
    ///
    /// Window splits and arrangement are local to this client.
    #[default]
    Independent,

    /// Client broadcasts its layout to followers.
    ///
    /// Other clients in `Follow` mode will mirror this layout.
    Broadcast,

    /// Client follows a specific broadcaster's layout.
    Follow {
        /// Client ID of the broadcaster.
        target: String,
    },

    /// Client accepts layout from any broadcaster.
    Accept,
}

impl LayoutSyncMode {
    /// Create a follow mode targeting a specific client.
    #[must_use]
    pub fn follow(target: impl Into<String>) -> Self {
        Self::Follow {
            target: target.into(),
        }
    }

    /// Check if this mode broadcasts layout.
    #[must_use]
    pub const fn is_broadcasting(&self) -> bool {
        matches!(self, Self::Broadcast)
    }

    /// Check if this mode receives layout from others.
    #[must_use]
    pub const fn is_receiving(&self) -> bool {
        matches!(self, Self::Follow { .. } | Self::Accept)
    }

    /// Check if layout is independent.
    #[must_use]
    pub const fn is_independent(&self) -> bool {
        matches!(self, Self::Independent)
    }

    /// Get the target client ID if in Follow mode.
    #[must_use]
    pub fn follow_target(&self) -> Option<&str> {
        match self {
            Self::Follow { target } => Some(target),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
