//! Synchronous task executor.
//!
//! Linux equivalent: Core scheduler in `kernel/sched/core.c`
//!
//! This module provides an executor for running tasks synchronously with
//! panic handling. Tasks that panic are caught and marked as failed rather
//! than crashing the entire runtime.
//!
//! # Panic Safety
//!
//! The executor wraps task execution in `catch_unwind`, ensuring that a
//! panicking task doesn't bring down the entire editor. Failed tasks are
//! marked as such and counted in statistics.
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//! use std::sync::Arc;
//!
//! let queue = Arc::new(WorkQueue::new());
//! let mut executor = Executor::new(Arc::clone(&queue));
//!
//! // Schedule some tasks
//! queue.push(Task::new(|| println!("Task 1")));
//! queue.push(Task::new(|| println!("Task 2")));
//!
//! // Process tasks
//! let processed = executor.tick();
//! assert_eq!(processed, 2);
//! ```

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use super::{task::Task, work_queue::WorkQueue};

/// Default batch size for processing.
const DEFAULT_BATCH_SIZE: usize = 16;

/// Task executor for synchronous task processing.
///
/// Provides a simple run-to-completion model where tasks are executed
/// one at a time. Panics are caught to prevent runtime crashes.
pub struct Executor {
    /// Work queue for pending tasks.
    work_queue: Arc<WorkQueue>,

    /// Maximum tasks to process per tick.
    batch_size: usize,

    /// Total tasks successfully executed.
    executed_count: u64,

    /// Total tasks that failed (error or panic).
    failed_count: u64,
}

impl Executor {
    /// Create a new executor with a shared work queue.
    #[must_use]
    pub const fn new(work_queue: Arc<WorkQueue>) -> Self {
        Self {
            work_queue,
            batch_size: DEFAULT_BATCH_SIZE,
            executed_count: 0,
            failed_count: 0,
        }
    }

    /// Set the batch size for processing.
    ///
    /// The batch size limits how many tasks are processed per `tick()` call.
    #[must_use]
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size.max(1);
        self
    }

    /// Process one tick (up to `batch_size` tasks).
    ///
    /// Returns the total number of tasks processed (successful + failed).
    pub fn tick(&mut self) -> usize {
        let mut processed = 0;

        while processed < self.batch_size {
            let Some(mut task) = self.work_queue.try_pop() else {
                break;
            };

            if Self::execute_task(&mut task) {
                self.executed_count += 1;
            } else {
                self.failed_count += 1;
            }
            processed += 1;
        }

        processed
    }

    /// Execute a single task with panic handling.
    ///
    /// Returns `true` if the task completed successfully, `false` if it
    /// failed or panicked.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn execute_task(task: &mut Task) -> bool {
        // Wrap in catch_unwind to handle panics
        let result = catch_unwind(AssertUnwindSafe(|| task.execute()));

        match result {
            Ok(Ok(())) => true,
            Ok(Err(_)) => {
                // Task returned an error
                task.mark_failed();
                false
            }
            Err(_panic) => {
                // Task panicked
                task.mark_failed();
                false
            }
        }
    }

    /// Schedule a task for execution.
    ///
    /// Returns `false` if the queue is full.
    #[must_use]
    pub fn schedule(&self, task: Task) -> bool {
        self.work_queue.push(task)
    }

    /// Schedule work using a closure.
    ///
    /// Convenience method that creates a task with normal priority.
    pub fn spawn<F>(&self, work: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        self.schedule(Task::new(work))
    }

    /// Get the total number of successfully executed tasks.
    #[inline]
    #[must_use]
    pub const fn executed_count(&self) -> u64 {
        self.executed_count
    }

    /// Get the total number of failed tasks.
    #[inline]
    #[must_use]
    pub const fn failed_count(&self) -> u64 {
        self.failed_count
    }

    /// Check if there's pending work.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        !self.work_queue.is_empty()
    }

    /// Get the current batch size.
    #[inline]
    #[must_use]
    pub const fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Drain all pending tasks, executing each one.
    ///
    /// Returns the total number of tasks processed.
    pub fn drain(&mut self) -> usize {
        let mut total = 0;
        while self.has_pending() {
            total += self.tick();
        }
        total
    }

    /// Reset execution statistics.
    pub const fn reset_stats(&mut self) {
        self.executed_count = 0;
        self.failed_count = 0;
    }
}

