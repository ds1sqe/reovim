//! Timer mechanism for delayed and periodic work scheduling.
//!
//! Linux equivalent: `kernel/time/timer.c`
//!
//! This module provides timer primitives for scheduling work to execute
//! after a delay or at regular intervals. Timers integrate with the
//! [`Runtime`](super::Runtime) tick cycle and execute callbacks via the
//! work queue for panic safety.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                       TimerWheel                             │
//! │  ┌──────────────────────────────────────────────────────┐   │
//! │  │  HashMap<TimerId, TimerEntry>                        │   │
//! │  │    - deadline: Instant                               │   │
//! │  │    - work: TimerWork (OneShot or Periodic)           │   │
//! │  └──────────────────────────────────────────────────────┘   │
//! │                           │                                  │
//! │                           │ tick(now)                        │
//! │                           v                                  │
//! │  ┌──────────────────────────────────────────────────────┐   │
//! │  │  WorkQueue <- expired timers scheduled as Tasks      │   │
//! │  └──────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//! use std::time::Duration;
//! use std::sync::atomic::{AtomicUsize, Ordering};
//! use std::sync::Arc;
//!
//! let mut runtime = Runtime::new();
//! runtime.boot();
//!
//! // Schedule a one-shot timer
//! let counter = Arc::new(AtomicUsize::new(0));
//! let counter_clone = counter.clone();
//! let _handle = runtime.schedule_delayed(Duration::from_millis(10), move || {
//!     counter_clone.fetch_add(1, Ordering::SeqCst);
//! });
//!
//! // Timer fires when tick advances past deadline
//! ```
//!
//! # Panic Safety
//!
//! Timer callbacks are executed via the work queue, which uses the executor's
//! `catch_unwind` for panic safety. A panicking callback is marked as failed
//! and does not repeat (for periodic timers).

use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use crate::arch::sync::Mutex;

use super::task::{Priority, Task};

/// Unique timer identifier.
///
/// Timer IDs are monotonically increasing and never reused within a session.
/// This follows the same pattern as [`TaskId`](super::TaskId) and
/// [`BufferId`](crate::mm::BufferId).
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let id1 = TimerId::new();
/// let id2 = TimerId::new();
/// assert_ne!(id1, id2);
/// assert!(id2.as_u64() > id1.as_u64());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimerId(u64);

impl TimerId {
    /// Create a new unique timer ID.
    ///
    /// IDs start at 1 and increment monotonically.
    #[must_use]
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw numeric value.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Create from raw value.
    ///
    /// Primarily for testing and deserialization.
    #[inline]
    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

impl Default for TimerId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TimerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Timer({})", self.0)
    }
}

/// Timer configuration.
///
/// Specifies when a timer should fire and at what priority the callback
/// should execute. This struct is provided for advanced use cases where
/// you need more control than `Runtime::schedule_delayed()` provides.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
/// use std::time::Duration;
///
/// // One-shot timer after 100ms
/// let config = TimerConfig {
///     delay: Duration::from_millis(100),
///     interval: None,
///     priority: Priority::NORMAL,
/// };
///
/// // Periodic timer every 50ms
/// let periodic = TimerConfig {
///     delay: Duration::from_millis(50),
///     interval: Some(Duration::from_millis(50)),
///     priority: Priority::LOW,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct TimerConfig {
    /// Initial delay before first execution.
    pub delay: Duration,

    /// Optional interval for periodic timers.
    ///
    /// If `Some`, the timer will reschedule after each execution.
    /// If `None`, the timer is one-shot and removed after firing.
    pub interval: Option<Duration>,

    /// Priority for the scheduled task.
    ///
    /// Determines execution order relative to other work in the queue.
    pub priority: Priority,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            delay: Duration::ZERO,
            interval: None,
            priority: Priority::NORMAL,
        }
    }
}

/// Type-erased one-shot timer callback.
type BoxedOneShotCallback = Box<dyn FnOnce() + Send + 'static>;

/// Type-erased periodic timer callback.
///
/// Uses `Arc` instead of `Box` because periodic callbacks need to be cloned
/// for each invocation.
type BoxedPeriodicCallback = Arc<dyn Fn() + Send + Sync + 'static>;

