use {
    super::super::{
        timer::{TimerEntry, TimerWork},
        *,
    },
    std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    },
};

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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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

// === is_pending ===

#[test]
fn test_timer_wheel_is_pending() {
    let wheel = Arc::new(TimerWheel::new());

    let handle = wheel.schedule_oneshot(Duration::from_secs(10), Priority::NORMAL, || {});
    assert!(wheel.is_pending(handle.id()));

    // Cancel the timer
    wheel.cancel(handle.id());
    assert!(!wheel.is_pending(handle.id()));
}

#[test]
fn test_timer_wheel_is_pending_nonexistent() {
    let wheel = Arc::new(TimerWheel::new());
    assert!(!wheel.is_pending(TimerId::from_raw(99999)));
}

// === Debug impls ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_timer_wheel_debug() {
    let wheel = Arc::new(TimerWheel::new());
    let debug = format!("{wheel:?}");
    assert!(debug.contains("TimerWheel"));
    assert!(debug.contains("pending"));
    assert!(debug.contains("max_timers"));
}

#[test]
fn test_timer_wheel_default() {
    let wheel = TimerWheel::default();
    assert_eq!(wheel.pending_count(), 0);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_timer_entry_debug() {
    let entry = TimerEntry {
        id: TimerId::from_raw(42),
        deadline: Instant::now(),
        interval: Some(Duration::from_millis(100)),
        priority: Priority::HIGH,
        work: TimerWork::Periodic(Arc::new(|| {})),
        cancelled: AtomicBool::new(false),
    };
    let debug = format!("{entry:?}");
    assert!(debug.contains("TimerEntry"));
    assert!(debug.contains("42"));
}

// === handle drop with live wheel ===

#[test]
fn test_timer_handle_drop_with_dead_wheel() {
    let id;
    {
        let wheel = Arc::new(TimerWheel::new());
        let handle = wheel.schedule_oneshot(Duration::from_mins(1), Priority::NORMAL, || {});
        id = handle.id();
        // wheel is dropped here while handle still has weak ref
        drop(wheel);
        // handle drops here - should not panic even though wheel is gone
    }
    let _ = id; // Suppress unused
}

// === timer not yet expired ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_timer_not_expired_on_tick() {
    let wheel = Arc::new(TimerWheel::new());
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let handle = wheel.schedule_oneshot(Duration::from_mins(1), Priority::NORMAL, move || {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });
    let _ = handle.detach();

    // Tick with current time - timer hasn't expired
    let tasks = wheel.tick(Instant::now());
    assert!(tasks.is_empty());
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    assert_eq!(wheel.pending_count(), 1);
}

// === Coverage: TimerConfig Clone/Debug ===

#[test]
fn test_timer_config_clone() {
    let config = TimerConfig {
        delay: Duration::from_millis(50),
        interval: Some(Duration::from_millis(100)),
        priority: Priority::HIGH,
    };
    #[allow(clippy::redundant_clone)]
    let cloned = config.clone();
    assert_eq!(cloned.delay, Duration::from_millis(50));
    assert_eq!(cloned.interval, Some(Duration::from_millis(100)));
    assert_eq!(cloned.priority, Priority::HIGH);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_timer_config_debug() {
    let config = TimerConfig::default();
    let debug = format!("{config:?}");
    assert!(debug.contains("TimerConfig"));
    assert!(debug.contains("delay"));
    assert!(debug.contains("interval"));
}

// === Coverage: TimerHandle debug with alive wheel ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_timer_handle_debug_with_alive_wheel() {
    let wheel = Arc::new(TimerWheel::new());
    let handle = wheel.schedule_oneshot(Duration::from_secs(10), Priority::NORMAL, || {});
    let debug = format!("{handle:?}");
    assert!(debug.contains("TimerHandle"));
    assert!(debug.contains("wheel_alive: true"));
    assert!(debug.contains("failed: false"));
}

// === Coverage: tick with already-taken OneShot (work is None) ===

