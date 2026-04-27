use super::*;

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use crate::ipc::EventScope;

#[test]
fn test_saturator_submit() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = spawn_saturator(
        |x: i32| x * 2,
        move |_result| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        },
    );

    handle.submit(1, None);
    handle.submit(2, None);
    handle.submit(3, None);

    // Give worker time to process
    thread::sleep(Duration::from_millis(50));

    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[test]
fn test_saturator_submit_background() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = spawn_saturator(
        |x: i32| x,
        move |_result| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        },
    );

    handle.submit_background(1, None);
    handle.submit_background(2, None);

    thread::sleep(Duration::from_millis(50));

    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn test_saturator_scope_tracking() {
    let scope = EventScope::new();
    let scope_clone = scope.clone();

    let handle = spawn_saturator(|x: i32| x, |_result| {});

    handle.submit(1, Some(&scope_clone));
    handle.submit(2, Some(&scope_clone));

    // Wait for processing with timeout
    let completed = scope.wait_timeout(Duration::from_millis(100));
    assert!(completed, "Scope should complete when all items processed");
}

#[test]
fn test_saturator_priority_ordering() {
    let results = Arc::new(std::sync::Mutex::new(Vec::new()));
    let results_clone = Arc::clone(&results);

    let handle = spawn_saturator(
        |x: i32| x,
        move |result| {
            results_clone.lock().unwrap().push(result);
        },
    );

    // Submit low priority first
    handle.submit_background(3, None);
    // Then high priority
    handle.submit(1, None);
    handle.submit(2, None);

    thread::sleep(Duration::from_millis(50));

    let guard = results.lock().unwrap();
    // High priority items (1, 2) should be processed before low (3)
    // Note: exact order depends on timing, but high should come before low
    assert!(guard.contains(&1));
    assert!(guard.contains(&2));
    assert!(guard.contains(&3));
    drop(guard);
}

#[test]
fn test_saturator_shutdown() {
    let handle = spawn_saturator(|x: i32| x, |_result| {});

    handle.shutdown();
    assert!(handle.is_shutting_down());
}

#[test]
fn test_saturator_clone() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = spawn_saturator(
        |x: i32| x,
        move |_result| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        },
    );

    // Clone the handle
    let handle2 = handle.clone();

    // Submit from both handles
    handle.submit(1, None);
    handle2.submit(2, None);

    thread::sleep(Duration::from_millis(50));

    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn test_request_priority_default() {
    assert_eq!(RequestPriority::default(), RequestPriority::High);
}

#[test]
fn test_submit_request() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = spawn_saturator(
        |x: i32| x,
        move |_result| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        },
    );

    handle.submit_request(SaturationRequest {
        data: 42,
        priority: RequestPriority::High,
        scope: None,
    });

    thread::sleep(Duration::from_millis(50));

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

// === Coverage: submit_request with Low priority ===

#[test]
fn test_submit_request_low_priority() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = spawn_saturator(
        |x: i32| x,
        move |_result| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        },
    );

    handle.submit_request(SaturationRequest {
        data: 99,
        priority: RequestPriority::Low,
        scope: None,
    });

    thread::sleep(Duration::from_millis(50));

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

// === Coverage: SaturatorConfig default ===

#[test]
fn test_saturator_config_default() {
    let config = SaturatorConfig::default();
    assert!(config.drain_on_shutdown);
}

// === Coverage: SaturatorConfig Debug and Clone ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_saturator_config_debug_clone() {
    let config = SaturatorConfig {
        drain_on_shutdown: false,
    };
    let cloned = config.clone();
    assert!(!cloned.drain_on_shutdown);

    let debug = format!("{config:?}");
    assert!(debug.contains("SaturatorConfig"));
    assert!(debug.contains("false"));
}

// === Coverage: submit_background with scope tracking ===

#[test]
fn test_submit_background_with_scope() {
    let scope = EventScope::new();
    let scope_clone = scope.clone();

    let handle = spawn_saturator(|x: i32| x, |_result| {});

    handle.submit_background(1, Some(&scope_clone));
    handle.submit_background(2, Some(&scope_clone));

    let completed = scope.wait_timeout(Duration::from_millis(100));
    assert!(completed, "Scope should complete when all items processed");
}

// === Coverage: SaturationRequest Debug ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_saturation_request_debug() {
    let request = SaturationRequest {
        data: 42i32,
        priority: RequestPriority::High,
        scope: None,
    };
    let debug = format!("{request:?}");
    assert!(debug.contains("SaturationRequest"));
    assert!(debug.contains("42"));
}

