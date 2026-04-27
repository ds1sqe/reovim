use {
    super::super::*,
    crate::ipc::{EventBus, EventScope},
    std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

// === Basic tests ===

#[test]
fn test_runtime_new() {
    let runtime = Runtime::new();
    assert_eq!(runtime.state(), RuntimeState::Booting);
    assert!(runtime.is_idle());
}

#[test]
fn test_runtime_with_config() {
    let config = RuntimeConfig {
        work_queue_capacity: 100,
        priority_queue_capacity: 50,
        batch_size: 8,
        max_timers: 64,
    };
    let runtime = Runtime::with_config(config);
    assert_eq!(runtime.state(), RuntimeState::Booting);
}

#[test]
fn test_runtime_boot() {
    let mut runtime = Runtime::new();
    assert_eq!(runtime.state(), RuntimeState::Booting);

    runtime.boot();
    assert_eq!(runtime.state(), RuntimeState::Running);
}

#[test]
#[should_panic(expected = "boot() called when not in Booting state")]
fn test_runtime_boot_twice_panics() {
    let mut runtime = Runtime::new();
    runtime.boot();
    runtime.boot(); // Should panic
}

// === Shutdown tests ===

#[test]
fn test_runtime_shutdown() {
    let mut runtime = Runtime::new();
    runtime.boot();
    assert_eq!(runtime.state(), RuntimeState::Running);

    runtime.shutdown();
    assert_eq!(runtime.state(), RuntimeState::Stopping);
}

#[test]
fn test_runtime_emergency_stop() {
    let mut runtime = Runtime::new();
    runtime.boot();

    runtime.emergency_stop();
    assert_eq!(runtime.state(), RuntimeState::Emergency);
}

#[test]
fn test_runtime_shutdown_via_command() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let sender = runtime.command_sender();
    sender.send(RuntimeCommand::Shutdown).unwrap();

    runtime.tick();
    assert_eq!(runtime.state(), RuntimeState::Stopping);
}

// === Task scheduling tests ===

#[test]
fn test_runtime_schedule_work() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = Arc::clone(&executed);

    assert!(runtime.schedule_work(move || {
        executed_clone.store(true, Ordering::SeqCst);
    }));

    runtime.tick();
    assert!(executed.load(Ordering::SeqCst));
}

#[test]
fn test_runtime_schedule_work_not_running() {
    let runtime = Runtime::new(); // Still booting
    assert!(!runtime.schedule_work(|| {}));
}

#[test]
fn test_runtime_schedule_task() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = Arc::clone(&executed);

    let task = Task::new(move || {
        executed_clone.store(true, Ordering::SeqCst);
    });

    assert!(runtime.schedule_task(task));
    runtime.tick();
    assert!(executed.load(Ordering::SeqCst));
}

#[test]
fn test_runtime_schedule_work_with_priority() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let order = Arc::new(std::sync::Mutex::new(Vec::new()));

    let order1 = Arc::clone(&order);
    runtime.schedule_work_with_priority(Priority::LOW, move || {
        order1.lock().unwrap().push("low");
    });

    let order2 = Arc::clone(&order);
    runtime.schedule_work_with_priority(Priority::HIGH, move || {
        order2.lock().unwrap().push("high");
    });

    // Note: WorkQueue is FIFO, not priority-ordered
    // Priority only matters for tasks in PriorityQueue
    runtime.tick();
    runtime.tick();

    let result = order.lock().unwrap().clone();
    // Tasks are executed in FIFO order from WorkQueue
    assert_eq!(result.len(), 2);
}

// === Render pending tests ===

#[test]
fn test_runtime_render_pending() {
    let mut runtime = Runtime::new();

    assert!(!runtime.is_render_pending());

    runtime.request_render();
    assert!(runtime.is_render_pending());

    assert!(runtime.take_render_pending());
    assert!(!runtime.is_render_pending());
}

// === Scope tests ===

#[test]
fn test_runtime_scope() {
    let mut runtime = Runtime::new();
    assert!(runtime.current_scope().is_none());

    let scope = EventScope::new();
    runtime.set_scope(scope);
    assert!(runtime.current_scope().is_some());

    runtime.clear_scope();
    assert!(runtime.current_scope().is_none());
}

// === Idle tests ===

#[test]
fn test_runtime_is_idle() {
    let mut runtime = Runtime::new();
    runtime.boot();
    assert!(runtime.is_idle());

    runtime.schedule_work(|| {});
    assert!(!runtime.is_idle());

    runtime.tick();
    assert!(runtime.is_idle());
}

// === Stats tests ===

#[test]
fn test_runtime_stats() {
    let mut runtime = Runtime::new();
    runtime.boot();

    runtime.schedule_work(|| {});
    runtime.schedule_work(|| panic!("intentional panic"));

    let stats = runtime.stats();
    assert_eq!(stats.state, RuntimeState::Running);
    assert_eq!(stats.work_queue_len, 2);
    assert_eq!(stats.tasks_executed, 0);

    runtime.tick();

    let stats = runtime.stats();
    assert_eq!(stats.work_queue_len, 0);
    // One succeeded, one failed
    assert_eq!(stats.tasks_executed, 1);
    assert_eq!(stats.tasks_failed, 1);
}