/// Timer callback storage.
///
/// Distinguishes between one-shot (`FnOnce`) and periodic (`Fn`) callbacks.
enum TimerWork {
    /// One-shot timer: callback is consumed on execution.
    OneShot(Option<BoxedOneShotCallback>),

    /// Periodic timer: callback can be called multiple times.
    Periodic(BoxedPeriodicCallback),
}

impl fmt::Debug for TimerWork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OneShot(_) => write!(f, "OneShot(...)"),
            Self::Periodic(_) => write!(f, "Periodic(...)"),
        }
    }
}

/// Internal timer entry in the wheel.
struct TimerEntry {
    /// Unique identifier.
    id: TimerId,

    /// When the timer should fire.
    deadline: Instant,

    /// Interval for periodic rescheduling.
    interval: Option<Duration>,

    /// Priority for the work task.
    priority: Priority,

    /// The callback to execute.
    work: TimerWork,

    /// Whether this timer has been cancelled.
    cancelled: AtomicBool,
}

impl TimerEntry {
    /// Check if this timer has been cancelled.
    #[inline]
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Mark this timer as cancelled.
    #[inline]
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl fmt::Debug for TimerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimerEntry")
            .field("id", &self.id)
            .field("deadline", &self.deadline)
            .field("interval", &self.interval)
            .field("priority", &self.priority)
            .field("work", &self.work)
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Default maximum number of concurrent timers.
pub const DEFAULT_MAX_TIMERS: usize = 1024;

/// Timer wheel for managing scheduled timers.
///
/// The timer wheel stores active timers indexed by ID and checks for
/// expired timers during each tick. Expired timers have their callbacks
/// scheduled as tasks on the work queue.
///
/// # Thread Safety
///
/// The wheel uses a `Mutex<HashMap>` internally, making it safe to
/// schedule and cancel from multiple threads.
pub struct TimerWheel {
    /// Active timers indexed by ID.
    timers: Mutex<HashMap<TimerId, TimerEntry>>,

    /// Next timer ID (atomic for thread-safe allocation).
    next_id: AtomicU64,

    /// Maximum number of timers allowed.
    max_timers: usize,

    /// Count of timers dropped due to capacity limit.
    dropped: AtomicU64,
}

impl TimerWheel {
    /// Create a new timer wheel with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_timers(DEFAULT_MAX_TIMERS)
    }

    /// Create a timer wheel with specified maximum timer count.
    #[must_use]
    pub fn with_max_timers(max_timers: usize) -> Self {
        Self {
            timers: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            max_timers,
            dropped: AtomicU64::new(0),
        }
    }

