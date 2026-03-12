#![allow(deprecated)]

use {
    super::super::{profiler::timestamp_ns, *},
    std::{
        error::Error,
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::Duration,
    },
};

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
    crate::profile_scope!("test_macro_scope", "test::macro");
    // Guard is dropped at end of scope - should be no-op with NopProfiler
}

// ========== profile_fn! macro ==========

#[test]
fn test_profile_fn_macro() {
    crate::profile_fn!("test_fn_profiled");
    // Uses module_path!() as target
}

// ========== profile_counter! macro ==========

#[test]
fn test_profile_counter_macro() {
    crate::profile_counter!("test_counter_macro");
    crate::profile_counter!("test_counter_macro_value", 42);
}

// ========== profile_histogram! macro ==========

#[test]
fn test_profile_histogram_macro() {
    crate::profile_histogram!("test_histogram_macro", 100);
}

// ========== profile! macro (legacy) ==========

#[test]
fn test_profile_legacy_macro() {
    crate::profile!("test_legacy_profile_macro");
}

// ========== NopProfiler Debug and Clone ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_nop_profiler_debug_clone_default() {
    let nop = NopProfiler;
    let cloned = nop;
    let debug = format!("{cloned:?}");
    assert!(debug.contains("NopProfiler"));

    let default_nop = NopProfiler;
    assert!(!default_nop.enabled("test"));
}

// ========== SetProfilerError Clone ==========

#[test]
fn test_set_profiler_error_clone() {
    let err = SetProfilerError;
    let cloned = err;
    assert_eq!(cloned, SetProfilerError);
}
