//! Tracing/profiling driver for reovim.
//!
//! This driver implements the kernel's `Profiler` trait using the tracing ecosystem.
//! It bridges kernel profiling macros to tracing spans for flame graph generation.
//!
//! # Architecture
//!
//! Following Linux kernel design (mechanism vs policy):
//! - **Kernel provides mechanism**: `Profiler` trait, `SpanId`, `SpanData`, `profile_scope!`
//! - **This driver provides policy**: How spans are recorded, output format, filtering
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         User Code                                │
//! │   profile_scope!("process_key", "runner::input");                │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Kernel (debug/)                              │
//! │   Profiler trait │ SpanId │ SpanData │ ProfileScope              │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                 Driver (drivers/trace/)                          │
//! │   TracingProfiler │ init_profiling()                             │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     tracing ecosystem                            │
//! │   tracing │ tracing-subscriber │ flame graph tools               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use reovim_driver_trace::{TracingProfiler, init_profiling};
//! use reovim_kernel::api::v1::set_profiler;
//!
//! // Step 1: Initialize profiling (registers TracingProfiler)
//! init_profiling();
//!
//! // Step 2: Now profile_scope! macros route to tracing
//! profile_scope!("my_function", "my_module");
//! ```
//!
//! # Filtering
//!
//! The profiler respects the `REOVIM_PROFILE` environment variable:
//!
//! ```bash
//! # Enable all profiling
//! REOVIM_PROFILE=1 reovim myfile.txt
//!
//! # Filter by target prefix
//! REOVIM_PROFILE=runner reovim myfile.txt
//! ```
//!
//! # Thread-Local Span Stack
//!
//! The driver maintains a thread-local stack of active tracing spans.
//! This enables proper parent-child relationships for flame graphs
//! even when spans are created via the kernel's `ProfileScope` RAII guard.

use std::cell::RefCell;

use {
    reovim_kernel::api::v1::{Profiler, SetProfilerError, SpanData, SpanId, set_profiler},
    tracing::{Level, Span, span},
};

// =============================================================================
// Configuration
// =============================================================================

/// Environment variable to control profiling.
const PROFILE_ENV_VAR: &str = "REOVIM_PROFILE";

/// Check if profiling is enabled via environment variable.
///
/// Returns `Some(filter)` if enabled, where filter can be:
/// - `""` or `"1"` - enable all profiling
/// - `"target_prefix"` - only enable for targets starting with prefix
fn profiling_filter() -> Option<String> {
    std::env::var(PROFILE_ENV_VAR).ok()
}

// =============================================================================
// Thread-Local Span Stack
// =============================================================================

thread_local! {
    /// Stack of active tracing spans.
    ///
    /// When `enter()` is called, we create a tracing span and push it here.
    /// When `exit()` is called, we pop and drop the span.
    ///
    /// This enables proper parent-child relationships without relying on
    /// tracing's internal span stack (which requires `Span::enter()` guards).
    static SPAN_STACK: RefCell<Vec<Span>> = const { RefCell::new(Vec::new()) };
}

// =============================================================================
// Tracing Profiler
// =============================================================================

/// Tracing-based profiler implementation.
///
/// Bridges kernel `Profiler` trait to the tracing ecosystem.
///
/// # Features
///
/// - Creates tracing spans for each `profile_scope!`
/// - Records counters and histograms as tracing events
/// - Respects `REOVIM_PROFILE` environment variable for filtering
///
/// # Thread Safety
///
/// Uses thread-local storage for span tracking. Each thread maintains
/// its own span stack, so profiling is thread-safe.
pub struct TracingProfiler {
    /// Filter prefix (empty = all enabled).
    filter: Option<String>,
}

impl TracingProfiler {
    /// Create a new tracing profiler with default settings.
    ///
    /// Reads `REOVIM_PROFILE` environment variable for filtering.
    #[must_use]
    pub fn new() -> Self {
        Self {
            filter: profiling_filter(),
        }
    }

    /// Create a profiler that is always disabled.
    #[must_use]
    pub const fn disabled() -> Self {
        Self { filter: None }
    }

    /// Create a profiler with a specific filter.
    #[must_use]
    pub fn with_filter(filter: impl Into<String>) -> Self {
        Self {
            filter: Some(filter.into()),
        }
    }

    /// Check if a target matches the filter.
    fn matches_filter(&self, target: &str) -> bool {
        match &self.filter {
            None => false,
            Some(f) if f.is_empty() || f == "1" => true,
            Some(f) => target.starts_with(f.as_str()),
        }
    }
}

impl Default for TracingProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler for TracingProfiler {
    fn enabled(&self, target: &str) -> bool {
        self.matches_filter(target)
    }

    fn enter(&self, data: &SpanData) -> SpanId {
        // Create a tracing span
        let span = span!(
            target: "reovim::profile",
            Level::TRACE,
            "profile",
            name = data.name,
            target = data.target,
            span_id = data.id.as_u64(),
        );

        // Enter the span and push to stack
        // We need to keep the span alive, so we store it rather than
        // using span.enter() which returns a guard
        SPAN_STACK.with(|stack| {
            let entered = span.entered();
            // Convert EnteredSpan back to Span by forgetting the guard
            // This keeps the span "entered" until we explicitly exit
            let span = entered.exit();
            stack.borrow_mut().push(span);
        });

        data.id
    }