    /// Schedule a one-shot timer.
    ///
    /// Returns a handle that cancels the timer when dropped.
    pub fn schedule_oneshot<F>(
        self: &Arc<Self>,
        delay: Duration,
        priority: Priority,
        work: F,
    ) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        self.schedule_internal(delay, None, priority, TimerWork::OneShot(Some(Box::new(work))))
    }

    /// Schedule a periodic timer.
    ///
    /// Returns a handle that cancels the timer when dropped.
    pub fn schedule_periodic<F>(
        self: &Arc<Self>,
        interval: Duration,
        priority: Priority,
        work: F,
    ) -> TimerHandle
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.schedule_internal(
            interval,
            Some(interval),
            priority,
            TimerWork::Periodic(Arc::new(work)),
        )
    }

    /// Internal scheduling implementation.
    fn schedule_internal(
        self: &Arc<Self>,
        delay: Duration,
        interval: Option<Duration>,
        priority: Priority,
        work: TimerWork,
    ) -> TimerHandle {
        let id = TimerId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let deadline = Instant::now() + delay;

        let mut timers = self.timers.lock();

        // Check capacity limit
        if timers.len() >= self.max_timers {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            // Timer limit exceeded - return a failed handle
            // Caller can check handle.is_failed() to detect this
            return TimerHandle::failed(id);
        }

        let entry = TimerEntry {
            id,
            deadline,
            interval,
            priority,
            work,
            cancelled: AtomicBool::new(false),
        };

        timers.insert(id, entry);
        drop(timers);

        TimerHandle::new(id, Arc::downgrade(self))
    }

    /// Cancel a timer by ID.
    ///
    /// Returns `true` if the timer was found and cancelled, `false` if
    /// it was already cancelled or has fired.
    pub fn cancel(&self, id: TimerId) -> bool {
        let timers = self.timers.lock();
        timers.get(&id).is_some_and(|entry| {
            if entry.is_cancelled() {
                false
            } else {
                entry.cancel();
                true
            }
        })
    }

    /// Check if a timer is still pending (scheduled but not yet fired).
    ///
    /// Returns `true` if the timer exists and hasn't been cancelled or fired.
    pub fn is_pending(&self, id: TimerId) -> bool {
        let timers = self.timers.lock();
        timers.get(&id).is_some_and(|entry| !entry.is_cancelled())
    }

    /// Process expired timers and return tasks to execute.
    ///
    /// This should be called during each tick with the current time.
    /// Returns a vector of tasks for expired timers.
    pub fn tick(&self, now: Instant) -> Vec<Task> {
        let mut tasks = Vec::new();
        let mut to_remove = Vec::new();
        let mut to_reschedule = Vec::new();

        {
            let mut timers = self.timers.lock();

            for (id, entry) in timers.iter_mut() {
                // Skip cancelled timers
                if entry.is_cancelled() {
                    to_remove.push(*id);
                    continue;
                }

                // Check if timer has expired
                if entry.deadline <= now {
                    match &mut entry.work {
                        TimerWork::OneShot(work_opt) => {
                            // Take the FnOnce callback
                            if let Some(work) = work_opt.take() {
                                let task = Task::with_priority(entry.priority, work);
                                tasks.push(task);
                            }
                            to_remove.push(*id);
                        }
                        TimerWork::Periodic(work) => {
                            // Clone the Arc callback for execution
                            let work_clone = Arc::clone(work);
                            let task = Task::with_priority(entry.priority, move || work_clone());
                            tasks.push(task);

                            // Reschedule for next interval
                            if let Some(interval) = entry.interval {
                                to_reschedule.push((*id, now + interval));
                            }
                        }
                    }
                }
            }

            // Remove completed/cancelled timers
            for id in to_remove {
                timers.remove(&id);
            }

            // Update deadlines for periodic timers
            for (id, new_deadline) in to_reschedule {
                if let Some(entry) = timers.get_mut(&id) {
                    entry.deadline = new_deadline;
                }
            }
        }

        tasks
    }

    /// Get the number of pending timers.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.timers.lock().len()
    }

    /// Get the number of timers dropped due to capacity limits.
    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl Default for TimerWheel {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TimerWheel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimerWheel")
            .field("pending", &self.pending_count())
            .field("max_timers", &self.max_timers)
            .field("dropped", &self.dropped_count())
            .finish_non_exhaustive()
    }
}

/// Handle to a scheduled timer with RAII cancellation.
///
/// When dropped, the handle automatically cancels the timer if it hasn't
/// fired yet. This follows the RAII pattern for resource management.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
/// use std::time::Duration;
///
/// let mut runtime = Runtime::new();
/// runtime.boot();
///
/// {
///     // Timer is scheduled
///     let handle = runtime.schedule_delayed(Duration::from_secs(10), || {
///         println!("This won't print");
///     });
///     // handle dropped here - timer is cancelled
/// }
///
/// // Timer was cancelled, won't fire
/// ```
///
/// # Manual Cancellation
///
/// You can also explicitly cancel via [`Runtime::cancel_timer`](super::Runtime::cancel_timer):
///
/// ```
/// use reovim_kernel::api::v1::*;
/// use std::time::Duration;
///
/// let mut runtime = Runtime::new();
/// runtime.boot();
///
/// let handle = runtime.schedule_delayed(Duration::from_secs(10), || {});
/// let id = handle.id();
///
/// // Explicit cancellation
/// std::mem::forget(handle); // Prevent drop cancellation
/// let was_pending = runtime.cancel_timer(id);
/// ```
pub struct TimerHandle {
    /// The timer's unique identifier.
    id: TimerId,

