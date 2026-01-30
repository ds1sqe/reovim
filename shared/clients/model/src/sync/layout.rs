//! Layout synchronization mode.
//!
//! Controls how window layout is synchronized between clients.

/// Synchronization mode for window layout.
///
/// Determines whether a client's window arrangement is independent
/// or synchronized with other clients.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
mod tests {
    use super::*;

    #[test]
    fn test_layout_sync_mode_default() {
        let mode = LayoutSyncMode::default();
        assert!(mode.is_independent());
        assert!(!mode.is_broadcasting());
        assert!(!mode.is_receiving());
    }

    #[test]
    fn test_layout_sync_mode_broadcast() {
        let mode = LayoutSyncMode::Broadcast;
        assert!(mode.is_broadcasting());
        assert!(!mode.is_receiving());
        assert!(!mode.is_independent());
    }

    #[test]
    fn test_layout_sync_mode_follow() {
        let mode = LayoutSyncMode::follow("client-1");
        assert!(mode.is_receiving());
        assert!(!mode.is_broadcasting());
        assert_eq!(mode.follow_target(), Some("client-1"));
    }

    #[test]
    fn test_layout_sync_mode_accept() {
        let mode = LayoutSyncMode::Accept;
        assert!(mode.is_receiving());
        assert!(!mode.is_broadcasting());
        assert_eq!(mode.follow_target(), None);
    }
}
