//! Thread-safe pending notification queue.
//!
//! Background threads (LSP completion, auto-start) push notifications here.
//! The synchronous `Trigger::execute()` drains the queue into
//! `NotificationState` on each invocation.
//!
//! This avoids the problem that background threads lack `SessionRuntime`
//! access needed for `NotificationState`.

use {parking_lot::Mutex, reovim_kernel::api::v1::Service};

/// Notification level matching [`reovim_module_notification::NotificationLevel`].
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

/// A pending notification to be flushed to `NotificationState`.
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
    /// is available to forward them to `NotificationState`.
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
mod tests {
    use super::*;

    #[test]
    fn new_queue_is_empty() {
        let queue = PendingNotificationQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn default_queue_is_empty() {
        let queue = PendingNotificationQueue::default();
        assert!(queue.is_empty());
    }

    #[test]
    fn push_and_drain() {
        let queue = PendingNotificationQueue::new();
        queue.push(PendingLevel::Info, "hello");
        queue.push(PendingLevel::Success, "world");

        assert_eq!(queue.len(), 2);
        assert!(!queue.is_empty());

        let items = queue.drain();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].level, PendingLevel::Info);
        assert_eq!(items[0].title, "hello");
        assert_eq!(items[1].level, PendingLevel::Success);
        assert_eq!(items[1].title, "world");

        // Queue is empty after drain.
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn drain_empty_returns_empty() {
        let queue = PendingNotificationQueue::new();
        let items = queue.drain();
        assert!(items.is_empty());
    }

    #[test]
    fn drain_clears_queue() {
        let queue = PendingNotificationQueue::new();
        queue.push(PendingLevel::Warning, "first");
        let _ = queue.drain();
        queue.push(PendingLevel::Error, "second");
        let items = queue.drain();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "second");
    }

    #[test]
    fn concurrent_push() {
        use std::sync::Arc;

        let queue = Arc::new(PendingNotificationQueue::new());
        let mut handles = Vec::new();

        for i in 0..10 {
            let q = Arc::clone(&queue);
            handles.push(std::thread::spawn(move || {
                q.push(PendingLevel::Info, format!("msg-{i}"));
            }));
        }

        for h in handles {
            h.join().expect("Thread panicked");
        }

        assert_eq!(queue.len(), 10);
        let items = queue.drain();
        assert_eq!(items.len(), 10);
        assert!(queue.is_empty());
    }

    #[test]
    fn all_levels() {
        let queue = PendingNotificationQueue::new();
        queue.push(PendingLevel::Info, "i");
        queue.push(PendingLevel::Success, "s");
        queue.push(PendingLevel::Warning, "w");
        queue.push(PendingLevel::Error, "e");

        let items = queue.drain();
        assert_eq!(items[0].level, PendingLevel::Info);
        assert_eq!(items[1].level, PendingLevel::Success);
        assert_eq!(items[2].level, PendingLevel::Warning);
        assert_eq!(items[3].level, PendingLevel::Error);
    }

    #[test]
    fn debug_impls() {
        let queue = PendingNotificationQueue::new();
        let debug = format!("{queue:?}");
        assert!(debug.contains("PendingNotificationQueue"));

        let level = PendingLevel::Warning;
        let debug = format!("{level:?}");
        assert_eq!(debug, "Warning");

        let notif = PendingNotification {
            level: PendingLevel::Error,
            title: "test".to_owned(),
        };
        let debug = format!("{notif:?}");
        assert!(debug.contains("PendingNotification"));
    }

    #[test]
    fn level_clone_copy_eq() {
        let level = PendingLevel::Success;
        let copied = level;
        #[allow(clippy::clone_on_copy)]
        let cloned = level.clone();
        assert_eq!(level, copied);
        assert_eq!(level, cloned);
        assert_ne!(PendingLevel::Info, PendingLevel::Error);
    }

    #[test]
    fn notification_clone_eq() {
        let notif = PendingNotification {
            level: PendingLevel::Info,
            title: "test".to_owned(),
        };
        let cloned = notif.clone();
        assert_eq!(notif, cloned);
    }
}
