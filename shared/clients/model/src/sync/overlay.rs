//! Overlay synchronization mode.
//!
//! Controls whether overlays are local or shared between clients.

use serde::{Deserialize, Serialize};

/// Synchronization mode for overlays.
///
/// Determines whether an overlay is visible only to the local client
/// or shared with all clients in the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum OverlaySyncMode {
    /// Overlay is local to this client.
    ///
    /// Only this client sees the overlay (e.g., local completion).
    #[default]
    Local,

    /// Overlay is shared with all clients.
    ///
    /// All clients in the session see the overlay (e.g., collaborative
    /// command palette, shared popup).
    Shared,
}

impl OverlaySyncMode {
    /// Check if this overlay is local only.
    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    /// Check if this overlay is shared.
    #[must_use]
    pub const fn is_shared(&self) -> bool {
        matches!(self, Self::Shared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_sync_mode_default() {
        let mode = OverlaySyncMode::default();
        assert!(mode.is_local());
        assert!(!mode.is_shared());
    }

    #[test]
    fn test_overlay_sync_mode_local() {
        let mode = OverlaySyncMode::Local;
        assert!(mode.is_local());
        assert!(!mode.is_shared());
    }

    #[test]
    fn test_overlay_sync_mode_shared() {
        let mode = OverlaySyncMode::Shared;
        assert!(mode.is_shared());
        assert!(!mode.is_local());
    }
}
