//! Handler metrics collection infrastructure.
//!
//! Provides low-overhead metrics collection for RPC handler performance tracking.
//! Uses relaxed atomics for minimal overhead (~30-40ns per request).

use std::{
    collections::HashMap,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use reovim_arch::sync::RwLock;

// ============================================================================
// Global Metrics Registry
// ============================================================================

/// Global metrics registry.
static HANDLER_METRICS: OnceLock<RwLock<HashMap<String, HandlerMetrics>>> = OnceLock::new();

/// Global request counter.
static TOTAL_REQUESTS: AtomicU64 = AtomicU64::new(0);

/// Get or initialize the metrics registry.
fn metrics_registry() -> &'static RwLock<HashMap<String, HandlerMetrics>> {
    HANDLER_METRICS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Increment total request counter.
pub fn increment_total_requests() {
    TOTAL_REQUESTS.fetch_add(1, Ordering::Relaxed);
}

/// Get total request count.
#[must_use]
pub fn total_requests() -> u64 {
    TOTAL_REQUESTS.load(Ordering::Relaxed)
}

// ============================================================================
// Handler Metrics
// ============================================================================

/// Metrics for a single handler.
///
/// Uses relaxed atomics for low-overhead tracking.
#[derive(Debug, Default)]
pub struct HandlerMetrics {
    /// Total number of calls.
    calls: AtomicU64,
    /// Total time spent in microseconds.
    total_micros: AtomicU64,
}

impl HandlerMetrics {
    /// Create new metrics.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
            total_micros: AtomicU64::new(0),
        }
    }

    /// Record a completed request.
    pub fn record(&self, elapsed_micros: u64) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.total_micros
            .fetch_add(elapsed_micros, Ordering::Relaxed);
    }

    /// Get call count.
    #[must_use]
    pub fn call_count(&self) -> u64 {
        self.calls.load(Ordering::Relaxed)
    }

    /// Get total time in microseconds.
    #[must_use]
    pub fn total_micros(&self) -> u64 {
        self.total_micros.load(Ordering::Relaxed)
    }

    /// Get average time per call in microseconds.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_micros(&self) -> f64 {
        let calls = self.call_count();
        if calls == 0 {
            0.0
        } else {
            self.total_micros() as f64 / calls as f64
        }
    }
}

// ============================================================================
// Request Timer
// ============================================================================

/// RAII timer for tracking request duration.
///
/// Records elapsed time to the metrics registry when dropped.
pub struct RequestTimer {
    method: String,
    start: Instant,
}

impl RequestTimer {
    /// Start a new timer for the given method.
    #[must_use]
    pub fn start(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            start: Instant::now(),
        }
    }
}

impl Drop for RequestTimer {
    #[allow(clippy::cast_possible_truncation)]
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_micros() as u64;

        // Record to registry
        let registry = metrics_registry();
        {
            // Try read lock first
            let read_guard = registry.read();
            if let Some(metrics) = read_guard.get(&self.method) {
                metrics.record(elapsed);
                return;
            }
        }
        // Need to create entry - acquire write lock
        let mut write_guard = registry.write();
        // Double-check after acquiring write lock
        if let Some(metrics) = write_guard.get(&self.method) {
            metrics.record(elapsed);
        } else {
            let metrics = HandlerMetrics::new();
            metrics.record(elapsed);
            write_guard.insert(self.method.clone(), metrics);
        }
    }
}

// ============================================================================
// Snapshot Functions
// ============================================================================

/// Snapshot of handler metrics.
#[derive(Debug, Clone)]
pub struct HandlerMetricsSnapshot {
    /// Method name.
    pub method: String,
    /// Call count.
    pub call_count: u64,
    /// Total time in microseconds.
    pub total_micros: u64,
    /// Average time per call.
    pub avg_micros: f64,
}

/// Get snapshot of all handler metrics.
#[must_use]
pub fn snapshot_handler_metrics() -> Vec<HandlerMetricsSnapshot> {
    let registry = metrics_registry();
    let guard = registry.read();

    guard
        .iter()
        .map(|(method, metrics)| HandlerMetricsSnapshot {
            method: method.clone(),
            call_count: metrics.call_count(),
            total_micros: metrics.total_micros(),
            avg_micros: metrics.avg_micros(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_metrics_new() {
        let metrics = HandlerMetrics::new();
        assert_eq!(metrics.call_count(), 0);
        assert_eq!(metrics.total_micros(), 0);
        assert!((metrics.avg_micros() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_handler_metrics_record() {
        let metrics = HandlerMetrics::new();
        metrics.record(100);
        metrics.record(200);

        assert_eq!(metrics.call_count(), 2);
        assert_eq!(metrics.total_micros(), 300);
        assert!((metrics.avg_micros() - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_request_timer() {
        let method = "test/method";
        {
            let _timer = RequestTimer::start(method);
            // Simulate some work
            std::thread::sleep(std::time::Duration::from_micros(100));
        }

        // After timer drops, metrics should be recorded
        let snapshots = snapshot_handler_metrics();
        let found = snapshots.iter().any(|s| s.method == method);
        assert!(found);
    }

    #[test]
    fn test_total_requests_counter() {
        let initial = total_requests();
        increment_total_requests();
        increment_total_requests();
        assert!(total_requests() >= initial + 2);
    }
}
