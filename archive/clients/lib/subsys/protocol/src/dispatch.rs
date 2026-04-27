//! Abstract notification dispatch contracts.

/// Outcome of a notification dispatch attempt.
///
/// Defined ahead of any consuming method so the consumer-facing API
/// shape is locked before the first dispatcher implementation lands.
/// `#[non_exhaustive]` lets new outcomes be added without breaking
/// downstream `match` arms.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// The dispatcher recognised and handled the notification.
    Handled,
    /// The dispatcher skipped the notification intentionally.
    Ignored,
    /// The notification type was not recognised by any registered handler.
    Unknown,
}

/// Abstract notification dispatch contract.
///
/// `Debug + Send + Sync` bounds let implementors be stored in
/// `Arc<dyn NotificationDispatcher>` across threads without unsafe.
pub trait NotificationDispatcher: std::fmt::Debug + Send + Sync {}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
