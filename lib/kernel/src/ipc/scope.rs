//! Event scope for lifecycle tracking.
//!
//! `EventScope` provides GC-like synchronization for tracking event lifecycles.
//! It enables callers to wait until all events dispatched within a scope have
//! been fully processed.
//!
//! # Design Philosophy
//!
//! This is the **only** synchronization mechanism for event lifecycle tracking.
//! It uses a simple reference-counting pattern:
//! - `increment()` when an event is emitted
//! - `decrement()` when event dispatch completes
//! - `wait()` blocks until counter reaches zero
//!
//! # Use Case
//!
//! The primary use case is RPC key injection, where the caller needs to wait
//! for all effects of injected keys to complete before returning a response.
//!
//! # Example
//!
//! ```
//! use reovim_kernel::ipc::EventScope;
//! use std::time::Duration;
//!
//! let scope = EventScope::new();
//!
//! // Simulate emitting events
//! scope.increment();
//! scope.increment();
//! assert_eq!(scope.in_flight(), 2);
//!
//! // Simulate event completion
//! scope.decrement();
//! scope.decrement();
//! assert!(scope.is_complete());
//!
//! // wait() returns immediately when counter is 0
//! scope.wait();
//! ```

use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use parking_lot::{Condvar, Mutex};

/// Unique identifier for an `EventScope`.
///
/// Used for debugging and tracing to distinguish between concurrent scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(u64);

impl ScopeId {
    /// Create a new unique `ScopeId`.
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    #[inline]
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scope-{}", self.0)
    }
}

/// Internal state shared between `EventScope` clones.
struct ScopeInner {
    /// Unique identifier for this scope.
    id: ScopeId,

    /// Count of in-flight events.
    in_flight: AtomicUsize,

    /// Condition variable for waiting.
    /// The Mutex holds a dummy value; we only need the condvar.
    condvar: (Mutex<()>, Condvar),
}

/// GC-like synchronization for event lifecycle tracking.
///
/// `EventScope` tracks in-flight events and allows callers to wait until
/// all events within the scope have been fully processed.
///
/// # Thread Safety
///
/// `EventScope` is `Clone`, `Send`, and `Sync`. Cloning creates a new handle
/// to the same underlying scope - all clones share the same counter.
///
/// # Timeout Behavior
///
/// The proven production timeout is **3 seconds** (see `DEFAULT_TIMEOUT`).
/// This is long enough to handle slow handlers while preventing deadlocks.
///
/// # Example
///
/// ```
/// use reovim_kernel::ipc::EventScope;
/// use std::thread;
/// use std::time::Duration;
///
/// let scope = EventScope::new();
/// let scope2 = scope.clone(); // Same underlying scope
///
/// // Simulate async event processing
/// scope.increment();
///
/// let handle = thread::spawn(move || {
///     thread::sleep(Duration::from_millis(10));
///     scope2.decrement(); // Signal completion
/// });
///
/// // Wait for all events to complete
/// let completed = scope.wait_timeout(Duration::from_secs(1));
/// assert!(completed);
///
/// handle.join().unwrap();
/// ```
#[derive(Clone)]
pub struct EventScope {
    inner: Arc<ScopeInner>,
}

/// Default timeout for scope waiting (3 seconds).
///
/// This is the proven production value, long enough for slow handlers
/// (like tree-sitter compilation) while preventing infinite waits.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

