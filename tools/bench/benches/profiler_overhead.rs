//! Profiler overhead benchmarks.
//!
//! Measures the overhead of the profiling infrastructure itself.

use {
    criterion::{Criterion, criterion_group, criterion_main},
    reovim_kernel::api::v1::*,
};

/// Benchmark: ProfileScope with NopProfiler (should be near-zero).
fn bench_profile_scope_nop(c: &mut Criterion) {
    // NopProfiler is the default when no profiler is set
    c.bench_function("profiler/scope_nop", |b| {
        b.iter(|| {
            let _scope = ProfileScope::new("test_scope", "bench::test");
            // Scope drops here
        });
    });
}

/// Benchmark: ProfileGuard (legacy, records to MetricsRegistry).
fn bench_profile_guard(c: &mut Criterion) {
    c.bench_function("profiler/guard_legacy", |b| {
        b.iter(|| {
            let _guard = ProfileGuard::new("bench_guard");
            // Guard drops here, records to histogram
        });
    });
}

/// Benchmark: SpanId creation.
fn bench_span_id_creation(c: &mut Criterion) {
    c.bench_function("profiler/span_id_new", |b| {
        b.iter(|| {
            let _ = SpanId::new();
        });
    });
}

/// Benchmark: SpanData creation.
fn bench_span_data_creation(c: &mut Criterion) {
    c.bench_function("profiler/span_data_new", |b| {
        b.iter(|| {
            let _ = SpanData::new("test_span", "bench::test");
        });
    });
}

/// Benchmark: Profiler enabled() check.
fn bench_profiler_enabled_check(c: &mut Criterion) {
    c.bench_function("profiler/enabled_check", |b| {
        b.iter(|| {
            let _ = profiler().enabled("bench::test");
        });
    });
}

/// Benchmark: Counter via profiler.
fn bench_profiler_counter(c: &mut Criterion) {
    c.bench_function("profiler/counter", |b| {
        b.iter(|| {
            profiler().counter("bench_counter", 1);
        });
    });
}

/// Benchmark: Histogram via profiler.
fn bench_profiler_histogram(c: &mut Criterion) {
    c.bench_function("profiler/histogram", |b| {
        b.iter(|| {
            profiler().histogram("bench_histogram", 100);
        });
    });
}

/// Benchmark: MetricsRegistry counter access.
fn bench_metrics_counter(c: &mut Criterion) {
    c.bench_function("metrics/counter_get_and_increment", |b| {
        b.iter(|| {
            let counter = metrics().counter("bench_counter");
            counter.increment();
        });
    });
}

/// Benchmark: MetricsRegistry histogram access.
fn bench_metrics_histogram(c: &mut Criterion) {
    c.bench_function("metrics/histogram_get_and_record", |b| {
        b.iter(|| {
            let hist = metrics().histogram("bench_histogram");
            hist.record(100);
        });
    });
}

/// Benchmark: Nested ProfileScope (simulates real call stack).
fn bench_nested_profile_scopes(c: &mut Criterion) {
    c.bench_function("profiler/nested_scopes_3", |b| {
        b.iter(|| {
            let _outer = ProfileScope::new("outer", "bench");
            {
                let _middle = ProfileScope::new("middle", "bench");
                {
                    let _inner = ProfileScope::new("inner", "bench");
                    // All scopes drop here in reverse order
                }
            }
        });
    });
}

criterion_group!(
    benches,
    bench_profile_scope_nop,
    bench_profile_guard,
    bench_span_id_creation,
    bench_span_data_creation,
    bench_profiler_enabled_check,
    bench_profiler_counter,
    bench_profiler_histogram,
    bench_metrics_counter,
    bench_metrics_histogram,
    bench_nested_profile_scopes,
);
criterion_main!(benches);
