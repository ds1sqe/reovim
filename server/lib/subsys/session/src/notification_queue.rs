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
//! - `lsp` module: `$/progress`, `window/showMessage`
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

/// Operation type for the pending notification queue.
///
/// Extends the original flat push with progress lifecycle operations
/// for `$/progress` and similar producer protocols (#691).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingOp {
    /// Simple notification (existing behavior).
    Push {
        /// Severity level.
        level: PendingLevel,
        /// Short title.
        title: String,
    },
    /// Begin a progress notification.
    ProgressBegin {
        /// Opaque token from the producer (e.g., LSP progress token).
        token: String,
        /// Short title (e.g., "Indexing").
        title: String,
        /// Optional detail message.
        message: String,
        /// Initial percentage (0..=100), or 0 if indeterminate.
        percentage: u8,
    },
    /// Update a progress notification.
    ProgressReport {
        /// Token matching a previous `ProgressBegin`.
        token: String,
        /// Updated detail message (if any).
        message: Option<String>,
        /// Updated percentage (if any).
        percentage: Option<u8>,
    },
    /// End a progress notification.
    ProgressEnd {
        /// Token matching a previous `ProgressBegin`.
        token: String,
        /// Optional final message.
        message: Option<String>,
    },
}

/// A pending notification entry with optional source attribution.
///
/// Wraps a [`PendingOp`] with an optional source tag for producer grouping.
/// The source identifies the notification producer (e.g., `"rust-analyzer"`,
/// `"git"`) so the TUI can group notifications into bordered boxes (#691).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEntry {
    /// Optional producer name for grouped display.
    pub source: Option<String>,
    /// The notification operation.
    pub op: PendingOp,
}

/// A pending notification to be flushed to display state.
///
/// Retained for backward compatibility. New code should use
/// [`PendingEntry`] + [`PendingOp`] via [`PendingNotificationQueue::push_op`].
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
    queue: Mutex<Vec<PendingEntry>>,
}

impl PendingNotificationQueue {
    /// Create a new empty queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }

    /// Push a simple notification from any thread.
    ///
    /// Convenience method for sourceless push operations.
    /// For source-tagged or progress notifications, use [`push_op`](Self::push_op).
    pub fn push(&self, level: PendingLevel, title: impl Into<String>) {
        self.queue.lock().push(PendingEntry {
            source: None,
            op: PendingOp::Push {
                level,
                title: title.into(),
            },
        });
    }

    /// Push a notification operation with optional source attribution.
    pub fn push_op(&self, source: Option<String>, op: PendingOp) {
        self.queue.lock().push(PendingEntry { source, op });
    }

    /// Drain all pending entries.
    ///
    /// Called from the command handler context where `SessionRuntime`
    /// is available to forward them to the display system.
    pub fn drain(&self) -> Vec<PendingEntry> {
        let mut queue = self.queue.lock();
        std::mem::take(&mut *queue)
    }

    /// Number of pending entries.
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