impl EventScope {
    /// Create a new `EventScope` with counter initialized to 0.
    ///
    /// The scope is considered complete when `in_flight() == 0`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ScopeInner {
                id: ScopeId::new(),
                in_flight: AtomicUsize::new(0),
                condvar: (Mutex::new(()), Condvar::new()),
            }),
        }
    }

    /// Get the unique ID of this scope.
    #[inline]
    #[must_use]
    pub fn id(&self) -> ScopeId {
        self.inner.id
    }

    /// Increment the in-flight counter.
    ///
    /// Call this when emitting an event within the scope.
    #[inline]
    pub fn increment(&self) {
        self.inner.in_flight.fetch_add(1, Ordering::SeqCst);
    }

    /// Decrement the in-flight counter.
    ///
    /// Call this when event dispatch completes. If the counter reaches 0,
    /// all waiters are notified.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if called when counter is already 0.
    #[inline]
    pub fn decrement(&self) {
        let prev = self.inner.in_flight.fetch_sub(1, Ordering::SeqCst);
        debug_assert!(prev > 0, "EventScope::decrement called when counter is 0");

        // Notify waiters if we just reached 0
        if prev == 1 {
            let (_lock, condvar) = &self.inner.condvar;
            condvar.notify_all();
        }
    }

    /// Get the current in-flight count.
    #[inline]
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.inner.in_flight.load(Ordering::SeqCst)
    }

    /// Check if the scope is complete (no in-flight events).
    #[inline]
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.in_flight() == 0
    }

    /// Block until the in-flight counter reaches 0.
    ///
    /// If the counter is already 0, returns immediately.
    ///
    /// # Warning
    ///
    /// This can block indefinitely if events are never decremented.
    /// Prefer `wait_timeout()` in production code.
    pub fn wait(&self) {
        let (mutex, condvar) = &self.inner.condvar;
        let mut guard = mutex.lock();

        while self.in_flight() > 0 {
            condvar.wait(&mut guard);
        }
    }

    /// Block until the in-flight counter reaches 0, with timeout.
    ///
    /// Returns `true` if the scope completed, `false` if the timeout elapsed.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::ipc::EventScope;
    /// use std::time::Duration;
    ///
    /// let scope = EventScope::new();
    ///
    /// // Already complete - returns immediately
    /// assert!(scope.wait_timeout(Duration::from_millis(100)));
    ///
    /// // With in-flight event that never completes
    /// scope.increment();
    /// assert!(!scope.wait_timeout(Duration::from_millis(10)));
    /// ```
    #[must_use]
    #[allow(clippy::significant_drop_tightening)] // Guard must be held for the wait loop
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        let (mutex, condvar) = &self.inner.condvar;
        let mut guard = mutex.lock();

        let deadline = std::time::Instant::now() + timeout;

        while self.in_flight() > 0 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let result = condvar.wait_for(&mut guard, remaining);
            if result.timed_out() && self.in_flight() > 0 {
                return false;
            }
        }

        true
    }

    /// Block until complete, with default timeout and warning logging.
    ///
    /// Uses `DEFAULT_TIMEOUT` (3 seconds). Returns `true` if completed,
    /// `false` if timed out.
    ///
    /// In production, this logs a warning if the timeout is reached.
    /// For kernel-level code, we just return the result.
    #[must_use]
    pub fn wait_with_default_timeout(&self) -> bool {
        self.wait_timeout(DEFAULT_TIMEOUT)
    }
}

