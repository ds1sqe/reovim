//! Log subscription system for real-time log streaming.
//!
//! Provides infrastructure for clients to subscribe to log events
//! and receive them as notifications.

use std::{
    collections::HashMap,
    sync::{
        Arc, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

use {
    reovim_arch::sync::ArcSwap,
    reovim_protocol::v1::{LogEntryPayload, LogLevel, LogSource},
};

use crate::{server::client::Client, session::ClientId};

use super::log_buffer::LogEntry;

// ============================================================================
// Types
// ============================================================================

/// Unique subscription identifier.
pub type SubscriptionId = u64;

/// A single log subscription.
#[derive(Clone)]
pub struct LogSubscription {
    /// The subscription ID.
    pub id: SubscriptionId,
    /// The client that created this subscription.
    pub client_id: ClientId,
    /// Weak reference to the client for sending notifications.
    /// When the client disconnects, this becomes invalid.
    pub client: Weak<Client>,
    /// Minimum log level to receive (None = all levels).
    pub level_filter: Option<LogLevel>,
}

impl std::fmt::Debug for LogSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogSubscription")
            .field("id", &self.id)
            .field("client_id", &self.client_id)
            .field("level_filter", &self.level_filter)
            .field("client_alive", &self.client.upgrade().is_some())
            .finish()
    }
}

/// Thread-safe registry of log subscriptions.
#[derive(Debug)]
pub struct LogSubscribers {
    /// Subscriptions indexed by ID.
    subscriptions: ArcSwap<HashMap<SubscriptionId, LogSubscription>>,
    /// Next subscription ID.
    next_id: AtomicU64,
}

impl LogSubscribers {
    /// Create a new empty subscriber registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscriptions: ArcSwap::from_pointee(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// Subscribe to log events.
    ///
    /// Returns a unique subscription ID for unsubscribing.
    ///
    /// # Arguments
    ///
    /// * `client` - Arc to the client (stored as Weak to allow cleanup)
    /// * `level_filter` - Minimum log level to receive (None = all levels)
    pub fn subscribe(
        &self,
        client: &Arc<Client>,
        level_filter: Option<LogLevel>,
    ) -> SubscriptionId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let subscription = LogSubscription {
            id,
            client_id: client.id(),
            client: Arc::downgrade(client),
            level_filter,
        };

        self.subscriptions.rcu(|current| {
            let mut new = (**current).clone();
            new.insert(id, subscription.clone());
            new
        });

        id
    }

    /// Unsubscribe from log events.
    ///
    /// Returns true if the subscription was found and removed.
    pub fn unsubscribe(&self, subscription_id: SubscriptionId) -> bool {
        let mut found = false;
        self.subscriptions.rcu(|current| {
            let mut new = (**current).clone();
            found = new.remove(&subscription_id).is_some();
            new
        });
        found
    }

    /// Remove all subscriptions for a client.
    ///
    /// Called when a client disconnects.
    pub fn remove_client(&self, client_id: ClientId) {
        self.subscriptions.rcu(|current| {
            let new: HashMap<_, _> = current
                .iter()
                .filter(|(_, sub)| sub.client_id != client_id)
                .map(|(k, v)| (*k, v.clone()))
                .collect();
            new
        });
    }

    /// Get all subscriptions that should receive a log entry.
    ///
    /// Filters by level if the subscription has a level filter.
    pub fn matching_subscriptions(&self, level: LogLevel) -> Vec<LogSubscription> {
        self.subscriptions
            .load()
            .values()
            .filter(|sub| sub.level_filter.is_none_or(|min_level| level >= min_level))
            .cloned()
            .collect()
    }

    /// Get all current subscriptions (for testing/debugging).
    pub fn all_subscriptions(&self) -> Vec<LogSubscription> {
        self.subscriptions.load().values().cloned().collect()
    }

    /// Check if subscriber list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.subscriptions.load().is_empty()
    }

    /// Get subscription count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.subscriptions.load().len()
    }
}

impl Default for LogSubscribers {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Subscriber Registry
// ============================================================================

/// Global log subscribers registry.
static LOG_SUBSCRIBERS: OnceLock<LogSubscribers> = OnceLock::new();

/// Get or initialize the global log subscribers registry.
pub fn log_subscribers() -> &'static LogSubscribers {
    LOG_SUBSCRIBERS.get_or_init(LogSubscribers::new)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert a `LogEntry` to a `LogEntryPayload` notification.
#[must_use]
pub fn entry_to_payload(entry: &LogEntry) -> LogEntryPayload {
    LogEntryPayload {
        timestamp: entry.timestamp_iso(),
        level: LogLevel::from_str_lossy(&entry.level),
        target: entry.target.clone(),
        message: entry.message.clone(),
        source: LogSource::Server,
    }
}

#[cfg(test)]
mod tests {
    use crate::{server::transport::TransportWriter, session::SessionId};

    use super::*;

    /// Create a test client with the given ID.
    fn test_client(id: u64) -> Arc<Client> {
        Client::new(ClientId::new(id), SessionId::new("test"), TransportWriter::from_stdio())
    }

    #[test]
    fn test_subscribe_returns_unique_id() {
        let subscribers = LogSubscribers::new();
        let client = test_client(1);

        let id1 = subscribers.subscribe(&client, None);
        let id2 = subscribers.subscribe(&client, None);

        assert_ne!(id1, id2);
        assert_eq!(subscribers.len(), 2);
    }

