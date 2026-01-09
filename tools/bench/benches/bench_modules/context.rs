//! Context plugin benchmarks
//!
//! Measures performance of the context caching and event system.
//! These benchmarks help identify latency in context updates.

use std::{
    hint::black_box,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use {
    criterion::{BenchmarkId, Criterion},
    reovim_core::{
        context_provider::{ContextHierarchy, ContextItem},
        event_bus::{DynEvent, EventBus, EventResult, HandlerContext},
    },
    reovim_plugin_context::{
        CachedContext, ContextManager, CursorContextUpdated, ViewportContextUpdated,
    },
};

/// Create a test context hierarchy with varying depth
fn create_test_hierarchy(depth: usize) -> ContextHierarchy {
    let items: Vec<ContextItem> = (0..depth)
        .map(|i| ContextItem {
            text: format!("scope_{}", i),
            start_line: i as u32 * 10,
            end_line: (i as u32 + 1) * 100,
            kind: "function".to_string(),
            level: i,
        })
        .collect();
    ContextHierarchy::with_items(1, 50, 0, items)
}

/// Benchmark ContextManager cursor context read (ArcSwap load)
///
/// Measures lock-free read performance for cached context.
/// This is called on every render frame.
pub fn bench_context_manager_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("context/manager_read");

    for depth in [0, 1, 3, 5, 10] {
        let manager = ContextManager::new();

        // Pre-populate cache
        if depth > 0 {
            manager.set_cursor_context(CachedContext {
                buffer_id: 1,
                line: 50,
                col: 0,
                context: Some(create_test_hierarchy(depth)),
            });
        }

        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, _| {
            b.iter(|| black_box(manager.get_cursor_context()));
        });
    }

    group.finish();
}

/// Benchmark ContextManager cursor context write (ArcSwap store)
///
/// Measures write performance when context is updated.
pub fn bench_context_manager_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("context/manager_write");

    for depth in [1, 3, 5, 10] {
        let manager = ContextManager::new();
        let hierarchy = create_test_hierarchy(depth);

        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, _| {
            b.iter(|| {
                manager.set_cursor_context(CachedContext {
                    buffer_id: 1,
                    line: 50,
                    col: 0,
                    context: Some(hierarchy.clone()),
                });
            });
        });
    }

    group.finish();
}

/// Benchmark cursor change detection
///
/// Measures the overhead of checking if cursor position changed.
pub fn bench_cursor_change_detection(c: &mut Criterion) {
    let manager = ContextManager::new();

    // Set initial position
    manager.update_cursor(1, 100, 50);

    c.bench_function("context/cursor_changed_same", |b| {
        b.iter(|| {
            // Check same position (common case - no change)
            black_box(manager.cursor_changed(1, 100, 50))
        });
    });

    c.bench_function("context/cursor_changed_different", |b| {
        b.iter(|| {
            // Check different position (cursor moved)
            black_box(manager.cursor_changed(1, 101, 50))
        });
    });
}

/// Benchmark viewport change detection
pub fn bench_viewport_change_detection(c: &mut Criterion) {
    let manager = ContextManager::new();

    // Set initial viewport
    manager.update_viewport(1, 50);

    c.bench_function("context/viewport_changed_same", |b| {
        b.iter(|| black_box(manager.viewport_changed(1, 50)));
    });

    c.bench_function("context/viewport_changed_different", |b| {
        b.iter(|| black_box(manager.viewport_changed(1, 51)));
    });
}

/// Benchmark buffer cache invalidation
///
/// Measures the overhead when a buffer is modified and cache needs clearing.
pub fn bench_cache_invalidation(c: &mut Criterion) {
    let mut group = c.benchmark_group("context/invalidation");

    // Test with cache populated vs empty
    for cached in [false, true] {
        let manager = ContextManager::new();

        if cached {
            manager.set_cursor_context(CachedContext {
                buffer_id: 1,
                line: 50,
                col: 0,
                context: Some(create_test_hierarchy(3)),
            });
            manager.set_viewport_context(CachedContext {
                buffer_id: 1,
                line: 0,
                col: 0,
                context: Some(create_test_hierarchy(3)),
            });
        }

        let label = if cached { "cached" } else { "empty" };

        group.bench_function(label, |b| {
            b.iter(|| {
                manager.invalidate_buffer(1);
                // Re-populate for next iteration if testing cached
                if cached {
                    manager.set_cursor_context(CachedContext {
                        buffer_id: 1,
                        line: 50,
                        col: 0,
                        context: Some(create_test_hierarchy(3)),
                    });
                }
            });
        });
    }

    group.finish();
}