    /// Weak reference to the timer wheel for cancellation.
    ///
    /// If the wheel has been dropped (runtime shutdown), cancellation
    /// is a no-op since all timers are already gone.
    wheel: Option<Weak<TimerWheel>>,

    /// Whether this handle represents a failed scheduling attempt.
    ///
    /// When `max_timers` is exceeded, we return a "failed" handle that
    /// doesn't actually have a timer to cancel.
    failed: bool,
}

impl TimerHandle {
    /// Create a new timer handle.
    pub(crate) const fn new(id: TimerId, wheel: Weak<TimerWheel>) -> Self {
        Self {
            id,
            wheel: Some(wheel),
            failed: false,
        }
    }

    /// Create a failed handle (timer wasn't actually scheduled).
    pub(crate) const fn failed(id: TimerId) -> Self {
        Self {
            id,
            wheel: None,
            failed: true,
        }
    }

    /// Get the timer's unique identifier.
    #[inline]
    #[must_use]
    pub const fn id(&self) -> TimerId {
        self.id
    }

    /// Check if this handle represents a failed scheduling attempt.
    ///
    /// Returns `true` if the timer was not actually scheduled (e.g., due
    /// to hitting the `max_timers` limit).
    #[inline]
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        self.failed
    }

    /// Prevent automatic cancellation on drop.
    ///
    /// After calling this, dropping the handle will NOT cancel the timer.
    /// Use this when you want the timer to fire regardless of handle lifetime.
    ///
    /// Returns the timer ID for manual cancellation if needed later.
    #[must_use]
    pub fn detach(mut self) -> TimerId {
        let id = self.id;
        self.wheel = None; // Prevent Drop from cancelling
        id
    }
}

impl Drop for TimerHandle {
    fn drop(&mut self) {
        // If we have a wheel reference and it's still alive, cancel the timer
        if let Some(ref weak) = self.wheel
            && let Some(wheel) = weak.upgrade()
        {
            wheel.cancel(self.id);
        }
    }
}

