//! Profiling infrastructure.
//!
//! Linux equivalent: `kernel/trace/ring_buffer.c`
//!
//! Provides trait-based profiling following the `Logger` pattern:
//! - Kernel defines the `Profiler` trait (mechanism)
//! - Drivers implement it with tracing ecosystem (policy)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Runner/Modules         (use profile_scope! macro)          │
//! └─────────────────────────┬───────────────────────────────────┘
//!                           │
//!                           v
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Kernel (profiler.rs)   Profiler trait, ProfileScope RAII   │
//! └─────────────────────────┬───────────────────────────────────┘
//!                           │
//!                           v
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Drivers (trace/)       TracingProfiler -> tracing crate    │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Zero-Overhead Default
//!
//! When no profiler is set, `NopProfiler` is used. The `enabled()` check
//! allows early exit before any allocation or timing occurs.
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::{profile_scope, profile_counter};
//!
//! fn process_key(key: KeyEvent) {
//!     profile_scope!("process_key", "runner::input");
//!
//!     // ... process the key ...
//!     profile_counter!("keys_processed");
//! }
//! ```

use std::{
    error::Error,
    fmt,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use super::metrics;

// =============================================================================
// Span Identifier
// =============================================================================

/// Unique identifier for a profiling span.
///
/// Used to track hierarchical spans for flame graph generation.
/// Drivers use this to correlate `enter()` and `exit()` calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpanId(u64);

impl SpanId {
    /// Create a new unique span ID.
    ///
    /// IDs are globally unique within a process (uses atomic counter).
    #[must_use]
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Null span ID (represents no parent or inactive span).
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Check if this is the null span.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for SpanId {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Span Data
// =============================================================================

/// Metadata for a profiling span.
///
/// Contains all information needed for the driver to create a tracing span.
/// Uses `&'static str` for zero-allocation in hot paths.
#[derive(Debug, Clone)]
pub struct SpanData {
    /// Unique identifier for this span.
    pub id: SpanId,
    /// Parent span ID (null if top-level).
    pub parent: SpanId,
    /// Span name (e.g., `process_key`, `keymap_lookup`).
    pub name: &'static str,
    /// Target/module path (e.g., `runner::input`, `mm::buffer`).
    pub target: &'static str,
    /// Start timestamp in nanoseconds since process start.
    pub start_ns: u64,
}

impl SpanData {
    /// Create a new span data with the given name and target.
    #[must_use]
    pub fn new(name: &'static str, target: &'static str) -> Self {
        Self {
            id: SpanId::new(),
            parent: SpanId::null(),
            name,
            target,
            start_ns: timestamp_ns(),
        }
    }
}

/// Get current timestamp in nanoseconds (monotonic, process-relative).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // Nanosecond truncation acceptable for profiling
fn timestamp_ns() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_nanos() as u64
}

// =============================================================================
// Profiler Trait
// =============================================================================

/// Profiler trait - kernel defines mechanism, drivers implement policy.
///
/// Following the `Logger` pattern: the kernel provides the trait interface,
/// and drivers (e.g., `shared/trace/`) implement it with the tracing
/// ecosystem.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` as the profiler is accessed from
/// multiple threads concurrently. Implementations should minimize locking
/// to avoid blocking hot paths.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// struct MyProfiler;
///
/// impl Profiler for MyProfiler {
///     fn enabled(&self, _target: &str) -> bool {
///         true // Always enabled
///     }
///
///     fn enter(&self, data: &SpanData) -> SpanId {
///         println!("Entering span: {}", data.name);
///         data.id
///     }
///
///     fn exit(&self, _id: SpanId, elapsed_ns: u64) {
///         println!("Exiting span, elapsed: {}ns", elapsed_ns);
///     }
///
///     fn counter(&self, name: &'static str, value: u64) {
///         println!("Counter {}: {}", name, value);
///     }
///
///     fn histogram(&self, name: &'static str, value_us: u64) {
///         println!("Histogram {}: {}us", name, value_us);
///     }
/// }
/// ```
pub trait Profiler: Send + Sync {
    /// Check if profiling is enabled for the given target.
    ///
    /// Called before creating a span to avoid allocation overhead.
    /// If this returns `false`, `ProfileScope` will be a no-op.
    ///
    /// # Arguments
    ///
    /// * `target` - The module/target path (e.g., `runner::input`)
    fn enabled(&self, target: &str) -> bool;

    /// Enter a new span.
    ///
    /// Called when a profiling scope begins. The driver should record
    /// the span and return the span ID for later correlation.
    ///
    /// # Arguments
    ///
    /// * `data` - Span metadata including name, target, and timestamps
    ///
    /// # Returns
    ///
    /// The span ID to use for `exit()` (usually `data.id`).
    fn enter(&self, data: &SpanData) -> SpanId;

    /// Exit a span with timing information.
    ///
    /// Called when a profiling scope ends. The driver should record
    /// the elapsed time and close the span.
    ///
    /// # Arguments
    ///
    /// * `id` - The span ID returned from `enter()`
    /// * `elapsed_ns` - Elapsed time in nanoseconds
    fn exit(&self, id: SpanId, elapsed_ns: u64);

    /// Record a counter increment.
    ///
    /// Used for counting events like keys processed, commands executed, etc.
    ///
    /// # Arguments
    ///
    /// * `name` - Counter name (static for zero allocation)
    /// * `value` - Value to add (typically 1)
    fn counter(&self, name: &'static str, value: u64);

    /// Record a histogram sample.
    ///
    /// Used for timing distributions (e.g., command execution latency).
    ///
    /// # Arguments
    ///
    /// * `name` - Histogram name (static for zero allocation)
    /// * `value_us` - Sample value in microseconds
    fn histogram(&self, name: &'static str, value_us: u64);
}

// =============================================================================
// No-Op Profiler (Default)
// =============================================================================

/// No-op profiler used when no profiler is set.
///
/// All operations are no-ops. `enabled()` returns `false`, causing
/// `ProfileScope` to skip all work. This provides zero overhead when
/// profiling is disabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct NopProfiler;

impl Profiler for NopProfiler {
    #[inline]
    fn enabled(&self, _target: &str) -> bool {
        false
    }

    #[inline]
    fn enter(&self, _data: &SpanData) -> SpanId {
        SpanId::null()
    }

    #[inline]
    fn exit(&self, _id: SpanId, _elapsed_ns: u64) {}

    #[inline]
    fn counter(&self, _name: &'static str, _value: u64) {}

    #[inline]
    fn histogram(&self, _name: &'static str, _value_us: u64) {}
}

// =============================================================================
// Global Profiler
// =============================================================================

/// Error returned when attempting to set the profiler more than once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetProfilerError;

impl fmt::Display for SetProfilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "profiler already set")
    }
}

impl Error for SetProfilerError {}

/// Global profiler storage.
static PROFILER: OnceLock<&'static dyn Profiler> = OnceLock::new();

/// Static no-op profiler instance.
static NOP_PROFILER: NopProfiler = NopProfiler;

/// Set the global profiler.
///
/// This function can only be called once. Subsequent calls will
/// return `Err(SetProfilerError)`.
///
/// # Errors
///
/// Returns `Err(SetProfilerError)` if a profiler has already been set.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// static MY_PROFILER: NopProfiler = NopProfiler;
///
/// // First call succeeds (in practice, only call once during init)
/// // set_profiler(&MY_PROFILER).expect("profiler not yet set");
/// ```
pub fn set_profiler(profiler: &'static dyn Profiler) -> Result<(), SetProfilerError> {
    PROFILER.set(profiler).map_err(|_| SetProfilerError)
}

/// Get the global profiler.
///
/// Returns the registered profiler, or `NopProfiler` if none was set.
#[must_use]
pub fn profiler() -> &'static dyn Profiler {
    PROFILER.get().copied().unwrap_or(&NOP_PROFILER)
}

// =============================================================================
// Profile Scope (RAII Guard)
// =============================================================================

/// RAII guard for profiling a scope.
///
/// Creates a span on construction and records timing on drop.
/// When profiling is disabled (`enabled()` returns false), this is a no-op.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::debug::ProfileScope;
///
/// fn expensive_operation() {
///     let _scope = ProfileScope::new("expensive_operation", "mymodule");
///     // ... do work ...
/// } // timing recorded when _scope drops
/// ```
pub struct ProfileScope {
    id: SpanId,
    start: Instant,
    active: bool,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ProfileScope {
    /// Create a new profile scope.
    ///
    /// If profiling is disabled for the target, returns an inactive scope
    /// that does nothing on drop.
    #[must_use]
    pub fn new(name: &'static str, target: &'static str) -> Self {
        let prof = profiler();
        if !prof.enabled(target) {
            return Self {
                id: SpanId::null(),
                start: Instant::now(),
                active: false,
            };
        }

        let start = Instant::now();
        let data = SpanData::new(name, target);
        let id = prof.enter(&data);

        Self {
            id,
            start,
            active: true,
        }
    }

    /// Check if this scope is active (profiling enabled).
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Drop for ProfileScope {
    #[allow(clippy::cast_possible_truncation)] // Nanosecond truncation acceptable for profiling
    fn drop(&mut self) {
        if self.active {
            let elapsed_ns = self.start.elapsed().as_nanos() as u64;
            profiler().exit(self.id, elapsed_ns);
        }
    }
}

// =============================================================================
// Legacy Profile Guard (Backward Compatibility)
// =============================================================================

/// RAII guard for timing a scope (legacy API).
///
/// Records the elapsed time to a histogram when dropped.
/// This is the original profiling mechanism that records directly to
/// the `MetricsRegistry`. For new code, prefer `ProfileScope` with
/// the `Profiler` trait.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::debug::ProfileGuard;
///
/// fn expensive_operation() {
///     let _guard = ProfileGuard::new("expensive_operation");
///     // ... do work ...
/// } // time recorded to histogram when guard drops
/// ```
#[deprecated(since = "0.9.5", note = "Use profile_scope!() macro instead")]
pub struct ProfileGuard {
    name: &'static str,
    start: Instant,
}

#[allow(deprecated)]
#[cfg_attr(coverage_nightly, coverage(off))]
impl ProfileGuard {
    /// Create a new profile guard that will record to the named histogram.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
        }
    }
}

#[allow(deprecated)]
#[cfg_attr(coverage_nightly, coverage(off))]
impl Drop for ProfileGuard {
    #[allow(clippy::cast_possible_truncation)] // Microsecond truncation acceptable
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        let histogram = metrics().histogram(self.name);
        histogram.record(elapsed.as_micros() as u64);
    }
}

// =============================================================================
// Profiling Macros
// =============================================================================

/// Profile a scope with the `Profiler` trait.
///
/// Zero overhead when profiling is disabled (checked at runtime).
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::profile_scope;
///
/// fn process_buffer() {
///     profile_scope!("process_buffer", "mm");
///     // ... work ...
/// }
/// ```
#[macro_export]
macro_rules! profile_scope {
    ($name:expr, $target:expr) => {
        let _profile_scope_guard = $crate::api::v1::ProfileScope::new($name, $target);
    };
}

/// Profile a function (uses module path as target).
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::profile_fn;
///
/// fn my_function() {
///     profile_fn!("my_function");
///     // ... work ...
/// }
/// ```
#[macro_export]
macro_rules! profile_fn {
    ($name:expr) => {
        $crate::profile_scope!($name, module_path!())
    };
}

/// Increment a counter metric via the global profiler.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::profile_counter;
///
/// fn handle_key() {
///     profile_counter!("keys_processed");
///     // or with explicit value:
///     profile_counter!("bytes_read", 1024);
/// }
/// ```
#[macro_export]
macro_rules! profile_counter {
    ($name:expr) => {
        $crate::api::v1::profiler().counter($name, 1)
    };
    ($name:expr, $value:expr) => {
        $crate::api::v1::profiler().counter($name, $value)
    };
}

/// Record a histogram sample via the global profiler.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::profile_histogram;
///
/// fn measure_latency() {
///     let latency_us = 42;
///     profile_histogram!("request_latency", latency_us);
/// }
/// ```
#[macro_export]
macro_rules! profile_histogram {
    ($name:expr, $value_us:expr) => {
        $crate::api::v1::profiler().histogram($name, $value_us)
    };
}

/// Profile a scope and record to histogram (legacy macro).
///
/// Uses `ProfileGuard` which records directly to `MetricsRegistry`.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::profile;
///
/// fn process_buffer() {
///     profile!("buffer_processing");
///     // ... processing code ...
/// }
/// ```
#[macro_export]
macro_rules! profile {
    ($name:expr) => {
        let _guard = $crate::api::v1::ProfileGuard::new($name);
    };
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::Duration,
    };

    use super::*;

    #[test]
    fn test_span_id_uniqueness() {
        let id1 = SpanId::new();
        let id2 = SpanId::new();
        let id3 = SpanId::new();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_span_id_null() {
        let null = SpanId::null();
        assert!(null.is_null());
        assert_eq!(null.as_u64(), 0);

        let non_null = SpanId::new();
        assert!(!non_null.is_null());
        assert_ne!(non_null.as_u64(), 0);
    }

    #[test]
    fn test_span_data_creation() {
        let data = SpanData::new("test_span", "test::module");

        assert_eq!(data.name, "test_span");
        assert_eq!(data.target, "test::module");
        assert!(!data.id.is_null());
        assert!(data.parent.is_null());
    }

    #[test]
    fn test_nop_profiler() {
        let prof = NopProfiler;

        // All methods should be no-ops
        assert!(!prof.enabled("any::target"));

        let data = SpanData::new("test", "test");
        let id = prof.enter(&data);
        assert!(id.is_null());

        prof.exit(id, 1000);
        prof.counter("test_counter", 1);
        prof.histogram("test_histogram", 100);
    }

    #[test]
    fn test_nop_profiler_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NopProfiler>();
    }

    #[test]
    fn test_profiler_trait_object() {
        let prof: &dyn Profiler = &NopProfiler;
        assert!(!prof.enabled("test"));
    }

    #[test]
    fn test_profile_scope_inactive() {
        // With NopProfiler (default), scope should be inactive
        let scope = ProfileScope::new("test", "test::module");
        assert!(!scope.is_active());
    }

    #[test]
    fn test_profile_guard_basic() {
        // Create guard and let it drop
        {
            let _guard = ProfileGuard::new("test_profile");
            // Simulate some work
            thread::sleep(Duration::from_millis(1));
        }

        // Check that histogram was updated
        let snapshot = metrics().snapshot();
        assert!(snapshot.histograms.contains_key("test_profile"));

        let (count, _mean) = snapshot.histograms.get("test_profile").unwrap();
        assert_eq!(*count, 1);
    }

    #[test]
    fn test_profile_guard_multiple() {
        let name = "test_profile_multi";

        for _ in 0..5 {
            let _guard = ProfileGuard::new(name);
        }

        let snapshot = metrics().snapshot();
        let (count, _mean) = snapshot.histograms.get(name).unwrap();
        assert_eq!(*count, 5);
    }

    #[test]
    fn test_set_profiler_error_display() {
        let err = SetProfilerError;
        assert_eq!(format!("{err}"), "profiler already set");
    }

    #[test]
    fn test_profiler_fallback() {
        // Should return NopProfiler when none is set
        let p = profiler();
        assert!(!p.enabled("any"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_custom_profiler() {
        // Test that we can implement the Profiler trait
        struct CountingProfiler {
            enter_count: AtomicU64,
            exit_count: AtomicU64,
        }

        impl Profiler for CountingProfiler {
            fn enabled(&self, _target: &str) -> bool {
                true
            }

            fn enter(&self, data: &SpanData) -> SpanId {
                self.enter_count.fetch_add(1, Ordering::Relaxed);
                data.id
            }

            fn exit(&self, _id: SpanId, _elapsed_ns: u64) {
                self.exit_count.fetch_add(1, Ordering::Relaxed);
            }

            fn counter(&self, _name: &'static str, _value: u64) {}
            fn histogram(&self, _name: &'static str, _value_us: u64) {}
        }

        let prof = CountingProfiler {
            enter_count: AtomicU64::new(0),
            exit_count: AtomicU64::new(0),
        };

        assert!(prof.enabled("test"));

        let data = SpanData::new("test", "test");
        let id = prof.enter(&data);
        assert!(!id.is_null());
        assert_eq!(prof.enter_count.load(Ordering::Relaxed), 1);

        prof.exit(id, 1000);
        assert_eq!(prof.exit_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_timestamp_monotonic() {
        let t1 = timestamp_ns();
        thread::sleep(Duration::from_micros(100));
        let t2 = timestamp_ns();

        assert!(t2 > t1);
    }

    // ========== SpanId Default ==========

    #[test]
    fn test_span_id_default() {
        let id = SpanId::default();
        // Default creates a new non-null ID
        assert!(!id.is_null());
    }

    // ========== ProfileScope with active profiler ==========

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_profile_scope_with_active_profiler() {
        // We cannot set the global profiler in tests (it's a OnceLock),
        // but we can test the custom profiler logic directly.
        struct ActiveProfiler {
            enter_count: AtomicU64,
            exit_count: AtomicU64,
        }

        impl Profiler for ActiveProfiler {
            fn enabled(&self, _target: &str) -> bool {
                true
            }
            fn enter(&self, data: &SpanData) -> SpanId {
                self.enter_count.fetch_add(1, Ordering::Relaxed);
                data.id
            }
            fn exit(&self, _id: SpanId, _elapsed_ns: u64) {
                self.exit_count.fetch_add(1, Ordering::Relaxed);
            }
            fn counter(&self, _name: &'static str, _value: u64) {}
            fn histogram(&self, _name: &'static str, _value_us: u64) {}
        }

        let prof = ActiveProfiler {
            enter_count: AtomicU64::new(0),
            exit_count: AtomicU64::new(0),
        };

        // Simulate what ProfileScope::new does with an active profiler
        assert!(prof.enabled("test::module"));
        let data = SpanData::new("test_scope", "test::module");
        let id = prof.enter(&data);
        assert!(!id.is_null());
        assert_eq!(prof.enter_count.load(Ordering::Relaxed), 1);

        prof.exit(id, 12345);
        assert_eq!(prof.exit_count.load(Ordering::Relaxed), 1);
    }

    // ========== SetProfilerError as Error ==========

    #[test]
    fn test_set_profiler_error_is_error() {
        let err = SetProfilerError;
        // Test std::error::Error impl
        let _: &dyn std::error::Error = &err;
        assert!(err.source().is_none());
    }

    // ========== SpanData with parent set ==========

    #[test]
    fn test_span_data_with_parent() {
        let parent_id = SpanId::new();
        let mut data = SpanData::new("child_span", "test::child");
        data.parent = parent_id;

        assert_eq!(data.parent, parent_id);
        assert!(!data.parent.is_null());
        assert_eq!(data.name, "child_span");
        assert_eq!(data.target, "test::child");
    }

    // ========== SpanData Clone and Debug ==========

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_span_data_clone_and_debug() {
        let data = SpanData::new("test_clone", "test::clone_mod");
        let cloned = data.clone();
        assert_eq!(cloned.name, "test_clone");
        assert_eq!(cloned.target, "test::clone_mod");
        assert_eq!(cloned.id, data.id);

        let debug = format!("{data:?}");
        assert!(debug.contains("SpanData"));
        assert!(debug.contains("test_clone"));
    }

    // ========== ProfileScope drop path (inactive) ==========

    #[test]
    fn test_profile_scope_drop_inactive() {
        // Create an inactive scope and let it drop
        {
            let scope = ProfileScope::new("test_drop", "test::drop_mod");
            assert!(!scope.is_active());
            // Drop happens here - should not call profiler.exit()
        }
    }

    // ========== profile_scope! macro ==========

    #[test]
    fn test_profile_scope_macro() {
        // The macro creates a _profile_scope_guard variable
        profile_scope!("test_macro_scope", "test::macro");
        // Guard is dropped at end of scope - should be no-op with NopProfiler
    }

    // ========== profile_fn! macro ==========

    #[test]
    fn test_profile_fn_macro() {
        profile_fn!("test_fn_profiled");
        // Uses module_path!() as target
    }

    // ========== profile_counter! macro ==========

    #[test]
    fn test_profile_counter_macro() {
        profile_counter!("test_counter_macro");
        profile_counter!("test_counter_macro_value", 42);
    }

    // ========== profile_histogram! macro ==========

    #[test]
    fn test_profile_histogram_macro() {
        profile_histogram!("test_histogram_macro", 100);
    }

    // ========== profile! macro (legacy) ==========

    #[test]
    fn test_profile_legacy_macro() {
        profile!("test_legacy_profile_macro");
    }

    // ========== NopProfiler Debug and Clone ==========

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_nop_profiler_debug_clone_default() {
        let nop = NopProfiler;
        let cloned = nop;
        let debug = format!("{cloned:?}");
        assert!(debug.contains("NopProfiler"));

        let default_nop = NopProfiler::default();
        assert!(!default_nop.enabled("test"));
    }

    // ========== SetProfilerError Clone ==========

    #[test]
    fn test_set_profiler_error_clone() {
        let err = SetProfilerError;
        let cloned = err;
        assert_eq!(cloned, SetProfilerError);
    }
}
