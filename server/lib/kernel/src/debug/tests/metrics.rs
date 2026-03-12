use super::super::*;

#[test]
fn test_counter_basic() {
    let counter = Counter::new();
    assert_eq!(counter.get(), 0);

    counter.increment();
    assert_eq!(counter.get(), 1);

    counter.add(5);
    assert_eq!(counter.get(), 6);

    counter.reset();
    assert_eq!(counter.get(), 0);
}

#[test]
#[allow(clippy::float_cmp)] // Exact comparison OK for 0.0
fn test_histogram_basic() {
    let hist = Histogram::new();
    assert_eq!(hist.count(), 0);
    assert_eq!(hist.mean(), 0.0);

    hist.record(10);
    hist.record(20);
    hist.record(30);

    assert_eq!(hist.count(), 3);
    assert_eq!(hist.sum(), 60);
    assert!((hist.mean() - 20.0).abs() < 0.001);
}

#[test]
fn test_histogram_buckets() {
    let hist = Histogram::new();

    // Record values that should fall into different buckets
    hist.record(0); // bucket 0
    hist.record(1); // bucket 1
    hist.record(2); // bucket 2
    hist.record(4); // bucket 3
    hist.record(8); // bucket 4

    let buckets = hist.buckets();
    assert_eq!(buckets[0], 1);
    assert_eq!(buckets[1], 1);
    assert_eq!(buckets[2], 1);
    assert_eq!(buckets[3], 1);
    assert_eq!(buckets[4], 1);
}

#[test]
fn test_registry() {
    let registry = MetricsRegistry::new();

    // Get counter and increment
    let counter = registry.counter("test_counter");
    counter.increment();
    counter.increment();

    // Get histogram and record
    let hist = registry.histogram("test_histogram");
    hist.record(100);

    // Take snapshot
    let snapshot = registry.snapshot();
    assert_eq!(snapshot.counters.get("test_counter"), Some(&2));
    assert!(snapshot.histograms.contains_key("test_histogram"));
}

#[test]
fn test_global_metrics() {
    let registry = metrics();

    // Should be able to get metrics from global registry
    let counter = registry.counter("global_test");
    counter.increment();

    let snapshot = registry.snapshot();
    assert!(snapshot.counters.contains_key("global_test"));
}

// ========== Default impls ==========

#[test]
fn test_counter_default() {
    let counter = Counter::default();
    assert_eq!(counter.get(), 0);
}

#[test]
fn test_histogram_default() {
    let hist = Histogram::default();
    assert_eq!(hist.count(), 0);
    assert_eq!(hist.sum(), 0);
}

#[test]
fn test_metrics_registry_default() {
    let registry = MetricsRegistry::default();
    let snapshot = registry.snapshot();
    assert!(snapshot.counters.is_empty());
    assert!(snapshot.histograms.is_empty());
}

// ========== Histogram reset ==========

#[test]
fn test_histogram_reset() {
    let hist = Histogram::new();
    hist.record(10);
    hist.record(20);
    assert_eq!(hist.count(), 2);
    assert_eq!(hist.sum(), 30);

    hist.reset();
    assert_eq!(hist.count(), 0);
    assert_eq!(hist.sum(), 0);
    let buckets = hist.buckets();
    assert!(buckets.iter().all(|&b| b == 0));
}

// ========== Registry cache hit (second call returns same) ==========

#[test]
fn test_registry_counter_cache_hit() {
    let registry = MetricsRegistry::new();
    let counter1 = registry.counter("cached");
    counter1.increment();

    // Second call should return the same counter (cache hit in read path)
    let counter2 = registry.counter("cached");
    assert_eq!(counter2.get(), 1);
}

#[test]
fn test_registry_histogram_cache_hit() {
    let registry = MetricsRegistry::new();
    let hist1 = registry.histogram("cached_hist");
    hist1.record(42);

    // Second call should return the same histogram (cache hit in read path)
    let hist2 = registry.histogram("cached_hist");
    assert_eq!(hist2.count(), 1);
    assert_eq!(hist2.sum(), 42);
}