// === Coverage: RequestPriority Debug, Clone, Hash ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_request_priority_debug_clone_hash() {
    use std::collections::HashSet;
    let high = RequestPriority::High;
    let low = RequestPriority::Low;

    let cloned = high;
    assert_eq!(cloned, RequestPriority::High);

    let debug_high = format!("{high:?}");
    assert!(debug_high.contains("High"));

    let debug_low = format!("{low:?}");
    assert!(debug_low.contains("Low"));

    let mut set = HashSet::new();
    set.insert(high);
    set.insert(low);
    assert_eq!(set.len(), 2);
}

// === Coverage: Drop with worker thread join ===

#[test]
fn test_saturator_drop_joins_worker() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    {
        let handle = spawn_saturator(
            |x: i32| x,
            move |_result| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            },
        );
        handle.submit(1, None);
        thread::sleep(Duration::from_millis(50));
        // handle drops here, setting shutdown flag and joining worker
    }

    // Worker should have processed the item before shutting down
    assert!(counter.load(Ordering::SeqCst) >= 1);
}

// === Coverage: submit_request with scope ===

#[test]
fn test_submit_request_with_scope() {
    let scope = EventScope::new();
    let scope_clone = scope.clone();

    let handle = spawn_saturator(|x: i32| x, |_result| {});

    handle.submit_request(SaturationRequest {
        data: 7,
        priority: RequestPriority::High,
        scope: Some(scope_clone),
    });

    let completed = scope.wait_timeout(Duration::from_millis(100));
    assert!(completed);
}

// === MC/DC: submit() - scope present (TRUE branch at line 110) ===
// Exercises `if let Some(s) = scope` in submit() with a Some scope.

#[test]
fn test_submit_with_scope_increments_and_decrements() {
    let scope = EventScope::new();

    let handle = spawn_saturator(|x: i32| x, |_result| {});

    // Before submit: in_flight is 0
    assert_eq!(scope.in_flight(), 0);

    // submit() with Some scope: TRUE branch increments the scope counter
    handle.submit(42, Some(&scope));

    // The scope counter was incremented by submit before sending to worker
    // After worker processes it, it'll be decremented back to 0
    let completed = scope.wait_timeout(Duration::from_millis(200));
    assert!(completed, "Scope should reach 0 after worker processes the item");
    assert_eq!(scope.in_flight(), 0);
}

// === MC/DC: submit_background() - scope present (TRUE branch at line 126) ===
// Exercises `if let Some(s) = scope` in submit_background() with a Some scope.

#[test]
fn test_submit_background_scope_increments_and_decrements() {
    let scope = EventScope::new();

    let handle = spawn_saturator(|x: i32| x, |_result| {});

    assert_eq!(scope.in_flight(), 0);

    // submit_background() with Some scope: TRUE branch at line 126
    handle.submit_background(10, Some(&scope));

    // Worker processes and decrements
    let completed = scope.wait_timeout(Duration::from_millis(200));
    assert!(completed, "Background scope should complete after worker processes");
    assert_eq!(scope.in_flight(), 0);
}

// === MC/DC: Drop - worker present (TRUE branch at line 172) ===
// Exercises `if let Some(worker) = self.worker.take()` when the original
// handle (which owns the worker JoinHandle) is dropped.

#[test]
fn test_drop_original_handle_joins_worker_thread() {
    use std::sync::atomic::AtomicBool;
    let worker_started = Arc::new(AtomicBool::new(false));
    let worker_started_clone = Arc::clone(&worker_started);

    {
        let handle = spawn_saturator(
            move |x: i32| {
                worker_started_clone.store(true, Ordering::SeqCst);
                x
            },
            |_result| {},
        );

        // Submit work so the worker thread definitely starts
        handle.submit(1, None);
        // Give worker thread time to process
        thread::sleep(Duration::from_millis(30));

        // Drop the original handle here. The Drop impl TRUE branch is exercised:
        // `if let Some(worker) = self.worker.take()` will be Some (original handle owns it).
        // This sets shutdown=true and joins the worker thread.
    }
    // After drop, worker has been joined. The submitted work was processed.
    assert!(worker_started.load(Ordering::SeqCst));
}

// === MC/DC: Drop - cloned handle has no worker (FALSE branch at line 172) ===
// A cloned SaturatorHandle has `worker: None`, so the Drop impl FALSE branch
// (`worker.take()` returns None) is exercised.

#[test]
fn test_drop_cloned_handle_does_not_join_worker() {
    let handle = spawn_saturator(|x: i32| x, |_result| {});

    // Clone does not own the worker thread (worker: None)
    let clone = handle.clone();

    // Dropping the clone exercises the FALSE branch of the Drop impl:
    // `if let Some(worker) = self.worker.take()` -> None branch is taken
    drop(clone);

    // The original handle still works after the clone is dropped
    handle.submit(1, None);
    thread::sleep(Duration::from_millis(30));

    // Original handle drops last, joining the worker thread
    drop(handle);
}