/// Benchmark CursorContextUpdated event dispatch
///
/// Measures end-to-end latency of context event creation and dispatch.
pub fn bench_cursor_context_event_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("context/event_dispatch");

    for handler_count in [1, 3, 5] {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        // Register handlers (simulating statusline, sticky-context, etc.)
        for _ in 0..handler_count {
            let counter_clone = Arc::clone(&counter);
            bus.subscribe::<CursorContextUpdated, _>(100, move |_event, _ctx| {
                counter_clone.fetch_add(1, Ordering::Relaxed);
                EventResult::Handled
            });
        }

        let sender = bus.sender();
        let hierarchy = create_test_hierarchy(3);

        group.bench_with_input(
            BenchmarkId::new("handlers", handler_count),
            &handler_count,
            |b, _| {
                b.iter(|| {
                    let event = DynEvent::new(CursorContextUpdated {
                        buffer_id: 1,
                        line: 50,
                        col: 0,
                        context: Some(hierarchy.clone()),
                    });
                    let mut ctx = HandlerContext::new(&sender);
                    black_box(bus.dispatch(&event, &mut ctx))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark ViewportContextUpdated event dispatch
pub fn bench_viewport_context_event_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("context/viewport_event_dispatch");

    for handler_count in [1, 3, 5] {
        let bus = EventBus::new(16);
        let counter = Arc::new(AtomicU64::new(0));

        for _ in 0..handler_count {
            let counter_clone = Arc::clone(&counter);
            bus.subscribe::<ViewportContextUpdated, _>(100, move |_event, _ctx| {
                counter_clone.fetch_add(1, Ordering::Relaxed);
                EventResult::Handled
            });
        }

        let sender = bus.sender();
        let hierarchy = create_test_hierarchy(3);

        group.bench_with_input(
            BenchmarkId::new("handlers", handler_count),
            &handler_count,
            |b, _| {
                b.iter(|| {
                    let event = DynEvent::new(ViewportContextUpdated {
                        window_id: 0,
                        buffer_id: 1,
                        top_line: 0,
                        context: Some(hierarchy.clone()),
                    });
                    let mut ctx = HandlerContext::new(&sender);
                    black_box(bus.dispatch(&event, &mut ctx))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark full context update cycle
///
/// Simulates the complete flow: cursor move -> context compute -> event dispatch -> cache update.
/// This measures the end-to-end latency a user would experience.
pub fn bench_full_context_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("context/full_cycle");

    for depth in [1, 3, 5] {
        let manager = Arc::new(ContextManager::new());
        let bus = EventBus::new(16);

        // Handler that caches the context (like statusline/sticky-context)
        let manager_clone = Arc::clone(&manager);
        bus.subscribe::<CursorContextUpdated, _>(100, move |event, _ctx| {
            // Simulate what consumers do: cache the context
            manager_clone.set_cursor_context(CachedContext {
                buffer_id: event.buffer_id,
                line: event.line,
                col: event.col,
                context: event.context.clone(),
            });
            EventResult::Handled
        });

        let sender = bus.sender();
        let hierarchy = create_test_hierarchy(depth);

        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, _| {
            let mut line = 0u32;
            b.iter(|| {
                // 1. Check if cursor changed
                let changed = manager.cursor_changed(1, line, 0);
                if changed {
                    manager.update_cursor(1, line, 0);
                }

                // 2. Emit context event
                let event = DynEvent::new(CursorContextUpdated {
                    buffer_id: 1,
                    line,
                    col: 0,
                    context: Some(hierarchy.clone()),
                });
                let mut ctx = HandlerContext::new(&sender);
                bus.dispatch(&event, &mut ctx);

                // 3. Read cached context (what renderer does)
                black_box(manager.get_cursor_context());

                line = line.wrapping_add(1);
            });
        });
    }

    group.finish();
}

/// Benchmark context hierarchy cloning
///
/// Context is cloned when cached and when events are dispatched.
/// This measures the overhead of cloning different hierarchy sizes.
pub fn bench_context_hierarchy_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("context/hierarchy_clone");

    for depth in [1, 3, 5, 10, 20] {
        let hierarchy = create_test_hierarchy(depth);

        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, _| {
            b.iter(|| black_box(hierarchy.clone()));
        });
    }

    group.finish();
}
