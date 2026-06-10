# debug/ - Debug and Tracing

Metrics collection, profiling, and runtime trace points.

## Source Location

`server/lib/kernel/src/debug/`

## Overview

The `debug/` subsystem provides instrumentation infrastructure following the
kernel "mechanism, not policy" principle:

- The kernel defines the `Profiler` and `TraceSink` traits (mechanism).
- Drivers implement them with the `tracing` ecosystem or other backends (policy).
- When no driver registers a backend, all operations are no-ops with zero overhead.

The subsystem has three components: trace points (`trace`), metrics
(`metrics`), and profiling (`profiler`).

```
Runner/Modules    (use profile_scope! macro, trace_event! macro)
       |
       v
Kernel debug/     Profiler trait, TracePoint, Counter, Histogram
       |
       v
Drivers           TracingProfiler -> tracing crate, custom sinks
```

## Components

### Trace Points (`trace.rs`)

Runtime-togglable markers. Define statically, check cheaply, emit to a
registered `TraceSink`.

```rust
pub struct TracePoint {
    pub name: &'static str,
    pub module: &'static str,
    // enabled: AtomicBool (runtime toggle)
}

pub trait TraceSink: Send + Sync {
    fn emit(&self, event: TraceEvent);
}
```

Macros:

```rust
// Define a trace point (module-level, static)
define_tracepoint!(TP_BUFFER_CREATE, "mm");

// Emit an event if enabled
trace_event!(TP_BUFFER_CREATE, "Created buffer {}", id);
```

`set_trace_sink(sink)` registers the global sink (one-time, `OnceLock`).

### Metrics (`metrics.rs`)

Lock-free `Counter` and `Histogram` types backed by atomics. Named instances
are accessible through the global `MetricsRegistry`.

```rust
pub struct Counter {
    value: AtomicU64,
}

pub struct Histogram {
    buckets: [AtomicU64; 16],  // power-of-two boundaries
    sum: AtomicU64,
    count: AtomicU64,
}
```

Usage:

```rust
// Static counter
static BUFFER_CREATES: Counter = Counter::new();
BUFFER_CREATES.increment();

// Named counter via global registry
metrics().counter("keys_processed").increment();

// Histogram
metrics().histogram("render_us").record(elapsed_us);
```

`MetricsRegistry::snapshot()` returns a `MetricsSnapshot` with all counter
values and histogram (count, mean) pairs.

### Profiler (`profiler.rs`)

Trait-based scope timing with RAII guards.

```rust
pub trait Profiler: Send + Sync {
    fn enabled(&self, target: &str) -> bool;
    fn enter(&self, data: &SpanData) -> SpanId;
    fn exit(&self, id: SpanId, elapsed_ns: u64);
    fn counter(&self, name: &'static str, value: u64);
    fn histogram(&self, name: &'static str, value_us: u64);
}
```

`NopProfiler` is the default — `enabled()` returns `false`, causing
`ProfileScope` to skip all work entirely.

`set_profiler(&'static dyn Profiler)` registers the global profiler once.

#### RAII Guard

```rust
pub struct ProfileScope { /* id, start, active */ }

impl ProfileScope {
    pub fn new(name: &'static str, target: &'static str) -> Self { /* ... */ }
}
// Records timing on drop when active.
```

#### Macros

```rust
// Profile a named scope
profile_scope!("process_key", "runner::input");

// Profile using module_path!() as target
profile_fn!("my_function");

// Increment a counter via profiler
profile_counter!("keys_processed");
profile_counter!("bytes_read", 1024);

// Record a histogram sample
profile_histogram!("request_latency", latency_us);
```

## Type Reference

| Type | Purpose |
|------|---------|
| `TracePoint` | Static, runtime-togglable trace point |
| `TraceEvent` | Event data emitted to a `TraceSink` |
| `TraceSink` | Trait: driver-implemented trace event consumer |
| `Counter` | Lock-free atomic event counter |
| `Histogram` | Lock-free power-of-two bucket histogram |
| `MetricsRegistry` | Named counter/histogram registry (global singleton) |
| `MetricsSnapshot` | Point-in-time snapshot of all metrics |
| `Profiler` | Trait: driver-implemented span recorder |
| `NopProfiler` | Zero-overhead no-op profiler (default) |
| `ProfileScope` | RAII guard that records scope timing on drop |
| `SpanId` | Unique span identifier for `Profiler::enter`/`exit` |
| `SpanData` | Span metadata (name, target, start timestamp) |

## API Exports

```rust
use reovim_kernel::api::v1::{
    // Profiler
    Profiler, NopProfiler, ProfileScope, SpanId, SpanData,
    set_profiler, profiler,
    // Metrics
    Counter, Histogram, MetricsRegistry, MetricsSnapshot, metrics,
    // Trace
    TracePoint, TraceEvent, TraceSink, emit_trace, set_trace_sink,
};
```

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [printk Subsystem](../printk/overview.md) - Kernel logging
