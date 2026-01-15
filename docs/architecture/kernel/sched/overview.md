# sched/ - Scheduler

Event loop and async task management.

## Source Location

`lib/kernel/src/sched/`

## Key Types

```rust
pub struct Runtime {
    event_rx: Receiver<Event>,
    work_queue: WorkQueue,
}

pub struct WorkQueue {
    tasks: VecDeque<Task>,
}
```

## Runtime

Main event loop:

```rust
loop {
    // 1. Process events
    while let Ok(event) = event_rx.try_recv() {
        handle_event(event);
    }

    // 2. Run scheduled tasks
    work_queue.run_pending();

    // 3. Render if needed
    if needs_render {
        render();
    }
}
```

## WorkQueue

Deferred task execution:

```rust
work_queue.schedule(|| {
    // Run later in event loop
});

work_queue.schedule_delayed(Duration::from_ms(100), || {
    // Run after delay
});
```

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [Concurrency Reference](../../contributing/internals/concurrency.md) - Tokio patterns