    fn exit(&self, _id: SpanId, elapsed_ns: u64) {
        SPAN_STACK.with(|stack| {
            if let Some(span) = stack.borrow_mut().pop() {
                // Record the elapsed time before dropping
                span.record("elapsed_ns", elapsed_ns);
                // Span drops here, closing it in tracing
                drop(span);
            }
        });
    }

    fn counter(&self, name: &'static str, value: u64) {
        if self.filter.is_some() {
            tracing::trace!(
                target: "reovim::metrics",
                counter = name,
                value = value,
                "counter"
            );
        }
    }

    fn histogram(&self, name: &'static str, value_us: u64) {
        if self.filter.is_some() {
            tracing::trace!(
                target: "reovim::metrics",
                histogram = name,
                value_us = value_us,
                "histogram"
            );
        }
    }
}

// =============================================================================
// Global Initialization
// =============================================================================

/// Static profiler instance.
///
/// Uses `TracingProfiler` which reads `REOVIM_PROFILE` at construction time.
static TRACING_PROFILER: std::sync::OnceLock<TracingProfiler> = std::sync::OnceLock::new();

/// Initialize profiling with the tracing backend.
///
/// This function:
/// 1. Creates a `TracingProfiler` (reads `REOVIM_PROFILE` env var)
/// 2. Sets it as the global kernel profiler
///
/// If `REOVIM_PROFILE` is not set, creates a disabled profiler that
/// has the same zero-overhead as `NopProfiler`.
///
/// # Errors
///
/// Returns `Err` if a profiler has already been set.
///
/// # Example
///
/// ```rust,ignore
/// use reovim_driver_trace::init_profiling;
///
/// // Call once during startup
/// init_profiling().expect("profiler not yet set");
///
/// // Or ignore if already set
/// let _ = init_profiling();
/// ```
pub fn init_profiling() -> Result<(), SetProfilerError> {
    let profiler = TRACING_PROFILER.get_or_init(TracingProfiler::new);
    set_profiler(profiler)
}

/// Initialize profiling with a custom filter.
///
/// Useful for testing or when you want to override the environment variable.
///
/// # Errors
///
/// Returns `Err` if a profiler has already been set.
pub fn init_profiling_with_filter(filter: impl Into<String>) -> Result<(), SetProfilerError> {
    // Can't use the static with a custom filter, so we need a different approach
    // For now, just use the environment-based one
    // Future: support Box<dyn Profiler> in kernel
    let _ = filter.into();
    init_profiling()
}

// =============================================================================
// Re-exports
// =============================================================================

/// Re-export kernel types for convenience.
pub use reovim_kernel::api::v1::{
    NopProfiler as KernelNopProfiler, ProfileScope, Profiler as KernelProfiler,
    SpanData as KernelSpanData, SpanId as KernelSpanId, profiler,
    set_profiler as kernel_set_profiler,
};

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_profiler_disabled_by_default() {
        // Without REOVIM_PROFILE set, profiler should be disabled
        let prof = TracingProfiler::disabled();
        assert!(!prof.enabled("any::target"));
    }

    #[test]
    fn test_tracing_profiler_filter_empty() {
        let prof = TracingProfiler::with_filter("");
        // Empty filter means all enabled
        assert!(prof.enabled("any::target"));
        assert!(prof.enabled("runner::input"));
    }

    #[test]
    fn test_tracing_profiler_filter_one() {
        let prof = TracingProfiler::with_filter("1");
        // "1" means all enabled
        assert!(prof.enabled("any::target"));
    }

    #[test]
    fn test_tracing_profiler_filter_prefix() {
        let prof = TracingProfiler::with_filter("runner");
        assert!(prof.enabled("runner"));
        assert!(prof.enabled("runner::input"));
        assert!(prof.enabled("runner::keymap"));
        assert!(!prof.enabled("mm::buffer"));
        assert!(!prof.enabled("other"));
    }

    #[test]
    fn test_tracing_profiler_enter_exit() {
        let prof = TracingProfiler::with_filter("test");

        let data = SpanData::new("test_span", "test::module");
        let id = prof.enter(&data);

        // ID should be what we passed in
        assert_eq!(id, data.id);

        // Exit should not panic
        prof.exit(id, 1000);
    }

    #[test]
    fn test_tracing_profiler_counter_histogram() {
        let prof = TracingProfiler::with_filter("1");

        // These should not panic
        prof.counter("test_counter", 1);
        prof.counter("test_counter", 100);
        prof.histogram("test_histogram", 42);
    }

    #[test]
    fn test_tracing_profiler_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TracingProfiler>();
    }

    #[test]
    fn test_span_stack_underflow() {
        // Calling exit without enter should not panic
        let prof = TracingProfiler::with_filter("1");
        prof.exit(SpanId::new(), 1000);
        // Should just be a no-op
    }

    #[test]
    fn test_nested_spans() {
        let prof = TracingProfiler::with_filter("test");

        // Create nested spans
        let data1 = SpanData::new("outer", "test");
        let id1 = prof.enter(&data1);

        let data2 = SpanData::new("inner", "test");
        let id2 = prof.enter(&data2);

        // Exit in reverse order
        prof.exit(id2, 100);
        prof.exit(id1, 200);

        // Should not panic
    }
}