    #[test]
    fn test_unsubscribe_valid_id() {
        let subscribers = LogSubscribers::new();
        let client = test_client(1);

        let id = subscribers.subscribe(&client, None);
        assert_eq!(subscribers.len(), 1);

        let result = subscribers.unsubscribe(id);
        assert!(result);
        assert_eq!(subscribers.len(), 0);
    }

    #[test]
    fn test_unsubscribe_invalid_id() {
        let subscribers = LogSubscribers::new();

        let result = subscribers.unsubscribe(999);
        assert!(!result);
    }

    #[test]
    fn test_level_filtering() {
        let subscribers = LogSubscribers::new();
        let client = test_client(1);

        // Subscribe with warn filter
        subscribers.subscribe(&client, Some(LogLevel::Warn));

        // Should match warn and error
        let matches = subscribers.matching_subscriptions(LogLevel::Error);
        assert_eq!(matches.len(), 1);

        let matches = subscribers.matching_subscriptions(LogLevel::Warn);
        assert_eq!(matches.len(), 1);

        // Should NOT match info and below
        let matches = subscribers.matching_subscriptions(LogLevel::Info);
        assert_eq!(matches.len(), 0);

        let matches = subscribers.matching_subscriptions(LogLevel::Debug);
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_multiple_subscriptions_same_client() {
        let subscribers = LogSubscribers::new();
        let client = test_client(1);

        let id1 = subscribers.subscribe(&client, Some(LogLevel::Error));
        let id2 = subscribers.subscribe(&client, Some(LogLevel::Debug));

        assert_eq!(subscribers.len(), 2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_broadcast_to_multiple_subscribers() {
        let subscribers = LogSubscribers::new();
        let client1 = test_client(1);
        let client2 = test_client(2);
        let client3 = test_client(3);

        subscribers.subscribe(&client1, None);
        subscribers.subscribe(&client2, None);
        subscribers.subscribe(&client3, Some(LogLevel::Warn));

        // Info should match 2 subscribers (client 1, 2 with no filter)
        let matches = subscribers.matching_subscriptions(LogLevel::Info);
        assert_eq!(matches.len(), 2);

        // Error should match all 3
        let matches = subscribers.matching_subscriptions(LogLevel::Error);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_remove_client() {
        let subscribers = LogSubscribers::new();
        let client1 = test_client(1);
        let client2 = test_client(2);

        subscribers.subscribe(&client1, None);
        subscribers.subscribe(&client1, Some(LogLevel::Warn));
        subscribers.subscribe(&client2, None);

        assert_eq!(subscribers.len(), 3);

        subscribers.remove_client(client1.id());

        assert_eq!(subscribers.len(), 1);
        let remaining = subscribers.all_subscriptions();
        assert_eq!(remaining[0].client_id, client2.id());
    }

    #[test]
    fn test_broadcast_empty_subscribers() {
        let subscribers = LogSubscribers::new();

        let matches = subscribers.matching_subscriptions(LogLevel::Info);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_concurrent_subscribe_safety() {
        use std::thread;

        let subscribers = LogSubscribers::new();
        let subscribers = std::sync::Arc::new(subscribers);

        // Pre-create clients (can't create in threads due to lifetime)
        let clients: Vec<_> = (0..10).map(test_client).collect();

        let mut handles = vec![];

        for client in clients {
            let subs = subscribers.clone();
            handles.push(thread::spawn(move || {
                subs.subscribe(&client, None);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(subscribers.len(), 10);
    }

    #[test]
    fn test_concurrent_subscribe_unsubscribe() {
        use std::thread;

        let subscribers = LogSubscribers::new();
        let subscribers = std::sync::Arc::new(subscribers);

        // Pre-create clients
        let clients: Vec<_> = (0..5).map(test_client).collect();

        // Subscribe from multiple threads
        let mut handles = vec![];
        for client in clients {
            let subs = subscribers.clone();
            handles.push(thread::spawn(move || subs.subscribe(&client, None)));
        }

        let ids: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Unsubscribe from multiple threads
        let mut handles = vec![];
        for id in ids {
            let subs = subscribers.clone();
            handles.push(thread::spawn(move || subs.unsubscribe(id)));
        }

        for handle in handles {
            assert!(handle.join().unwrap());
        }

        assert!(subscribers.is_empty());
    }

    #[test]
    fn test_subscription_cleanup_on_disconnect() {
        let subscribers = LogSubscribers::new();
        let client = test_client(1);

        subscribers.subscribe(&client, None);
        subscribers.subscribe(&client, Some(LogLevel::Warn));
        assert_eq!(subscribers.len(), 2);

        // Simulate disconnect
        subscribers.remove_client(client.id());
        assert!(subscribers.is_empty());
    }

    #[test]
    fn test_entry_to_payload() {
        let entry = LogEntry::new("INFO", "test::module", "test message");
        let payload = entry_to_payload(&entry);

        assert_eq!(payload.level, LogLevel::Info);
        assert_eq!(payload.target, "test::module");
        assert_eq!(payload.message, "test message");
        assert_eq!(payload.source, LogSource::Server);
    }

    #[test]
    fn test_weak_client_expires_when_dropped() {
        let subscribers = LogSubscribers::new();
        let client = test_client(1);

        let id = subscribers.subscribe(&client, None);

        // Client is still alive
        let subs = subscribers.all_subscriptions();
        assert!(subs[0].client.upgrade().is_some());

        // Drop the client
        drop(client);

        // Weak reference should now be expired
        let subs = subscribers.all_subscriptions();
        assert!(subs[0].client.upgrade().is_none());

        // Unsubscribe still works
        assert!(subscribers.unsubscribe(id));
    }
}
