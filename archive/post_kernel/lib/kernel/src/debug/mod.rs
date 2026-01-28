//! Debug and tracing subsystem.
//!
//! Linux equivalent: `kernel/trace/`
//!
//! Provides tracing infrastructure, metrics collection, and profiling.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Debug Subsystem                          │
//! │                                                             │
//! │  ┌─────────────────┐  ┌─────────────────┐                   │
//! │  │    Tracing      │  │    Metrics      │                   │
//! │  │ (trace points,  │  │ (counters,      │                   │
//! │  │  events, sinks) │  │  histograms)    │                   │
//! │  └─────────────────┘  └─────────────────┘                   │
//! │                                                             │
//! │  ┌─────────────────┐                                        │
//! │  │    Profiler     │                                        │
//! │  │ (RAII guards,   │                                        │
//! │  │  scope timing)  │                                        │
//! │  └─────────────────┘                                        │
//! │                                                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Components
//!
//! - [`trace`] - Trace points and event emission
//! - [`metrics`] - Counters and histograms for performance monitoring
//! - [`profiler`] - RAII-based scope timing
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::debug::{TracePoint, ProfileGuard, metrics};
//!
//! // Define a trace point
//! static TP_BUFFER_OP: TracePoint = TracePoint::new("buffer_op", "mm");
//!
//! fn process_buffer() {
//!     // Profile the function
//!     let _profile = ProfileGuard::new("process_buffer");
//!
//!     // Emit trace event
//!     if TP_BUFFER_OP.is_enabled() {
//!         // trace event...
//!     }
//!
//!     // Increment a counter (returns Arc<Counter>)
//!     metrics().counter("buffer_ops").increment();
//! }
//! ```

mod metrics;
mod profiler;
mod trace;

// Re-export tracing types
#[allow(unused_imports)]
pub use trace::{TraceEvent, TracePoint, TraceSink, emit_trace, set_trace_sink};

// Re-export metrics types
#[allow(unused_imports)]
pub use metrics::{Counter, Histogram, MetricsRegistry, MetricsSnapshot, metrics};

// Re-export profiler types
#[allow(unused_imports, deprecated)]
pub use profiler::{
    NopProfiler, ProfileGuard, ProfileScope, Profiler, SetProfilerError, SpanData, SpanId,
    profiler, set_profiler,
};
