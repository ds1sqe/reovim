//! Event dispatch benchmarks.
//!
//! Measures performance of the event bus with varying handler counts.

use {
    criterion::{BenchmarkId, Criterion, criterion_group, criterion_main},
    reovim_kernel::api::v1::*,
};

/// Test event for benchmarking.
#[derive(Debug)]
struct BenchEvent {
    #[allow(dead_code)]
    value: u64,
}

impl Event for BenchEvent {}

/// Benchmark: Event dispatch with no handlers.
fn bench_dispatch_no_handlers(c: &mut Criterion) {
    let bus = EventBus::new();

    c.bench_function("event/dispatch_0_handlers", |b| {
        b.iter(|| {
            bus.emit(BenchEvent { value: 42 });
        });
    });
}

/// Benchmark: Event dispatch with varying handler counts.
fn bench_dispatch_with_handlers(c: &mut Criterion) {
    let mut group = c.benchmark_group("event/dispatch");

    for &handler_count in &[1, 5, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("handlers", handler_count),
            &handler_count,
            |b, &handler_count| {
                let bus = EventBus::new();

                // Register handlers (keep subscriptions alive)
                let _subs: Vec<_> = (0..handler_count)
                    .map(|i| bus.subscribe::<BenchEvent, _>(i * 10, |_event| EventResult::Handled))
                    .collect();

                b.iter(|| {
                    bus.emit(BenchEvent { value: 42 });
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Event dispatch where handlers propagate.
fn bench_dispatch_propagate(c: &mut Criterion) {
    let mut group = c.benchmark_group("event/dispatch_propagate");

    for &handler_count in &[1, 5, 10] {
        group.bench_with_input(
            BenchmarkId::new("handlers", handler_count),
            &handler_count,
            |b, &handler_count| {
                let bus = EventBus::new();

                // Handlers that don't consume (propagate the event)
                let _subs: Vec<_> = (0..handler_count)
                    .map(|i| bus.subscribe::<BenchEvent, _>(i * 10, |_event| EventResult::Handled))
                    .collect();

                b.iter(|| {
                    bus.emit(BenchEvent { value: 42 });
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Subscription creation and teardown.
fn bench_subscribe(c: &mut Criterion) {
    let bus = EventBus::new();

    c.bench_function("event/subscribe", |b| {
        b.iter(|| {
            let _sub = bus.subscribe::<BenchEvent, _>(100, |_event| EventResult::Handled);
            // Subscription dropped here, auto-unsubscribes
        });
    });
}

/// Benchmark: Event dispatch with different event types.
#[derive(Debug)]
struct BenchEventA {
    #[allow(dead_code)]
    value: u64,
}
impl Event for BenchEventA {}

#[derive(Debug)]
struct BenchEventB {
    #[allow(dead_code)]
    value: u64,
}
impl Event for BenchEventB {}

#[derive(Debug)]
struct BenchEventC {
    #[allow(dead_code)]
    value: u64,
}
impl Event for BenchEventC {}

fn bench_dispatch_multiple_types(c: &mut Criterion) {
    let bus = EventBus::new();

    // Register handlers for different event types
    let _sub_a = bus.subscribe::<BenchEventA, _>(100, |_| EventResult::Handled);
    let _sub_b = bus.subscribe::<BenchEventB, _>(100, |_| EventResult::Handled);
    let _sub_c = bus.subscribe::<BenchEventC, _>(100, |_| EventResult::Handled);

    c.bench_function("event/dispatch_multi_type", |b| {
        b.iter(|| {
            bus.emit(BenchEventA { value: 1 });
            bus.emit(BenchEventB { value: 2 });
            bus.emit(BenchEventC { value: 3 });
        });
    });
}

criterion_group!(
    benches,
    bench_dispatch_no_handlers,
    bench_dispatch_with_handlers,
    bench_dispatch_propagate,
    bench_subscribe,
    bench_dispatch_multiple_types,
);
criterion_main!(benches);
