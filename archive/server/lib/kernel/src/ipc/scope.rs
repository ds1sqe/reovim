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
//! use reovim_kernel::api::v1::*;
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

use reovim_arch::sync::{Condvar, Mutex};

/// Unique identifier for an `EventScope`.
///
/// Used for debugging and tracing to distinguish between concurrent scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(u64);

impl ScopeId {
    /// Create a new unique `ScopeId`.
    pub(crate) fn new() -> Self {
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
/// use reovim_kernel::api::v1::*;
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
    /// use reovim_kernel::api::v1::*;
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