impl Default for EventScope {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for EventScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventScope")
            .field("id", &self.inner.id)
            .field("in_flight", &self.in_flight())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::thread};

    // ========== ScopeId tests ==========

    #[test]
    fn test_scope_id_unique() {
        let id1 = ScopeId::new();
        let id2 = ScopeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_scope_id_display() {
        let id = ScopeId::new();
        let display = format!("{id}");
        assert!(display.starts_with("scope-"));
    }

    #[test]
    fn test_scope_id_as_u64() {
        let id = ScopeId::new();
        assert!(id.as_u64() > 0);
    }

    // ========== EventScope basic tests ==========

    #[test]
    fn test_event_scope_new() {
        let scope = EventScope::new();
        assert_eq!(scope.in_flight(), 0);
        assert!(scope.is_complete());
    }

    #[test]
    fn test_event_scope_increment_decrement() {
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
    fn test_event_scope_clone_shares_state() {
        let scope1 = EventScope::new();
        let scope2 = scope1.clone();

        scope1.increment();
        assert_eq!(scope2.in_flight(), 1);

        scope2.increment();
        assert_eq!(scope1.in_flight(), 2);

        scope1.decrement();
        scope1.decrement();
        assert!(scope2.is_complete());
    }

    #[test]
    fn test_event_scope_unique_ids() {
        let scope1 = EventScope::new();
        let scope2 = EventScope::new();
        assert_ne!(scope1.id(), scope2.id());
    }

    #[test]
    fn test_event_scope_clone_same_id() {
        let scope1 = EventScope::new();
        let scope2 = scope1.clone();
        assert_eq!(scope1.id(), scope2.id());
    }

    // ========== EventScope wait tests ==========

    #[test]
    fn test_event_scope_wait_immediate() {
        let scope = EventScope::new();
        // Already complete - should return immediately
        scope.wait();
        assert!(scope.is_complete());
    }

    #[test]
    fn test_event_scope_wait_timeout_immediate() {
        let scope = EventScope::new();
        let completed = scope.wait_timeout(Duration::from_millis(100));
        assert!(completed);
    }

    #[test]
    fn test_event_scope_wait_timeout_expires() {
        let scope = EventScope::new();
        scope.increment();

        let completed = scope.wait_timeout(Duration::from_millis(10));
        assert!(!completed);
        assert_eq!(scope.in_flight(), 1);
    }

    #[test]
    fn test_event_scope_wait_timeout_completes() {
        let scope = EventScope::new();
        let scope2 = scope.clone();

        scope.increment();

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            scope2.decrement();
        });

        let completed = scope.wait_timeout(Duration::from_secs(1));
        assert!(completed);
        handle.join().unwrap();
    }

    #[test]
    fn test_event_scope_wait_with_thread() {
        let scope = EventScope::new();
        let scope2 = scope.clone();

        scope.increment();

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(5));
            scope2.decrement();
        });

        scope.wait();
        assert!(scope.is_complete());
        handle.join().unwrap();
    }

    // ========== EventScope concurrent tests ==========

    #[test]
    fn test_event_scope_concurrent_increments() {
        let scope = EventScope::new();
        let mut handles = vec![];

        for _ in 0..10 {
            let s = scope.clone();
            handles.push(thread::spawn(move || {
                s.increment();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(scope.in_flight(), 10);
    }

    #[test]
    fn test_event_scope_concurrent_increment_decrement() {
        let scope = EventScope::new();
        let mut handles = vec![];

        // First, increment 10 times
        for _ in 0..10 {
            scope.increment();
        }

        // Then decrement from multiple threads
        for _ in 0..10 {
            let s = scope.clone();
            handles.push(thread::spawn(move || {
                s.decrement();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert!(scope.is_complete());
    }

    #[test]
    fn test_event_scope_multiple_waiters() {
        let scope = EventScope::new();
        scope.increment();

        let mut handles = vec![];

        // Start multiple waiters
        for _ in 0..3 {
            let s = scope.clone();
            handles.push(thread::spawn(move || {
                let completed = s.wait_timeout(Duration::from_secs(1));
                assert!(completed);
            }));
        }

        // Small delay then decrement
        thread::sleep(Duration::from_millis(10));
        scope.decrement();

        for h in handles {
            h.join().unwrap();
        }
    }

    // ========== Debug and Default tests ==========

    #[test]
    fn test_event_scope_debug() {
        let scope = EventScope::new();
        scope.increment();
        let debug_str = format!("{scope:?}");
        assert!(debug_str.contains("EventScope"));
        assert!(debug_str.contains("in_flight: 1"));
    }

    #[test]
    fn test_event_scope_default() {
        let scope = EventScope::default();
        assert!(scope.is_complete());
    }

    #[test]
    fn test_event_scope_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EventScope>();
    }
}
