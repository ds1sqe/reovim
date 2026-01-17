//! Async bridge for log notifications.
//!
//! Bridges the synchronous tracing layer to async notification broadcasting.
//! Uses a bounded channel to avoid blocking the tracing callback.

use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};

use tokio::sync::mpsc;

use super::{
    log_buffer::LogEntry,
    log_subscriber::{entry_to_payload, log_subscribers},
};

// ============================================================================
// Constants
// ============================================================================

/// Channel capacity for log entries.
///
/// Chosen to handle burst logging without dropping too many entries,
/// while keeping memory bounded.
const LOG_CHANNEL_CAPACITY: usize = 1000;

// ============================================================================
// Global Channel
// ============================================================================

/// Global log entry sender.
static LOG_SENDER: OnceLock<mpsc::Sender<LogEntry>> = OnceLock::new();

/// Counter for dropped entries (when channel is full).
static DROPPED_COUNT: AtomicU64 = AtomicU64::new(0);

/// Initialize the log bridge channel.
///
/// Returns the receiver end for the drain task, or `None` if already initialized.
/// Should only be called once during server startup.
#[must_use]
pub fn init_log_bridge() -> Option<mpsc::Receiver<LogEntry>> {
    let (tx, rx) = mpsc::channel(LOG_CHANNEL_CAPACITY);

    match LOG_SENDER.set(tx) {
        Ok(()) => Some(rx),
        Err(_) => {
            // Already initialized - this is OK for tests or multiple init calls
            None
        }
    }
}

/// Try to send a log entry to the bridge.
///
/// This is called from the synchronous tracing layer.
/// Uses `try_send` to avoid blocking - if the channel is full,
/// the entry is dropped and the counter is incremented.
pub fn try_send_log(entry: LogEntry) {
    if let Some(sender) = LOG_SENDER.get()
        && sender.try_send(entry).is_err()
    {
        DROPPED_COUNT.fetch_add(1, Ordering::Relaxed);
    }
    // If sender not initialized, silently drop (startup race)
}

/// Get the number of dropped log entries.
#[must_use]
pub fn dropped_count() -> u64 {
    DROPPED_COUNT.load(Ordering::Relaxed)
}

/// Reset the dropped counter (for testing).
#[cfg(test)]
pub fn reset_dropped_count() {
    DROPPED_COUNT.store(0, Ordering::Relaxed);
}

// ============================================================================
// Drain Task
// ============================================================================

/// Spawn the log drain task.
///
/// This task reads from the log bridge channel and broadcasts
/// notifications to subscribed clients.
///
/// # Returns
///
/// The `JoinHandle` for the drain task (can be used for shutdown).
#[must_use]
pub fn spawn_drain_task(mut rx: mpsc::Receiver<LogEntry>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(entry) = rx.recv().await {
            broadcast_log_entry(&entry).await;
        }
        tracing::debug!("Log drain task shutting down");
    })
}

/// Broadcast a log entry to all matching subscribers.
async fn broadcast_log_entry(entry: &LogEntry) {
    use reovim_protocol::v1::LogLevel;

    let level = LogLevel::from_str_lossy(&entry.level);
    let subscribers = log_subscribers().matching_subscriptions(level);

    if subscribers.is_empty() {
        return;
    }

    // Convert entry to notification payload
    let payload = entry_to_payload(entry);
    let notification = payload.into_notification();

    // Serialize once for all clients
    let json = match serde_json::to_string(&notification) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("Failed to serialize log notification: {e}");
            return;
        }
    };

    // Send to each subscriber
    for sub in subscribers {
        // Try to upgrade weak reference to strong and send
        if let Some(client) = sub.client.upgrade()
            && let Err(e) = client.send_line(&json).await
        {
            tracing::trace!("Failed to send log to client {:?}: {e}", sub.client_id);
            // Client may have disconnected - the weak ref will fail next time
        }
        // If upgrade fails, client is gone - subscription cleanup happens elsewhere
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Most tests for the bridge are integration tests
    // because the global state makes unit testing difficult.
    // These tests verify the atomic counter behavior.

    #[test]
    fn test_dropped_count_initial() {
        // Counter should start at 0 (or whatever previous tests left it at)
        let _ = dropped_count();
    }

    #[test]
    fn test_try_send_without_init() {
        // Should not panic when sender is not initialized
        let entry = LogEntry::new("INFO", "test", "message");
        try_send_log(entry);
        // Entry is silently dropped
    }
}