// === Tick behavior tests ===

#[test]
fn test_runtime_tick_returns_false_on_stop() {
    let mut runtime = Runtime::new();
    runtime.boot();

    assert!(runtime.tick());

    runtime.shutdown();
    assert!(!runtime.tick());
}

#[test]
fn test_runtime_tick_drains_on_shutdown() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..5 {
        let counter_clone = Arc::clone(&counter);
        runtime.schedule_work(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
    }

    runtime.shutdown();
    runtime.tick(); // Should drain all tasks

    assert_eq!(counter.load(Ordering::SeqCst), 5);
}

// === Run tests ===

#[test]
fn test_runtime_run() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    // Schedule work that will trigger shutdown
    runtime.schedule_work(move || {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    // Send shutdown command
    let sender = runtime.command_sender();
    sender.send(RuntimeCommand::Shutdown).unwrap();

    runtime.run();

    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert!(runtime.state().is_shutting_down());
}

// === Debug tests ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_runtime_debug() {
    let runtime = Runtime::new();
    let debug_str = format!("{runtime:?}");
    assert!(debug_str.contains("Runtime"));
    assert!(debug_str.contains("Booting"));
}

// === Event bus integration ===

#[test]
fn test_runtime_with_event_bus() {
    let bus = Arc::new(EventBus::new());
    let bus_clone = Arc::clone(&bus);

    let runtime = Runtime::new().with_event_bus(bus);

    // Both should point to the same EventBus
    assert!(Arc::ptr_eq(runtime.event_bus(), &bus_clone));
}

// === Command channel tests ===

#[test]
fn test_runtime_command_schedule_task() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = Arc::clone(&executed);

    let task = Task::new(move || {
        executed_clone.store(true, Ordering::SeqCst);
    });

    let sender = runtime.command_sender();
    sender.send(RuntimeCommand::ScheduleTask(task)).unwrap();

    runtime.tick();
    runtime.tick(); // Need second tick to execute the scheduled task

    assert!(executed.load(Ordering::SeqCst));
}

// === Send test ===

#[test]
fn test_runtime_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Runtime>();
}

// === schedule_delayed ===

#[test]
fn test_runtime_schedule_delayed() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = runtime.schedule_delayed(std::time::Duration::ZERO, move || {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });
    assert!(!handle.is_failed());

    // Tick to fire timer and schedule work, then execute
    runtime.tick();
    runtime.tick();

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

// === schedule_periodic ===

#[test]
fn test_runtime_schedule_periodic() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = runtime.schedule_periodic(std::time::Duration::ZERO, move || {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });
    let _ = handle.detach();

    // Tick multiple times
    for _ in 0..4 {
        runtime.tick();
    }

    assert!(counter.load(Ordering::SeqCst) >= 1);
}

// === cancel_timer ===

#[test]
fn test_runtime_cancel_timer() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let handle = runtime.schedule_delayed(std::time::Duration::from_mins(1), || {});
    let id = handle.detach();
    assert!(runtime.cancel_timer(id));
}

// === timer_wheel() ===

#[test]
fn test_runtime_timer_wheel() {
    let runtime = Runtime::new();
    let wheel = runtime.timer_wheel();
    assert_eq!(wheel.pending_count(), 0);
}

// === queue_event ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_runtime_queue_event() {
    use crate::ipc::{DynEvent, Event};

    #[derive(Debug)]
    struct TestEvent;
    impl Event for TestEvent {}

    let mut runtime = Runtime::new();
    runtime.boot();

    let event = DynEvent::new(TestEvent);
    assert!(runtime.queue_event(event));
}

// === Emergency command ===

#[test]
fn test_runtime_emergency_via_command() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let sender = runtime.command_sender();
    sender.send(RuntimeCommand::Emergency).unwrap();

    runtime.tick();
    assert_eq!(runtime.state(), RuntimeState::Emergency);
}

// === schedule when not accepting work ===

#[test]
fn test_runtime_schedule_work_when_stopping() {
    let mut runtime = Runtime::new();
    runtime.boot();
    runtime.shutdown();

    // Stopping state should not accept new work
    assert!(!runtime.schedule_work(|| {}));
}

#[test]
fn test_runtime_schedule_task_not_running() {
    let runtime = Runtime::new(); // Booting state
    let task = Task::new(|| {});
    assert!(!runtime.schedule_task(task));
}

// === Default ===

#[test]
fn test_runtime_default() {
    let runtime = Runtime::default();
    assert_eq!(runtime.state(), RuntimeState::Booting);
}

// === work_queue accessor ===

#[test]
fn test_runtime_work_queue() {
    let runtime = Runtime::new();
    assert!(runtime.work_queue().is_empty());
}

// === schedule_work_with_priority when not running ===

#[test]
fn test_runtime_schedule_work_with_priority_not_running() {
    let runtime = Runtime::new(); // Booting state
    assert!(!runtime.schedule_work_with_priority(Priority::HIGH, || {}));
}

