use {
    super::super::{executor::DEFAULT_BATCH_SIZE, *},
    std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
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
