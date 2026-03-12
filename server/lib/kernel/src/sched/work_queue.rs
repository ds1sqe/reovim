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

