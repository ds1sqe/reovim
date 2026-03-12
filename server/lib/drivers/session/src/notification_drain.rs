//! Notification drain trait for cross-module decoupling.
//!
//! Defines the [`NotificationDrain`] trait so modules can drain
//! [`PendingNotificationQueue`] into session-level notification state
//! without depending on the concrete notification module.
//!
//! # Architecture (#542)
//!
//! This is mechanism: it defines WHAT operation is available (draining
//! pending notifications into session state). The `reovim-module-notification`
//! provides the policy: HOW notifications are stored and displayed.

use std::sync::Arc;

use reovim_kernel::api::v1::Service;

use crate::SessionRuntime;

/// Trait for draining pending notifications into session state.
///
/// Implemented by `reovim-module-notification`. Consumers invoke this
/// to forward background-thread notifications (from [`PendingNotificationQueue`])
/// into the per-client `NotificationState` without importing the module.
pub trait NotificationDrain: Send + Sync {
    /// Drain pending notifications and push them into session state.
    ///
    /// Implementations should:
    /// 1. Read from [`PendingNotificationQueue`] in `ServiceRegistry`
    /// 2. Convert [`PendingLevel`] to module-specific notification levels
    /// 3. Push into the appropriate `SessionExtension`
    /// 4. Record extension changes via `runtime.take_changes()`
    fn drain_pending(&self, runtime: &mut SessionRuntime<'_>);
}

/// Registry for notification drain implementations.
///
/// Allows modules to register a [`NotificationDrain`] implementation that
/// other modules can discover via `ServiceRegistry`.
pub struct NotificationDrainRegistry {
    drain: parking_lot::RwLock<Option<Arc<dyn NotificationDrain>>>,
}

impl std::fmt::Debug for NotificationDrainRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationDrainRegistry")
            .field("has_drain", &self.drain.read().is_some())
            .finish()
    }
}

impl NotificationDrainRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            drain: parking_lot::RwLock::new(None),
        }
    }

    /// Register a notification drain implementation.
    pub fn register(&self, drain: Arc<dyn NotificationDrain>) {
        *self.drain.write() = Some(drain);
    }

    /// Get the registered notification drain implementation.
    #[must_use]
    pub fn get(&self) -> Option<Arc<dyn NotificationDrain>> {
        self.drain.read().clone()
    }
}

impl Default for NotificationDrainRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for NotificationDrainRegistry {}
#[cfg(test)]
#[path = "notification_drain_tests.rs"]
mod tests;
