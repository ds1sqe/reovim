//! Event scope tracking for deterministic event synchronization
//!
//! This module provides GC-like tracking of event lifecycles:
//! - Increment counter on event emit
//! - Decrement counter on dispatch completion
//! - Wait for counter to reach zero
//!
//! # Example
//!
//! ```ignore
//! let scope = EventScope::new();
//!
//! // Track events
//! scope.increment();  // Event emitted
//! scope.increment();  // Child event emitted
//!
//! // Later, when events complete
//! scope.decrement();  // First event done
//! scope.decrement();  // Second event done, counter = 0
//!
//! // Wait for all events
//! scope.wait().await;  // Returns immediately since counter = 0
//! ```

use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use tokio::sync::Notify;

/// Unique identifier for event scopes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(u64);

impl ScopeId {
    /// Generate a new unique scope ID
    fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value
    #[must_use]
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ScopeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Tracks in-flight events and signals completion
///
/// `EventScope` uses atomic reference counting to track how many events
/// are currently being processed. When the counter reaches zero, any
/// waiters are notified.
///
/// Scopes are cheap to clone (Arc-based) and can be passed through
/// event chains to track the entire lifecycle.
#[derive(Debug, Clone)]
pub struct EventScope {
    id: ScopeId,
    in_flight: Arc<AtomicUsize>,
    completion: Arc<Notify>,
    #[cfg(debug_assertions)]
    created_at: std::time::Instant,
}

impl EventScope {
    /// Create a new root scope
    ///
    /// The scope starts with an in-flight count of 0.
    #[must_use]
    pub fn new() -> Self {
        let scope = Self {
            id: ScopeId::next(),
            in_flight: Arc::new(AtomicUsize::new(0)),
            completion: Arc::new(Notify::new()),
            #[cfg(debug_assertions)]
            created_at: std::time::Instant::now(),
        };
        tracing::debug!(scope_id = %scope.id, "EventScope created");
        scope
    }

    /// Increment in-flight counter (called when event is emitted)
    pub fn increment(&self) {
        let prev = self.in_flight.fetch_add(1, Ordering::SeqCst);
        tracing::trace!(
            scope_id = %self.id,
            in_flight = prev + 1,
            "EventScope increment"
        );
    }

    /// Decrement counter and notify if zero (called when event dispatch completes)
    pub fn decrement(&self) {
        let prev = self.in_flight.fetch_sub(1, Ordering::SeqCst);
        tracing::trace!(
            scope_id = %self.id,
            in_flight = prev.saturating_sub(1),
            "EventScope decrement"
        );

        if prev == 1 {
            #[cfg(debug_assertions)]
            tracing::debug!(
                scope_id = %self.id,
                elapsed_ms = %self.created_at.elapsed().as_millis(),
                "EventScope completed"
            );
            self.completion.notify_waiters();
        }
    }

    /// Wait for all events in scope to complete
    ///
    /// Returns immediately if the counter is already zero.
    pub async fn wait(&self) {
        if self.in_flight.load(Ordering::SeqCst) == 0 {
            return;
        }
        self.completion.notified().await;
    }

    /// Wait with timeout
    ///
    /// Returns `true` if completed within timeout, `false` if timed out.
    pub async fn wait_timeout(&self, duration: std::time::Duration) -> bool {
        if self.in_flight.load(Ordering::SeqCst) == 0 {
            return true;
        }
        tokio::time::timeout(duration, self.completion.notified())
            .await
            .is_ok()
    }

    /// Wait with warning on timeout
    ///
    /// Logs a warning if the scope doesn't complete within the timeout,
    /// which may indicate a scope leak (missing decrement).
    pub async fn wait_with_warning(&self, timeout: std::time::Duration) {
        if !self.wait_timeout(timeout).await {
            tracing::warn!(
                scope_id = %self.id,
                in_flight = self.in_flight(),
                "EventScope timeout - possible leak"
            );
        }
    }

    /// Get the scope ID
    #[must_use]
    pub fn id(&self) -> ScopeId {
        self.id
    }

    /// Get current in-flight count
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Check if all events have completed (in-flight count is zero)
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.in_flight() == 0
    }
}

impl Default for EventScope {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_id_uniqueness() {
        let id1 = ScopeId::next();
        let id2 = ScopeId::next();
        let id3 = ScopeId::next();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_scope_new() {
        let scope = EventScope::new();
        assert_eq!(scope.in_flight(), 0);
        assert!(scope.is_complete());
    }

    #[test]
    fn test_scope_increment_decrement() {
        let scope = EventScope::new();

        scope.increment();
        assert_eq!(scope.in_flight(), 1);
        assert!(!scope.is_complete());

        scope.increment();
        assert_eq!(scope.in_flight(), 2);

        scope.decrement();
        assert_eq!(scope.in_flight(), 1);

        scope.decrement();
        assert_eq!(scope.in_flight(), 0);
        assert!(scope.is_complete());
    }

    #[test]
    fn test_scope_clone_shares_state() {
        let scope1 = EventScope::new();
        let scope2 = scope1.clone();

        scope1.increment();
        assert_eq!(scope2.in_flight(), 1);

        scope2.increment();
        assert_eq!(scope1.in_flight(), 2);

        scope1.decrement();
        scope2.decrement();
        assert_eq!(scope1.in_flight(), 0);
        assert_eq!(scope2.in_flight(), 0);
    }

    #[tokio::test]
    async fn test_scope_wait_immediate() {
        let scope = EventScope::new();
        // Should return immediately when count is 0
        scope.wait().await;
    }

    #[tokio::test]
    async fn test_scope_wait_after_decrement() {
        let scope = EventScope::new();
        scope.increment();

        let scope_clone = scope.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            scope_clone.decrement();
        });

        scope.wait().await;
        assert!(scope.is_complete());
    }

    #[tokio::test]
    async fn test_scope_wait_timeout_success() {
        let scope = EventScope::new();
        scope.increment();

        let scope_clone = scope.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            scope_clone.decrement();
        });

        let completed = scope
            .wait_timeout(std::time::Duration::from_millis(100))
            .await;
        assert!(completed);
    }

    #[tokio::test]
    async fn test_scope_wait_timeout_failure() {
        let scope = EventScope::new();
        scope.increment();
        // Don't decrement - should timeout

        let completed = scope
            .wait_timeout(std::time::Duration::from_millis(10))
            .await;
        assert!(!completed);
    }

    #[tokio::test]
    async fn test_scope_multiple_waiters() {
        let scope = EventScope::new();
        scope.increment();

        let scope1 = scope.clone();
        let scope2 = scope.clone();
        let scope3 = scope.clone();

        let handle1 = tokio::spawn(async move { scope1.wait().await });
        let handle2 = tokio::spawn(async move { scope2.wait().await });

        // Give tasks time to start waiting
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        // Decrement should wake all waiters
        scope3.decrement();

        // Both should complete
        tokio::time::timeout(std::time::Duration::from_millis(100), handle1)
            .await
            .expect("handle1 timed out")
            .expect("handle1 panicked");
        tokio::time::timeout(std::time::Duration::from_millis(100), handle2)
            .await
            .expect("handle2 timed out")
            .expect("handle2 panicked");
    }

    #[test]
    fn test_scope_id_display() {
        let scope = EventScope::new();
        let display = format!("{}", scope.id());
        assert!(!display.is_empty());
    }
}
