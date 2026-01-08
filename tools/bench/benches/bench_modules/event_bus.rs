//! Event bus dispatch benchmarks
//!
//! Measures synchronous dispatch performance and DynEvent overhead.
//! These benchmarks help identify latency sources in the event system (Issue #86).

use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion};
use reovim_core::event_bus::{DynEvent, Event, EventBus, EventResult, HandlerContext};

/// Test event for benchmarking
#[derive(Debug)]
struct BenchEvent {
    value: u64,
}

impl Event for BenchEvent {}

/// Benchmark DynEvent creation (type erasure + Box allocation)
pub fn bench_dyn_event_creation(c: &mut Criterion) {
    c.bench_function("event_bus/dyn_event_new", |b| {
        b.iter(|| {
            let event = BenchEvent { value: 42 };
            black_box(DynEvent::new(event))
        });
    });
}

/// Benchmark dispatch with varying handler counts
///
/// Tests handler counts from 0 to 100 to understand scaling behavior.
/// Real-world usage typically has 5-30 handlers per event type.
pub fn bench_dispatch_handler_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/dispatch");

    for handler_count in [0, 1, 5, 10, 20, 50, 100] {
        // Setup: Create bus and register handlers
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        for _ in 0..handler_count {
            let counter_clone = Arc::clone(&counter);
            bus.subscribe::<BenchEvent, _>(100, move |event, _ctx| {
                counter_clone.fetch_add(event.value, Ordering::Relaxed);
                EventResult::Handled
            });
        }

        let sender = bus.sender();

        group.bench_with_input(
            BenchmarkId::new("handlers", handler_count),
            &handler_count,
            |b, _| {
                b.iter(|| {
                    let event = DynEvent::new(BenchEvent { value: 1 });
                    let mut ctx = HandlerContext::new(&sender);
                    black_box(bus.dispatch(&event, &mut ctx))
                });
            },
        );
    }

    group.finish();
}

/// Additional event types for multi-type benchmarks
#[derive(Debug)]
struct BenchEvent2 {
    value: u64,
}
impl Event for BenchEvent2 {}

#[derive(Debug)]
struct BenchEvent3 {
    value: u64,
}
impl Event for BenchEvent3 {}

