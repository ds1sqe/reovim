//! Metrics collection infrastructure.
//!
//! Linux equivalent: `kernel/trace/ftrace` (function tracing metrics)
//!
//! Provides counters and histograms for performance monitoring.
//! All operations are lock-free using atomics for minimal overhead.

use std::{
    collections::HashMap,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use reovim_arch::sync::RwLock;

/// Counter for tracking occurrences.
///
/// Uses atomic operations for lock-free updates.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::debug::Counter;
///
/// static BUFFER_CREATES: Counter = Counter::new();
///
/// fn create_buffer() {
///     BUFFER_CREATES.increment();
///     // ...
/// }
/// ```
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    /// Create a new counter initialized to zero.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    /// Increment the counter by 1.
    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Add a value to the counter.
    pub fn add(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Get the current counter value.
    #[must_use]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset the counter to zero.
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

/// Histogram for tracking value distributions.
///
/// Uses power-of-two bucket boundaries for efficient indexing.
/// Bucket boundaries: 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, inf
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::debug::Histogram;
///
/// static RESPONSE_TIMES: Histogram = Histogram::new();
///
/// fn handle_request() {
///     let start = std::time::Instant::now();
///     // ... handle request ...
///     RESPONSE_TIMES.record(start.elapsed().as_micros() as u64);
/// }
/// ```
pub struct Histogram {
    /// 16 buckets with power-of-two boundaries.
    buckets: [AtomicU64; 16],
    /// Sum of all recorded values.
    sum: AtomicU64,
    /// Count of all recorded values.
    count: AtomicU64,
}

impl Histogram {
    /// Create a new histogram.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buckets: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// Record a value in the histogram.
    ///
    /// Value 0 goes to bucket 0.
    /// Values 1-1 go to bucket 1 (2^0).
    /// Values 2-3 go to bucket 2 (2^1).
    /// And so on...
    pub fn record(&self, value: u64) {
        // Calculate bucket index using leading zeros
        // For value 0, this gives bucket 0
        // For values 1-1, bucket 1
        // For values 2-3, bucket 2
        // etc.
        let bucket = if value == 0 {
            0
        } else {
            (64 - value.leading_zeros()).min(15) as usize
        };

        self.buckets[bucket].fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the mean value of all recorded samples.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Acceptable for metrics
    pub fn mean(&self) -> f64 {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            self.sum.load(Ordering::Relaxed) as f64 / count as f64
        }
    }

    /// Get the total count of recorded samples.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get the sum of all recorded values.
    #[must_use]
    pub fn sum(&self) -> u64 {
        self.sum.load(Ordering::Relaxed)
    }

    /// Get bucket counts.
    #[must_use]
    pub fn buckets(&self) -> [u64; 16] {
        let mut result = [0u64; 16];
        for (i, bucket) in self.buckets.iter().enumerate() {
            result[i] = bucket.load(Ordering::Relaxed);
        }
        result
    }

    /// Reset all histogram data.
    pub fn reset(&self) {
        for bucket in &self.buckets {
            bucket.store(0, Ordering::Relaxed);
        }
        self.sum.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics registry for named metrics.
///
/// Provides a centralized registry for counters and histograms.
/// Uses `Arc` for safe shared access without raw pointers.
pub struct MetricsRegistry {
    counters: RwLock<HashMap<&'static str, std::sync::Arc<Counter>>>,
    histograms: RwLock<HashMap<&'static str, std::sync::Arc<Histogram>>>,
}

impl MetricsRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a counter by name.
    ///
    /// Returns an `Arc<Counter>` that can be used to update the counter.
    #[must_use]
    pub fn counter(&self, name: &'static str) -> std::sync::Arc<Counter> {
        // First try read-only access
        {
            let counters = self.counters.read();
            if let Some(counter) = counters.get(name) {
                return counter.clone();
            }
        }

        // Need to insert - acquire write lock
        let mut counters = self.counters.write();
        counters
            .entry(name)
            .or_insert_with(|| std::sync::Arc::new(Counter::new()))
            .clone()
    }

    /// Get or create a histogram by name.
    #[must_use]
    pub fn histogram(&self, name: &'static str) -> std::sync::Arc<Histogram> {
        {
            let histograms = self.histograms.read();
            if let Some(histogram) = histograms.get(name) {
                return histogram.clone();
            }
        }

        let mut histograms = self.histograms.write();
        histograms
            .entry(name)
            .or_insert_with(|| std::sync::Arc::new(Histogram::new()))
            .clone()
    }

    /// Take a snapshot of all metrics.
    #[must_use]
    pub fn snapshot(&self) -> MetricsSnapshot {
        let counters = self.counters.read();
        let histograms = self.histograms.read();

        MetricsSnapshot {
            counters: counters.iter().map(|(k, v)| (*k, v.get())).collect(),
            histograms: histograms
                .iter()
                .map(|(k, v)| (*k, (v.count(), v.mean())))
                .collect(),
        }
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of all metrics at a point in time.
#[derive(Debug)]
pub struct MetricsSnapshot {
    /// Counter name -> current value.
    pub counters: HashMap<&'static str, u64>,
    /// Histogram name -> (count, mean).
    pub histograms: HashMap<&'static str, (u64, f64)>,
}

/// Global metrics registry.
static METRICS: OnceLock<MetricsRegistry> = OnceLock::new();

/// Get the global metrics registry.
#[must_use]
pub fn metrics() -> &'static MetricsRegistry {
    METRICS.get_or_init(MetricsRegistry::new)
}