#[test]
fn test_timer_tick_oneshot_already_taken() {
    let wheel = Arc::new(TimerWheel::new());

    // Schedule a one-shot that fires immediately
    let handle = wheel.schedule_oneshot(Duration::ZERO, Priority::NORMAL, || {});
    let _ = handle.detach();

    // First tick takes the work
    let tasks = wheel.tick(Instant::now() + Duration::from_millis(1));
    assert_eq!(tasks.len(), 1);

    // Timer should be removed after first tick
    assert_eq!(wheel.pending_count(), 0);
}

// === Coverage: periodic timer scheduled with different interval ===

#[test]
fn test_timer_periodic_with_nonzero_interval() {
    let wheel = Arc::new(TimerWheel::new());
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let handle =
        wheel.schedule_periodic(Duration::from_millis(100), Priority::HIGH, move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
    let _ = handle.detach();

    // Schedule fires at now + 100ms, tick at now + 200ms fires it
    let mut tasks = wheel.tick(Instant::now() + Duration::from_millis(200));
    assert_eq!(tasks.len(), 1);
    let _ = tasks[0].execute();
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Should still be pending after periodic fire
    assert_eq!(wheel.pending_count(), 1);
}

// === Coverage: max_timers exceeded for periodic timer ===

#[test]
fn test_timer_max_timers_exceeded_periodic() {
    let wheel = Arc::new(TimerWheel::with_max_timers(1));

    let _h1 = wheel.schedule_oneshot(Duration::from_secs(1), Priority::NORMAL, || {});

    // Second timer (periodic) should fail
    let h2 = wheel.schedule_periodic(Duration::from_secs(1), Priority::NORMAL, || {});
    assert!(h2.is_failed());
    assert_eq!(wheel.dropped_count(), 1);
}

// === Coverage: cancel nonexistent timer ===

#[test]
fn test_timer_cancel_nonexistent() {
    let wheel = Arc::new(TimerWheel::new());
    assert!(!wheel.cancel(TimerId::from_raw(99999)));
}

// === MC/DC: schedule_internal capacity overflow with max_timers=1 ===

#[test]
fn test_timer_wheel_capacity_overflow_at_one() {
    // max_timers=1: first schedule succeeds (len < 1), second hits the
    // `if timers.len() >= self.max_timers` true branch and returns a failed handle.
    let wheel = Arc::new(TimerWheel::with_max_timers(1));

    let h1 = wheel.schedule_oneshot(Duration::from_secs(10), Priority::NORMAL, || {});
    assert!(!h1.is_failed());
    assert_eq!(wheel.pending_count(), 1);
    assert_eq!(wheel.dropped_count(), 0);

    // Second scheduling hits the overflow branch
    let h2 = wheel.schedule_oneshot(Duration::from_secs(10), Priority::NORMAL, || {});
    assert!(h2.is_failed());
    assert_eq!(wheel.dropped_count(), 1);
    assert_eq!(wheel.pending_count(), 1); // Still only 1 timer
}

// === MC/DC: cancel() already-cancelled timer (is_cancelled() true branch) ===

#[test]
fn test_timer_cancel_already_cancelled_returns_false() {
    // Cancel a timer once (succeeds, is_cancelled() was false -> sets to true).
    // Cancel the same timer again: is_cancelled() is now true -> returns false.
    // This exercises the true branch of `if entry.is_cancelled()` in cancel().
    let wheel = Arc::new(TimerWheel::new());

    let handle = wheel.schedule_oneshot(Duration::from_secs(10), Priority::NORMAL, || {});
    let id = handle.id();

    // First cancel: is_cancelled() == false -> cancel and return true
    let first = wheel.cancel(id);
    assert!(first, "first cancel should succeed");

    // Second cancel: is_cancelled() == true -> return false (already cancelled)
    let second = wheel.cancel(id);
    assert!(!second, "second cancel should return false (already cancelled)");
}