impl fmt::Debug for TimerHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimerHandle")
            .field("id", &self.id)
            .field("failed", &self.failed)
            .field("wheel_alive", &self.wheel.as_ref().is_some_and(|w| w.strong_count() > 0))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_id_unique() {
        let id1 = TimerId::new();
        let id2 = TimerId::new();
        let id3 = TimerId::new();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert!(id2.as_u64() > id1.as_u64());
        assert!(id3.as_u64() > id2.as_u64());
    }

    #[test]
    fn test_timer_id_display() {
        let id = TimerId::from_raw(42);
        assert_eq!(format!("{id}"), "Timer(42)");
    }

    #[test]
    fn test_timer_id_from_raw() {
        let id = TimerId::from_raw(123);
        assert_eq!(id.as_u64(), 123);
    }

    #[test]
    fn test_timer_id_default() {
        let id1 = TimerId::default();
        let id2 = TimerId::default();
        assert_ne!(id1, id2); // Each default creates a new unique ID
    }

    #[test]
    fn test_timer_id_ordering() {
        let id1 = TimerId::from_raw(10);
        let id2 = TimerId::from_raw(20);
        let id3 = TimerId::from_raw(10);

        assert!(id1 < id2);
        assert!(id2 > id1);
        assert_eq!(id1, id3);
    }

    #[test]
    fn test_timer_config_default() {
        let config = TimerConfig::default();
        assert_eq!(config.delay, Duration::ZERO);
        assert!(config.interval.is_none());
        assert_eq!(config.priority, Priority::NORMAL);
    }

    #[test]
    fn test_timer_config_periodic() {
        let config = TimerConfig {
            delay: Duration::from_millis(100),
            interval: Some(Duration::from_millis(50)),
            priority: Priority::HIGH,
        };

        assert_eq!(config.delay, Duration::from_millis(100));
        assert_eq!(config.interval, Some(Duration::from_millis(50)));
        assert_eq!(config.priority, Priority::HIGH);
    }

    #[test]
    fn test_timer_handle_debug() {
        let handle = TimerHandle::failed(TimerId::from_raw(99));
        let debug = format!("{handle:?}");
        assert!(debug.contains("TimerHandle"));
        assert!(debug.contains("99"));
        assert!(debug.contains("failed: true"));
    }

    #[test]
    fn test_timer_handle_failed() {
        let handle = TimerHandle::failed(TimerId::from_raw(1));
        assert!(handle.is_failed());
        assert_eq!(handle.id().as_u64(), 1);
    }

    #[test]
    fn test_timer_handle_detach() {
        let handle = TimerHandle::failed(TimerId::from_raw(42));
        let id = handle.detach();
        assert_eq!(id.as_u64(), 42);
        // Handle is consumed, no cancel on drop
    }

    #[test]
    fn test_timer_entry_cancel() {
        let entry = TimerEntry {
            id: TimerId::from_raw(1),
            deadline: Instant::now(),
            interval: None,
            priority: Priority::NORMAL,
            work: TimerWork::OneShot(Some(Box::new(|| {}))),
            cancelled: AtomicBool::new(false),
        };

        assert!(!entry.is_cancelled());
        entry.cancel();
        assert!(entry.is_cancelled());
    }

    #[test]
    fn test_timer_entry_interval() {
        let one_shot = TimerEntry {
            id: TimerId::from_raw(1),
            deadline: Instant::now(),
            interval: None,
            priority: Priority::NORMAL,
            work: TimerWork::OneShot(Some(Box::new(|| {}))),
            cancelled: AtomicBool::new(false),
        };

        let periodic = TimerEntry {
            id: TimerId::from_raw(2),
            deadline: Instant::now(),
            interval: Some(Duration::from_millis(100)),
            priority: Priority::NORMAL,
            work: TimerWork::Periodic(Arc::new(|| {})),
            cancelled: AtomicBool::new(false),
        };

        // One-shot timers have no interval
        assert!(one_shot.interval.is_none());
        // Periodic timers have an interval
        assert!(periodic.interval.is_some());
    }

    #[test]
    fn test_timer_work_debug() {
        let one_shot = TimerWork::OneShot(Some(Box::new(|| {})));
        let periodic = TimerWork::Periodic(Arc::new(|| {}));

        assert_eq!(format!("{one_shot:?}"), "OneShot(...)");
        assert_eq!(format!("{periodic:?}"), "Periodic(...)");
    }

    #[test]
    fn test_timer_wheel_schedule_and_tick() {
        use std::sync::atomic::AtomicUsize;

        let wheel = Arc::new(TimerWheel::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // Schedule a timer with zero delay (fires immediately on next tick)
        let handle = wheel.schedule_oneshot(Duration::ZERO, Priority::NORMAL, move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        assert!(!handle.is_failed());
        assert_eq!(wheel.pending_count(), 1);

        // Tick should fire the timer and return a task
        let mut tasks = wheel.tick(Instant::now() + Duration::from_millis(1));
        assert_eq!(tasks.len(), 1);

        // Execute the task
        let _ = tasks[0].execute();

        // Counter should have been incremented
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Timer should be removed after firing
        assert_eq!(wheel.pending_count(), 0);
    }

    #[test]
    fn test_timer_wheel_periodic_rescheduling() {
        use std::sync::atomic::AtomicUsize;

        let wheel = Arc::new(TimerWheel::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // Schedule periodic timer with zero delay (fires immediately, repeats every 100ms)
        let handle = wheel.schedule_periodic(Duration::ZERO, Priority::NORMAL, move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        assert!(!handle.is_failed());
        let _ = handle.detach(); // Detach so it keeps running

        // First tick - should fire and reschedule
        let mut tasks = wheel.tick(Instant::now());
        assert_eq!(tasks.len(), 1);
        let _ = tasks[0].execute();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Timer should still be pending (rescheduled with interval = 0)
        assert_eq!(wheel.pending_count(), 1);

        // Second tick - should fire again
        let mut tasks = wheel.tick(Instant::now() + Duration::from_millis(1));
        assert_eq!(tasks.len(), 1);
        let _ = tasks[0].execute();
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        // Still pending
        assert_eq!(wheel.pending_count(), 1);
    }

    #[test]
    fn test_timer_handle_drop_cancels() {
        use std::sync::atomic::AtomicUsize;

        let wheel = Arc::new(TimerWheel::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        {
            // Create and drop handle in this scope
            let _handle =
                wheel.schedule_oneshot(Duration::from_secs(1), Priority::NORMAL, move || {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                });
            assert_eq!(wheel.pending_count(), 1);
            // Handle drops here, should cancel timer
        }

        // Timer should be cancelled
        // Tick should not produce any tasks
        let tasks = wheel.tick(Instant::now() + Duration::from_secs(2));
        assert!(tasks.is_empty());

        // Counter should not have been incremented
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_timer_wheel_max_timers_enforced() {
        let wheel = Arc::new(TimerWheel::with_max_timers(3)); // Very small limit

        // Schedule 3 timers successfully
        let h1 = wheel.schedule_oneshot(Duration::from_secs(1), Priority::NORMAL, || {});
        let h2 = wheel.schedule_oneshot(Duration::from_secs(1), Priority::NORMAL, || {});
        let h3 = wheel.schedule_oneshot(Duration::from_secs(1), Priority::NORMAL, || {});

        assert!(!h1.is_failed());
        assert!(!h2.is_failed());
        assert!(!h3.is_failed());
        assert_eq!(wheel.pending_count(), 3);

        // Fourth timer should fail
        let h4 = wheel.schedule_oneshot(Duration::from_secs(1), Priority::NORMAL, || {});
        assert!(h4.is_failed());
        assert_eq!(wheel.dropped_count(), 1);

        // Still only 3 pending
        assert_eq!(wheel.pending_count(), 3);
    }

    #[test]
    fn test_timer_wheel_cancel() {
        let wheel = Arc::new(TimerWheel::new());

        let handle = wheel.schedule_oneshot(Duration::from_secs(1), Priority::NORMAL, || {});

        assert_eq!(wheel.pending_count(), 1);

        // Cancel via the wheel directly
        let cancelled = wheel.cancel(handle.id());
        assert!(cancelled);

        // Timer still exists but is marked cancelled
        assert_eq!(wheel.pending_count(), 1);

        // Tick should remove cancelled timer without producing a task
        let tasks = wheel.tick(Instant::now() + Duration::from_secs(2));
        assert!(tasks.is_empty());
        assert_eq!(wheel.pending_count(), 0);
    }

    #[test]
    fn test_timer_wheel_cancel_idempotent() {
        let wheel = Arc::new(TimerWheel::new());

        let handle = wheel.schedule_oneshot(Duration::from_secs(1), Priority::NORMAL, || {});

        // First cancel succeeds
        assert!(wheel.cancel(handle.id()));

        // Second cancel returns false (already cancelled)
        assert!(!wheel.cancel(handle.id()));
    }

    #[test]
    fn test_timer_wheel_thread_safety() {
        use std::{sync::atomic::AtomicUsize, thread};

        let wheel = Arc::new(TimerWheel::new());
        let counter = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        // Spawn multiple threads that schedule timers
        for _ in 0..4 {
            let wheel = Arc::clone(&wheel);
            let counter = Arc::clone(&counter);

            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    let counter = Arc::clone(&counter);
                    let _ = wheel.schedule_oneshot(Duration::ZERO, Priority::NORMAL, move || {
                        counter.fetch_add(1, Ordering::SeqCst);
                    });
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Should have 40 pending timers (or close, some may have fired)
        let pending = wheel.pending_count();
        assert!(pending <= 40, "pending count should be <= 40, got {pending}");

        // Tick to fire all timers
        let mut tasks = wheel.tick(Instant::now() + Duration::from_secs(1));

        // Execute all tasks
        for task in &mut tasks {
            let _ = task.execute();
        }

        // Counter should match tasks executed
        assert_eq!(counter.load(Ordering::SeqCst), tasks.len());
    }

    #[test]
    fn test_timer_wheel_empty_tick() {
        let wheel = Arc::new(TimerWheel::new());

        // Tick on empty wheel should be safe
        let tasks = wheel.tick(Instant::now());
        assert!(tasks.is_empty());
        assert_eq!(wheel.pending_count(), 0);
    }
}
