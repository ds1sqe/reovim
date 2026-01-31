//! Work queue for deferred task execution.
//!
//! Linux equivalent: `kernel/workqueue.c`
//!
//! This module provides a bounded, thread-safe queue for scheduling
//! tasks to be executed later by the runtime.
//!
//! # Features
//!
//! - **Bounded capacity**: Prevents unbounded memory growth
//! - **Overflow detection**: Tracks dropped tasks without blocking
//! - **Thread-safe**: Can be accessed from multiple threads
//! - **FIFO ordering**: Tasks processed in submission order
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! let queue = WorkQueue::new();
//!
//! // Push some tasks
//! queue.push(Task::new(|| println!("Task 1")));
//! queue.push(Task::new(|| println!("Task 2")));
//!
//! // Pop and execute
//! while let Some(mut task) = queue.try_pop() {
//!     task.execute().ok();
//! }
//! ```

use std::{
    collections::VecDeque,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::arch::sync::Mutex;

use super::task::Task;

/// Default capacity for work queue.
pub const DEFAULT_CAPACITY: usize = 1024;

/// Maximum capacity (to prevent unbounded growth).
pub const MAX_CAPACITY: usize = 4096;

/// Work queue for deferred tasks.
///
/// Thread-safe, FIFO queue with bounded capacity. When the queue is full,
/// new tasks are dropped and the overflow counter is incremented.
///
/// # Thread Safety
///
/// The queue uses a `Mutex<VecDeque<Task>>` internally, making it safe
/// to push from multiple threads. However, heavy contention should be
/// avoided for performance.
pub struct WorkQueue {
    /// Queue storage.
    queue: Mutex<VecDeque<Task>>,

    /// Maximum capacity.
    capacity: usize,

    /// Number of tasks dropped due to overflow.
    dropped: AtomicUsize,
}

impl WorkQueue {
    /// Create a new work queue with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a work queue with specified capacity.
    ///
    /// Capacity is clamped to `MAX_CAPACITY`.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.min(MAX_CAPACITY);
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
            dropped: AtomicUsize::new(0),
        }
    }

    /// Push a task to the queue.
    ///
    /// Returns `true` if the task was queued, `false` if dropped due to overflow.
    /// This is non-blocking - overflow tasks are immediately dropped.
    pub fn push(&self, task: Task) -> bool {
        let mut queue = self.queue.lock();
        if queue.len() >= self.capacity {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        queue.push_back(task);
        true
    }

    /// Try to pop a task from the queue.
    ///
    /// Returns `None` if the queue is empty.
    #[must_use]
    pub fn try_pop(&self) -> Option<Task> {
        self.queue.lock().pop_front()
    }

    /// Drain all tasks from the queue.
    ///
    /// Returns all pending tasks, leaving the queue empty.
    #[must_use]
    pub fn drain(&self) -> Vec<Task> {
        let mut queue = self.queue.lock();
        queue.drain(..).collect()
    }

    /// Process up to `limit` pending tasks.
    ///
    /// Pops and executes tasks in order. Returns the number of tasks
    /// successfully executed. Tasks that fail or panic are counted
    /// but not returned.
    pub fn process_pending(&self, limit: usize) -> usize {
        let mut processed = 0;
        while processed < limit {
            let Some(mut task) = self.try_pop() else {
                break;
            };
            // Execute, ignoring errors (caller can use executor for panic handling)
            let _ = task.execute();
            processed += 1;
        }
        processed
    }

    /// Get the number of pending tasks.
    ///
    /// Note: This is a snapshot and may change immediately after returning.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    /// Check if the queue is empty.
    ///
    /// Note: This is a snapshot and may change immediately after returning.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }

    /// Get the queue capacity.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the number of dropped tasks (overflow).
    ///
    /// This count accumulates over the lifetime of the queue.
    #[must_use]
    pub fn dropped_count(&self) -> usize {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Clear all pending tasks without executing them.
    ///
    /// Returns the number of tasks cleared.
    pub fn clear(&self) -> usize {
        let mut queue = self.queue.lock();
        let count = queue.len();
        queue.clear();
        count
    }
}

impl Default for WorkQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for WorkQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkQueue")
            .field("len", &self.len())
            .field("capacity", &self.capacity)
            .field("dropped", &self.dropped_count())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::sync::{Arc, atomic::AtomicUsize},
    };

    // === Basic tests ===

    #[test]
    fn test_new() {
        let queue = WorkQueue::new();
        assert_eq!(queue.capacity(), DEFAULT_CAPACITY);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.dropped_count(), 0);
    }

    #[test]
    fn test_with_capacity() {
        let queue = WorkQueue::with_capacity(100);
        assert_eq!(queue.capacity(), 100);
    }

    #[test]
    fn test_with_capacity_clamped() {
        let queue = WorkQueue::with_capacity(MAX_CAPACITY + 1000);
        assert_eq!(queue.capacity(), MAX_CAPACITY);
    }

    #[test]
    fn test_push_and_pop() {
        let queue = WorkQueue::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        queue.push(Task::new(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }));

        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());

        let mut task = queue.try_pop().unwrap();
        assert!(task.execute().is_ok());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_fifo_order() {
        let queue = WorkQueue::new();
        let order = Arc::new(Mutex::new(Vec::new()));

        for i in 0..5 {
            let order_clone = Arc::clone(&order);
            queue.push(Task::new(move || {
                order_clone.lock().push(i);
            }));
        }

        while let Some(mut task) = queue.try_pop() {
            task.execute().ok();
        }

        let result = order.lock().clone();
        assert_eq!(result, vec![0, 1, 2, 3, 4]);
    }

    // === Overflow tests ===

    #[test]
    fn test_overflow() {
        let queue = WorkQueue::with_capacity(3);

        assert!(queue.push(Task::new(|| {})));
        assert!(queue.push(Task::new(|| {})));
        assert!(queue.push(Task::new(|| {})));
        assert!(!queue.push(Task::new(|| {}))); // Should fail
        assert!(!queue.push(Task::new(|| {}))); // Should fail

        assert_eq!(queue.len(), 3);
        assert_eq!(queue.dropped_count(), 2);
    }

    #[test]
    fn test_overflow_recovery() {
        let queue = WorkQueue::with_capacity(2);

        queue.push(Task::new(|| {}));
        queue.push(Task::new(|| {}));
        assert!(!queue.push(Task::new(|| {}))); // Overflow

        let _ = queue.try_pop(); // Free up space
        assert!(queue.push(Task::new(|| {}))); // Should succeed now
        assert_eq!(queue.len(), 2);
    }

    // === Drain tests ===

    #[test]
    fn test_drain() {
        let queue = WorkQueue::new();
        for _ in 0..5 {
            queue.push(Task::new(|| {}));
        }

        let tasks = queue.drain();
        assert_eq!(tasks.len(), 5);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_drain_empty() {
        let queue = WorkQueue::new();
        let tasks = queue.drain();
        assert!(tasks.is_empty());
    }

    // === Process pending tests ===

    #[test]
    fn test_process_pending() {
        let queue = WorkQueue::new();
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let counter_clone = Arc::clone(&counter);
            queue.push(Task::new(move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }));
        }

        // Process only 5
        let processed = queue.process_pending(5);
        assert_eq!(processed, 5);
        assert_eq!(counter.load(Ordering::SeqCst), 5);
        assert_eq!(queue.len(), 5);

        // Process remaining
        let processed = queue.process_pending(10);
        assert_eq!(processed, 5);
        assert_eq!(counter.load(Ordering::SeqCst), 10);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_process_pending_empty() {
        let queue = WorkQueue::new();
        let processed = queue.process_pending(10);
        assert_eq!(processed, 0);
    }

    // === Clear tests ===

    #[test]
    fn test_clear() {
        let queue = WorkQueue::new();
        for _ in 0..5 {
            queue.push(Task::new(|| {}));
        }

        let cleared = queue.clear();
        assert_eq!(cleared, 5);
        assert!(queue.is_empty());
    }

    // === Debug tests ===

    #[test]
    fn test_debug() {
        let queue = WorkQueue::with_capacity(100);
        queue.push(Task::new(|| {}));
        let debug_str = format!("{queue:?}");
        assert!(debug_str.contains("WorkQueue"));
        assert!(debug_str.contains("len"));
        assert!(debug_str.contains("capacity"));
    }

    // === Concurrent tests ===

    #[test]
    fn test_concurrent_push() {
        use std::thread;

        let queue = Arc::new(WorkQueue::with_capacity(1000));
        let mut handles = vec![];

        for _ in 0..10 {
            let queue_clone = Arc::clone(&queue);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    queue_clone.push(Task::new(|| {}));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have 500 tasks (10 threads * 50 tasks)
        assert_eq!(queue.len(), 500);
    }

    // === Send + Sync tests ===

    #[test]
    fn test_work_queue_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WorkQueue>();
    }
}