impl std::fmt::Debug for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Executor")
            .field("batch_size", &self.batch_size)
            .field("executed_count", &self.executed_count)
            .field("failed_count", &self.failed_count)
            .field("pending", &self.work_queue.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    fn make_executor() -> Executor {
        Executor::new(Arc::new(WorkQueue::new()))
    }

    // === Basic tests ===

    #[test]
    fn test_new() {
        let executor = make_executor();
        assert_eq!(executor.batch_size(), DEFAULT_BATCH_SIZE);
        assert_eq!(executor.executed_count(), 0);
        assert_eq!(executor.failed_count(), 0);
        assert!(!executor.has_pending());
    }

    #[test]
    fn test_with_batch_size() {
        let queue = Arc::new(WorkQueue::new());
        let executor = Executor::new(queue).with_batch_size(5);
        assert_eq!(executor.batch_size(), 5);
    }

    #[test]
    fn test_batch_size_minimum() {
        let queue = Arc::new(WorkQueue::new());
        let executor = Executor::new(queue).with_batch_size(0);
        assert_eq!(executor.batch_size(), 1); // Clamped to 1
    }

    // === Execution tests ===

    #[test]
    fn test_tick() {
        let queue = Arc::new(WorkQueue::new());
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..5 {
            let counter_clone = Arc::clone(&counter);
            queue.push(Task::new(move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }));
        }

        let mut executor = Executor::new(queue);
        let processed = executor.tick();

        assert_eq!(processed, 5);
        assert_eq!(counter.load(Ordering::SeqCst), 5);
        assert_eq!(executor.executed_count(), 5);
        assert_eq!(executor.failed_count(), 0);
    }

    #[test]
    fn test_tick_batch_limit() {
        let queue = Arc::new(WorkQueue::new());
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..20 {
            let counter_clone = Arc::clone(&counter);
            queue.push(Task::new(move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }));
        }

        let mut executor = Executor::new(Arc::clone(&queue)).with_batch_size(5);

        // First tick processes 5
        let processed = executor.tick();
        assert_eq!(processed, 5);
        assert_eq!(counter.load(Ordering::SeqCst), 5);
        assert_eq!(queue.len(), 15);

        // Second tick processes 5 more
        let processed = executor.tick();
        assert_eq!(processed, 5);
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_spawn() {
        let queue = Arc::new(WorkQueue::new());
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = Arc::clone(&executed);

        let mut executor = Executor::new(Arc::clone(&queue));
        assert!(executor.spawn(move || {
            executed_clone.store(true, Ordering::SeqCst);
        }));

        executor.tick();
        assert!(executed.load(Ordering::SeqCst));
    }

    // === Panic handling tests ===

    #[test]
    fn test_panic_handling() {
        let queue = Arc::new(WorkQueue::new());
        let before_panic = Arc::new(AtomicBool::new(false));
        let after_panic = Arc::new(AtomicBool::new(false));

        let before_clone = Arc::clone(&before_panic);
        let after_clone = Arc::clone(&after_panic);

        // Task that runs before panic
        queue.push(Task::new(move || {
            before_clone.store(true, Ordering::SeqCst);
        }));

        // Task that panics
        queue.push(Task::new(|| {
            panic!("intentional panic for testing");
        }));

        // Task that runs after panic
        queue.push(Task::new(move || {
            after_clone.store(true, Ordering::SeqCst);
        }));

        let mut executor = Executor::new(queue);
        let processed = executor.tick();

        // All three tasks should have been processed
        assert_eq!(processed, 3);
        assert!(before_panic.load(Ordering::SeqCst));
        assert!(after_panic.load(Ordering::SeqCst));

        // One failed, two succeeded
        assert_eq!(executor.executed_count(), 2);
        assert_eq!(executor.failed_count(), 1);
    }

    #[test]
    fn test_execute_task_success() {
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = Arc::clone(&executed);

        let mut task = Task::new(move || {
            executed_clone.store(true, Ordering::SeqCst);
        });

        assert!(Executor::execute_task(&mut task));
        assert!(executed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_execute_task_panic() {
        let mut task = Task::new(|| {
            panic!("test panic");
        });

        assert!(!Executor::execute_task(&mut task));
    }

    // === Drain tests ===

    #[test]
    fn test_drain() {
        let queue = Arc::new(WorkQueue::new());
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..50 {
            let counter_clone = Arc::clone(&counter);
            queue.push(Task::new(move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }));
        }

        let mut executor = Executor::new(queue).with_batch_size(10);
        let total = executor.drain();

        assert_eq!(total, 50);
        assert_eq!(counter.load(Ordering::SeqCst), 50);
        assert!(!executor.has_pending());
    }

    // === Stats tests ===

    #[test]
    fn test_reset_stats() {
        let queue = Arc::new(WorkQueue::new());
        queue.push(Task::new(|| {}));

        let mut executor = Executor::new(queue);
        executor.tick();
        assert_eq!(executor.executed_count(), 1);

        executor.reset_stats();
        assert_eq!(executor.executed_count(), 0);
        assert_eq!(executor.failed_count(), 0);
    }

    // === Debug tests ===

    #[test]
    fn test_debug() {
        let executor = make_executor();
        let debug_str = format!("{executor:?}");
        assert!(debug_str.contains("Executor"));
        assert!(debug_str.contains("batch_size"));
        assert!(debug_str.contains("executed_count"));
    }
}
