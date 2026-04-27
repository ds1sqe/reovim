use {
    super::super::*,
    crate::arch::sync::Mutex,
    std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

// === Basic tests ===

#[test]
fn test_new() {
    let queue = WorkQueue::new();
    assert_eq!(queue.capacity(), WORK_QUEUE_DEFAULT_CAPACITY);
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
    let queue = WorkQueue::with_capacity(WORK_QUEUE_MAX_CAPACITY + 1000);
    assert_eq!(queue.capacity(), WORK_QUEUE_MAX_CAPACITY);
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
#[cfg_attr(coverage_nightly, coverage(off))]
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

// === Default impl ===

#[test]
fn test_work_queue_default() {
    let queue = WorkQueue::default();
    assert_eq!(queue.capacity(), WORK_QUEUE_DEFAULT_CAPACITY);
    assert!(queue.is_empty());
}

// === Clear returns count ===

#[test]
fn test_clear_returns_count() {
    let queue = WorkQueue::new();
    queue.push(Task::new(|| {}));
    queue.push(Task::new(|| {}));
    queue.push(Task::new(|| {}));

    let cleared = queue.clear();
    assert_eq!(cleared, 3);
    assert!(queue.is_empty());

    // Clear empty queue
    let cleared_empty = queue.clear();
    assert_eq!(cleared_empty, 0);
}

// === process_pending with limit larger than queue ===

#[test]
fn test_process_pending_limit_exceeds_queue() {
    let queue = WorkQueue::new();
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..3 {
        let counter_clone = Arc::clone(&counter);
        queue.push(Task::new(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }));
    }

    // Limit is 100 but only 3 tasks
    let processed = queue.process_pending(100);
    assert_eq!(processed, 3);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    assert!(queue.is_empty());
}

// === Coverage: Debug impl shows dropped count ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_debug_with_dropped() {
    let queue = WorkQueue::with_capacity(1);
    queue.push(Task::new(|| {}));
    queue.push(Task::new(|| {})); // Dropped
    let debug_str = format!("{queue:?}");
    assert!(debug_str.contains("dropped"));
    assert!(debug_str.contains('1')); // dropped count
}

// === Coverage: try_pop on empty returns None ===

#[test]
fn test_try_pop_empty_returns_none() {
    let queue = WorkQueue::new();
    assert!(queue.try_pop().is_none());
}

// === Coverage: process_pending with limit zero ===

#[test]
fn test_process_pending_zero_limit() {
    let queue = WorkQueue::new();
    queue.push(Task::new(|| {}));
    let processed = queue.process_pending(0);
    assert_eq!(processed, 0);
    assert_eq!(queue.len(), 1);
}

// === Coverage: with_capacity zero ===

#[test]
fn test_with_capacity_zero() {
    let queue = WorkQueue::with_capacity(0);
    assert_eq!(queue.capacity(), 0);
    // Cannot push anything
    assert!(!queue.push(Task::new(|| {})));
    assert_eq!(queue.dropped_count(), 1);
}

// === MC/DC: push() overflow branch (capacity == 1, second push drops) ===

#[test]
fn test_push_overflow_capacity_one() {
    // Capacity = 1: first push succeeds (len < capacity), second fails (len >= capacity).
    // This ensures the true branch of `if queue.len() >= self.capacity` is taken
    // with capacity=1 to isolate the single-condition MC/DC branch.
    let queue = WorkQueue::with_capacity(1);
    assert!(queue.push(Task::new(|| {}))); // len was 0 < 1, succeeds
    assert!(!queue.push(Task::new(|| {}))); // len is now 1 >= 1, drops
    assert_eq!(queue.dropped_count(), 1);
    assert_eq!(queue.len(), 1);
}

// === MC/DC: process_pending() empty-queue break branch ===

#[test]
fn test_process_pending_breaks_on_empty_queue() {
    // Exercises the `else { break }` arm of the `let Some(mut task) = self.try_pop()`
    // pattern when the queue is exhausted before the limit is reached.
    let queue = WorkQueue::with_capacity(2);
    queue.push(Task::new(|| {}));
    queue.push(Task::new(|| {}));

    // limit=100 but only 2 tasks: loop must break early via the else branch
    let processed = queue.process_pending(100);
    assert_eq!(processed, 2);
    assert!(queue.is_empty());
}