// === Coverage: ScheduleTask via command when not accepting work ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_runtime_command_schedule_task_when_stopping() {
    let mut runtime = Runtime::new();
    runtime.boot();

    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = Arc::clone(&executed);

    let sender = runtime.command_sender();
    sender.send(RuntimeCommand::Shutdown).unwrap();
    sender
        .send(RuntimeCommand::ScheduleTask(Task::new(move || {
            executed_clone.store(true, Ordering::SeqCst);
        })))
        .unwrap();

    runtime.tick();
    assert_eq!(runtime.state(), RuntimeState::Stopping);
}

// === Coverage: shutdown when not running (from Booting) ===

#[test]
fn test_runtime_shutdown_from_booting() {
    let mut runtime = Runtime::new();
    runtime.shutdown();
    assert_eq!(runtime.state(), RuntimeState::Booting);
}

// === Coverage: emergency_stop from any state ===

#[test]
fn test_runtime_emergency_stop_from_booting() {
    let mut runtime = Runtime::new();
    runtime.emergency_stop();
    assert_eq!(runtime.state(), RuntimeState::Emergency);
}

// === Coverage: dispatch_events with scope ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_runtime_dispatch_events_with_scope() {
    use crate::ipc::{DynEvent, Event, EventScope};

    #[derive(Debug)]
    struct ScopeTestEvent;
    impl Event for ScopeTestEvent {}

    let mut runtime = Runtime::new();
    runtime.boot();

    let scope = EventScope::new();
    scope.increment();
    scope.increment();
    runtime.set_scope(scope.clone());

    let event = DynEvent::new(ScopeTestEvent);
    runtime.queue_event(event);

    runtime.tick();
    assert!(scope.in_flight() < 2);
}

// === Coverage: RuntimeConfig Debug/Clone ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_runtime_config_debug_clone() {
    let config = RuntimeConfig::default();
    let cloned = config;
    assert_eq!(cloned.batch_size, DEFAULT_BATCH_SIZE);
    let debug = format!("{config:?}");
    assert!(debug.contains("RuntimeConfig"));
}

// === Coverage: RuntimeCommand Debug ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_runtime_command_debug() {
    let cmd = RuntimeCommand::Shutdown;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("Shutdown"));
}

// === Coverage: RuntimeStats Default ===

#[test]
fn test_runtime_stats_default() {
    let stats = RuntimeStats::default();
    assert_eq!(stats.state, RuntimeState::Booting);
    assert_eq!(stats.tasks_executed, 0);
}

// === Coverage: tick with Emergency state ===

#[test]
fn test_runtime_tick_emergency_returns_false() {
    let mut runtime = Runtime::new();
    runtime.boot();
    runtime.emergency_stop();
    assert!(!runtime.tick());
}

// === MC/DC: schedule_work() when state is Emergency (not accepting work) ===

#[test]
fn test_runtime_schedule_work_when_emergency() {
    // Emergency state: can_accept_work() == false -> return false.
    // This exercises the `!self.state.can_accept_work()` true branch of
    // schedule_work() from a different non-accepting state than Booting.
    let mut runtime = Runtime::new();
    runtime.boot();
    runtime.emergency_stop();
    assert_eq!(runtime.state(), RuntimeState::Emergency);
    assert!(!runtime.schedule_work(|| {}));
}

// === MC/DC: schedule_work_with_priority() when state is Emergency ===

#[test]
fn test_runtime_schedule_work_with_priority_when_emergency() {
    // Emergency state: can_accept_work() == false -> return false.
    // Covers the rejection branch of schedule_work_with_priority() from
    // a state distinct from Booting (already tested) to satisfy MC/DC.
    let mut runtime = Runtime::new();
    runtime.boot();
    runtime.emergency_stop();
    assert!(!runtime.schedule_work_with_priority(Priority::HIGH, || {}));
}

// === MC/DC: schedule_task() when state is Emergency ===

#[test]
fn test_runtime_schedule_task_when_emergency() {
    // Emergency state: can_accept_work() == false -> return false.
    // Covers the rejection branch of schedule_task() from Emergency state.
    let mut runtime = Runtime::new();
    runtime.boot();
    runtime.emergency_stop();
    let task = Task::new(|| {});
    assert!(!runtime.schedule_task(task));
}

// === MC/DC: shutdown() when state is already Stopping (no-op) ===

#[test]
fn test_runtime_shutdown_when_already_stopping() {
    // state == Stopping: the `if self.state == RuntimeState::Running` condition
    // is false -> no-op. State remains Stopping.
    // This provides a distinct false-branch case from test_runtime_shutdown_from_booting.
    let mut runtime = Runtime::new();
    runtime.boot();
    runtime.shutdown(); // Running -> Stopping
    assert_eq!(runtime.state(), RuntimeState::Stopping);

    // Second shutdown call: state is Stopping, not Running -> no state change
    runtime.shutdown();
    assert_eq!(runtime.state(), RuntimeState::Stopping);
}
