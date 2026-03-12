//! Thread-safe pending notification queue.
//!
//! Background threads (LSP completion, snippet reload) push notifications here.
//! A synchronous command handler drains the queue into `NotificationState` on
//! each invocation.
//!
//! This avoids the problem that background threads lack `SessionRuntime`
//! access needed for `NotificationState`.
//!
//! # Architecture
//!
//! This lives in the driver layer (mechanism) because multiple modules
//! need to push notifications without depending on each other:
//! - `completion` module: LSP completion results
//! - `snippet` module: reload/catalog notifications
//! - `notification` module: drains into display state

use {parking_lot::Mutex, reovim_kernel::api::v1::Service};

/// Notification level for pending notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingLevel {
    /// Informational message.
    Info,
    /// Success confirmation.
    Success,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
}

/// A pending notification to be flushed to display state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingNotification {
    /// Severity level.
    pub level: PendingLevel,
    /// Short title.
    pub title: String,
}

/// Thread-safe queue for notifications from background threads.
///
/// Registered as a [`Service`] in `ServiceRegistry` so that both the
/// background threads (which have `Arc<ServiceRegistry>`) and the
/// command handler (which has `SessionRuntime`) can access it.
#[derive(Debug)]
pub struct PendingNotificationQueue {
    queue: Mutex<Vec<PendingNotification>>,
}

impl PendingNotificationQueue {
    /// Create a new empty queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }

    /// Push a notification from any thread.
    pub fn push(&self, level: PendingLevel, title: impl Into<String>) {
        self.queue.lock().push(PendingNotification {
            level,
            title: title.into(),
        });
    }

    /// Drain all pending notifications.
    ///
    /// Called from the command handler context where `SessionRuntime`
    /// is available to forward them to the display system.
    pub fn drain(&self) -> Vec<PendingNotification> {
        let mut queue = self.queue.lock();
        std::mem::take(&mut *queue)
    }

    /// Number of pending notifications.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    /// Whether the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }
}

impl Default for PendingNotificationQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for PendingNotificationQueue {}
#[cfg(test)]
#[path = "notification_queue_tests.rs"]
mod tests;