/// Benchmark dispatch with multiple event types registered
///
/// Tests HashMap lookup performance when multiple event types are registered.
/// This simulates a realistic scenario where many plugins register handlers.
pub fn bench_dispatch_many_event_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/dispatch_multi_type");

    for event_type_count in [1, 5, 10, 20] {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        // Register handlers for the target event type
        let counter_clone = Arc::clone(&counter);
        bus.subscribe::<BenchEvent, _>(100, move |event, _ctx| {
            counter_clone.fetch_add(event.value, Ordering::Relaxed);
            EventResult::Handled
        });

        // Register handlers for other event types to fill the HashMap
        for i in 0..event_type_count {
            let counter_clone = Arc::clone(&counter);
            if i % 2 == 0 {
                bus.subscribe::<BenchEvent2, _>(100, move |event, _ctx| {
                    counter_clone.fetch_add(event.value, Ordering::Relaxed);
                    EventResult::Handled
                });
            } else {
                bus.subscribe::<BenchEvent3, _>(100, move |event, _ctx| {
                    counter_clone.fetch_add(event.value, Ordering::Relaxed);
                    EventResult::Handled
                });
            }
        }

        let sender = bus.sender();

        group.bench_with_input(
            BenchmarkId::new("event_types", event_type_count),
            &event_type_count,
            |b, _| {
                b.iter(|| {
                    let event = DynEvent::new(BenchEvent { value: 1 });
                    let mut ctx = HandlerContext::new(&sender);
                    black_box(bus.dispatch(&event, &mut ctx))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark handler registration (subscribe)
pub fn bench_handler_registration(c: &mut Criterion) {
    c.bench_function("event_bus/subscribe", |b| {
        b.iter(|| {
            let bus = EventBus::new(16);
            bus.subscribe::<BenchEvent, _>(100, |_, _| EventResult::Handled);
            black_box(bus)
        });
    });
}

/// Benchmark dispatch with handlers at different priority levels
///
/// Tests the overhead of priority sorting during registration and dispatch.
/// Handlers are registered with varying priorities to test sort stability.
pub fn bench_dispatch_priority_sorted(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/dispatch_priority");

    for handler_count in [5, 10, 20] {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        // Register handlers with varying priorities (not in order)
        for i in 0..handler_count {
            let counter_clone = Arc::clone(&counter);
            // Assign priorities in reverse order to test sorting
            let priority = (handler_count - i) as u32 * 10;
            bus.subscribe::<BenchEvent, _>(priority, move |event, _ctx| {
                counter_clone.fetch_add(event.value, Ordering::Relaxed);
                EventResult::Handled
            });
        }

        let sender = bus.sender();

        group.bench_with_input(
            BenchmarkId::new("handlers", handler_count),
            &handler_count,
            |b, _| {
                b.iter(|| {
                    let event = DynEvent::new(BenchEvent { value: 1 });
                    let mut ctx = HandlerContext::new(&sender);
                    black_box(bus.dispatch(&event, &mut ctx))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark dispatch where handlers emit additional events
///
/// Tests the latency when handlers trigger cascading events (common in plugins).
/// This measures the overhead of ctx.emit() during dispatch.
pub fn bench_dispatch_with_emit(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/dispatch_emit");

    for emit_count in [0, 1, 5, 10] {
        let bus = EventBus::new(256);
        let counter = Arc::new(AtomicU64::new(0));

        let counter_clone = Arc::clone(&counter);
        bus.subscribe::<BenchEvent, _>(100, move |event, ctx| {
            counter_clone.fetch_add(event.value, Ordering::Relaxed);
            // Emit additional events to test chaining overhead
            for _ in 0..emit_count {
                ctx.emit(BenchEvent2 { value: 1 });
            }
            EventResult::Handled
        });

        // Handler for the emitted events
        let counter_clone = Arc::clone(&counter);
        bus.subscribe::<BenchEvent2, _>(100, move |event, _ctx| {
            counter_clone.fetch_add(event.value, Ordering::Relaxed);
            EventResult::Handled
        });

        let sender = bus.sender();

        group.bench_with_input(
            BenchmarkId::new("emits", emit_count),
            &emit_count,
            |b, _| {
                b.iter(|| {
                    let event = DynEvent::new(BenchEvent { value: 1 });
                    let mut ctx = HandlerContext::new(&sender);
                    black_box(bus.dispatch(&event, &mut ctx))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark RwLock acquisition overhead during dispatch
///
/// This measures the read lock acquisition time which is relevant for
/// understanding contention issues in multi-threaded scenarios.
pub fn bench_rwlock_acquisition(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/rwlock");

    // Test with different numbers of registered event types
    for type_count in [1, 10, 50, 100] {
        let bus = EventBus::new(16);

        // Register handlers for many different event types to grow the HashMap
        for i in 0..type_count {
            if i % 3 == 0 {
                bus.subscribe::<BenchEvent, _>(100, |_, _| EventResult::Handled);
            } else if i % 3 == 1 {
                bus.subscribe::<BenchEvent2, _>(100, |_, _| EventResult::Handled);
            } else {
                bus.subscribe::<BenchEvent3, _>(100, |_, _| EventResult::Handled);
            }
        }

        let sender = bus.sender();

        group.bench_with_input(
            BenchmarkId::new("registered_handlers", type_count),
            &type_count,
            |b, _| {
                b.iter(|| {
                    // Dispatch to non-existent event type to measure pure lock overhead
                    let event = DynEvent::new(BenchEvent { value: 1 });
                    let mut ctx = HandlerContext::new(&sender);
                    black_box(bus.dispatch(&event, &mut ctx))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark consumed event (early exit)
///
/// Tests performance when a handler returns EventResult::Consumed
/// which should stop propagation to subsequent handlers.
pub fn bench_dispatch_consumed(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/dispatch_consumed");

    for handler_count in [5, 10, 20, 50] {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        // First handler consumes the event
        let counter_clone = Arc::clone(&counter);
        bus.subscribe::<BenchEvent, _>(50, move |event, _ctx| {
            counter_clone.fetch_add(event.value, Ordering::Relaxed);
            EventResult::Consumed // Early exit
        });

        // These handlers should never be called
        for _ in 1..handler_count {
            let counter_clone = Arc::clone(&counter);
            bus.subscribe::<BenchEvent, _>(100, move |event, _ctx| {
                counter_clone.fetch_add(event.value, Ordering::Relaxed);
                EventResult::Handled
            });
        }

        let sender = bus.sender();

        group.bench_with_input(
            BenchmarkId::new("total_handlers", handler_count),
            &handler_count,
            |b, _| {
                b.iter(|| {
                    let event = DynEvent::new(BenchEvent { value: 1 });
                    let mut ctx = HandlerContext::new(&sender);
                    black_box(bus.dispatch(&event, &mut ctx))
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// BOTTLENECK SIMULATION BENCHMARKS
// =============================================================================
// These benchmarks simulate the actual bottleneck scenario where a slow handler
// (like tree-sitter query compilation) blocks subsequent event dispatch.

/// Event type that represents a "fast" event (like auto-pair's RequestInsertText)
#[derive(Debug)]
struct FastEvent {
    value: u64,
}
impl Event for FastEvent {}

/// Event type that triggers slow processing (like RegisterLanguage)
#[derive(Debug)]
struct SlowEvent {
    work_duration_us: u64,
}
impl Event for SlowEvent {}

/// Simulates CPU-intensive work (like Query::new())
#[inline(never)]
fn simulate_cpu_work(duration_us: u64) {
    let start = std::time::Instant::now();
    let target = Duration::from_micros(duration_us);
    // Busy-wait to simulate CPU work (more accurate than sleep for short durations)
    while start.elapsed() < target {
        black_box(0u64.wrapping_add(1));
    }
}

/// Benchmark: Slow handler blocks fast event dispatch
///
/// This simulates the real bottleneck: a RegisterLanguage handler doing
/// query compilation blocks BufferModified events from being processed.
pub fn bench_blocking_handler(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/bottleneck_blocking");
    group.measurement_time(Duration::from_secs(3));

    // Test with different blocking durations (in microseconds)
    // Real tree-sitter: rust=26ms, c=2.8ms, etc.
    for block_us in [100, 1000, 5000, 10000] {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        // Slow handler that blocks (simulates query compilation)
        bus.subscribe::<SlowEvent, _>(50, move |event, _ctx| {
            simulate_cpu_work(event.work_duration_us);
            EventResult::Handled
        });

        // Fast handler (simulates auto-pair)
        let counter_clone = Arc::clone(&counter);
        bus.subscribe::<FastEvent, _>(100, move |event, _ctx| {
            counter_clone.fetch_add(event.value, Ordering::Relaxed);
            EventResult::Handled
        });

        let sender = bus.sender();

        group.bench_with_input(
            BenchmarkId::new("block_us", block_us),
            &block_us,
            |b, &block_us| {
                b.iter(|| {
                    // First dispatch the slow event (blocks the bus)
                    let slow = DynEvent::new(SlowEvent {
                        work_duration_us: block_us,
                    });
                    let mut ctx = HandlerContext::new(&sender);
                    bus.dispatch(&slow, &mut ctx);

                    // Then dispatch the fast event (should be fast after blocking)
                    let fast = DynEvent::new(FastEvent { value: 1 });
                    let mut ctx = HandlerContext::new(&sender);
                    black_box(bus.dispatch(&fast, &mut ctx))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Queue depth impact on latency
///
/// Simulates multiple events queued behind a slow handler.
/// This is what happens when user types while languages are registering.
pub fn bench_queue_behind_slow_handler(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/bottleneck_queue");
    group.measurement_time(Duration::from_secs(3));

    // Number of fast events queued behind slow event
    for queue_depth in [1, 5, 10, 20] {
        let bus = EventBus::new(256);
        let counter = Arc::new(AtomicU64::new(0));

        // Slow handler (simulates 1ms query compilation)
        bus.subscribe::<SlowEvent, _>(50, move |event, _ctx| {
            simulate_cpu_work(event.work_duration_us);
            EventResult::Handled
        });

        // Fast handler
        let counter_clone = Arc::clone(&counter);
        bus.subscribe::<FastEvent, _>(100, move |event, _ctx| {
            counter_clone.fetch_add(event.value, Ordering::Relaxed);
            EventResult::Handled
        });

        let sender = bus.sender();

        group.bench_with_input(
            BenchmarkId::new("queued_events", queue_depth),
            &queue_depth,
            |b, &queue_depth| {
                b.iter(|| {
                    // Dispatch slow event first
                    let slow = DynEvent::new(SlowEvent {
                        work_duration_us: 1000, // 1ms
                    });
                    let mut ctx = HandlerContext::new(&sender);
                    bus.dispatch(&slow, &mut ctx);

                    // Dispatch multiple fast events (queued behind slow)
                    for i in 0..queue_depth {
                        let fast = DynEvent::new(FastEvent { value: i as u64 });
                        let mut ctx = HandlerContext::new(&sender);
                        bus.dispatch(&fast, &mut ctx);
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Multiple slow handlers in sequence
///
/// Simulates what happens when multiple languages register at startup.
/// Each language's query compilation adds to the total blocking time.
pub fn bench_sequential_slow_handlers(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/bottleneck_sequential");
    group.measurement_time(Duration::from_secs(5));

    // Simulates registering N languages, each taking some time
    for (lang_count, per_lang_us) in [(1, 5000), (4, 2500), (8, 1000)] {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        // Slow handler
        bus.subscribe::<SlowEvent, _>(50, move |event, _ctx| {
            simulate_cpu_work(event.work_duration_us);
            EventResult::Handled
        });

        // Fast handler
        let counter_clone = Arc::clone(&counter);
        bus.subscribe::<FastEvent, _>(100, move |event, _ctx| {
            counter_clone.fetch_add(event.value, Ordering::Relaxed);
            EventResult::Handled
        });

        let sender = bus.sender();
        let label = format!("{}x{}us", lang_count, per_lang_us);

        group.bench_with_input(BenchmarkId::new("langs", label), &(), |b, _| {
            b.iter(|| {
                // Register N languages (each blocks)
                for _ in 0..lang_count {
                    let slow = DynEvent::new(SlowEvent {
                        work_duration_us: per_lang_us,
                    });
                    let mut ctx = HandlerContext::new(&sender);
                    bus.dispatch(&slow, &mut ctx);
                }

                // Then dispatch the fast event
                let fast = DynEvent::new(FastEvent { value: 1 });
                let mut ctx = HandlerContext::new(&sender);
                black_box(bus.dispatch(&fast, &mut ctx))
            });
        });
    }

    group.finish();
}

/// Benchmark: Fast event latency WITH vs WITHOUT blocking
///
/// Directly measures the latency difference when a slow handler is present.
/// This quantifies the impact of the bottleneck.
pub fn bench_latency_with_without_blocking(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_bus/bottleneck_comparison");

    // WITHOUT blocking - baseline fast event dispatch
    {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        let counter_clone = Arc::clone(&counter);
        bus.subscribe::<FastEvent, _>(100, move |event, _ctx| {
            counter_clone.fetch_add(event.value, Ordering::Relaxed);
            EventResult::Handled
        });

        let sender = bus.sender();

        group.bench_function("fast_only", |b| {
            b.iter(|| {
                let fast = DynEvent::new(FastEvent { value: 1 });
                let mut ctx = HandlerContext::new(&sender);
                black_box(bus.dispatch(&fast, &mut ctx))
            });
        });
    }

    // WITH blocking - slow handler registered (but not triggered)
    {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        // Register slow handler (exists but not called in this benchmark)
        bus.subscribe::<SlowEvent, _>(50, move |event, _ctx| {
            simulate_cpu_work(event.work_duration_us);
            EventResult::Handled
        });

        let counter_clone = Arc::clone(&counter);
        bus.subscribe::<FastEvent, _>(100, move |event, _ctx| {
            counter_clone.fetch_add(event.value, Ordering::Relaxed);
            EventResult::Handled
        });

        let sender = bus.sender();

        group.bench_function("fast_with_slow_registered", |b| {
            b.iter(|| {
                let fast = DynEvent::new(FastEvent { value: 1 });
                let mut ctx = HandlerContext::new(&sender);
                black_box(bus.dispatch(&fast, &mut ctx))
            });
        });
    }

    group.finish();
}
