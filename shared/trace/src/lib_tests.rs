use super::*;

#[test]
fn test_tracing_profiler_disabled_by_default() {
    // Without REOVIM_PROFILE set, profiler should be disabled
    let prof = TracingProfiler::disabled();
    assert!(!prof.enabled("any::target"));
}

#[test]
fn test_tracing_profiler_disabled_filter_is_none() {
    let prof = TracingProfiler::disabled();
    assert!(!prof.enabled(""));
    assert!(!prof.enabled("runner"));
    assert!(!prof.enabled("mm::buffer"));
}

#[test]
fn test_tracing_profiler_default() {
    // Default uses environment variable. In test environment,
    // REOVIM_PROFILE is typically not set, so it should be disabled.
    // (We can't guarantee the env var state in all test runners,
    // so just verify it doesn't panic)
    let _prof = TracingProfiler::default();
}

#[test]
fn test_tracing_profiler_filter_empty() {
    let prof = TracingProfiler::with_filter("");
    // Empty filter means all enabled
    assert!(prof.enabled("any::target"));
    assert!(prof.enabled("runner::input"));
    assert!(prof.enabled(""));
}

#[test]
fn test_tracing_profiler_filter_one() {
    let prof = TracingProfiler::with_filter("1");
    // "1" means all enabled
    assert!(prof.enabled("any::target"));
    assert!(prof.enabled(""));
    assert!(prof.enabled("runner::input"));
}

#[test]
fn test_tracing_profiler_filter_prefix() {
    let prof = TracingProfiler::with_filter("runner");
    assert!(prof.enabled("runner"));
    assert!(prof.enabled("runner::input"));
    assert!(prof.enabled("runner::keymap"));
    assert!(!prof.enabled("mm::buffer"));
    assert!(!prof.enabled("other"));
    assert!(!prof.enabled(""));
    assert!(!prof.enabled("run")); // "run" does not start with "runner"
}

#[test]
fn test_tracing_profiler_filter_prefix_exact_boundary() {
    let prof = TracingProfiler::with_filter("mm");
    assert!(prof.enabled("mm"));
    assert!(prof.enabled("mm::buffer"));
    assert!(prof.enabled("mm::allocator"));
    // "mmap".starts_with("mm") is true, so this matches
    assert!(prof.enabled("mmap"));
    assert!(!prof.enabled("runner"));
    assert!(!prof.enabled("m"));
}

#[test]
fn test_tracing_profiler_filter_long_prefix() {
    let prof = TracingProfiler::with_filter("runner::input::keymap");
    assert!(prof.enabled("runner::input::keymap"));
    assert!(prof.enabled("runner::input::keymap::vim"));
    assert!(!prof.enabled("runner::input"));
    assert!(!prof.enabled("runner"));
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
fn test_tracing_profiler_enter_exit_multiple() {
    let prof = TracingProfiler::with_filter("test");

    // Enter and exit multiple sequential spans
    for i in 0u64..5 {
        let name = if i % 2 == 0 { "even_span" } else { "odd_span" };
        let data = SpanData::new(name, "test::module");
        let id = prof.enter(&data);
        prof.exit(id, i * 100);
    }
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
fn test_tracing_profiler_counter_when_disabled() {
    let prof = TracingProfiler::disabled();

    // Counter and histogram should be no-ops when disabled (filter is None)
    prof.counter("test_counter_disabled", 1);
    prof.counter("test_counter_disabled", 100);
    prof.histogram("test_histogram_disabled", 42);
    // Should not panic
}

#[test]
fn test_tracing_profiler_counter_zero_value() {
    let prof = TracingProfiler::with_filter("1");
    prof.counter("zero_counter", 0);
    prof.histogram("zero_histogram", 0);
}

#[test]
fn test_tracing_profiler_counter_large_value() {
    let prof = TracingProfiler::with_filter("1");
    prof.counter("large_counter", u64::MAX);
    prof.histogram("large_histogram", u64::MAX);
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
fn test_span_stack_multiple_underflow() {
    // Multiple exits without enters should all be no-ops
    let prof = TracingProfiler::with_filter("1");
    prof.exit(SpanId::new(), 100);
    prof.exit(SpanId::new(), 200);
    prof.exit(SpanId::new(), 300);
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

#[test]
fn test_deeply_nested_spans() {
    let prof = TracingProfiler::with_filter("test");

    let mut ids = Vec::new();
    for i in 0..10 {
        let name = if i % 2 == 0 { "even" } else { "odd" };
        let data = SpanData::new(name, "test::deep");
        let id = prof.enter(&data);
        ids.push(id);
    }

    // Exit in reverse order (LIFO)
    for (i, id) in ids.into_iter().rev().enumerate() {
        prof.exit(id, u64::try_from(i).unwrap_or(0) * 10);
    }
}

#[test]
fn test_enter_returns_span_id_from_data() {
    let prof = TracingProfiler::with_filter("test");

    let data = SpanData::new("my_span", "test");
    let returned_id = prof.enter(&data);

    // The profiler should return the same ID that was in SpanData
    assert_eq!(returned_id, data.id);

    prof.exit(returned_id, 0);
}

#[test]
fn test_enabled_delegates_to_matches_filter() {
    // Verify that enabled() calls matches_filter() correctly
    let prof = TracingProfiler::with_filter("runner");
    assert!(prof.enabled("runner::input")); // matches_filter returns true
    assert!(!prof.enabled("mm::buffer")); // matches_filter returns false
}

#[test]
fn test_exit_with_zero_elapsed() {
    let prof = TracingProfiler::with_filter("test");

    let data = SpanData::new("zero_time", "test");
    let id = prof.enter(&data);
    prof.exit(id, 0);
}

#[test]
fn test_exit_with_large_elapsed() {
    let prof = TracingProfiler::with_filter("test");

    let data = SpanData::new("long_span", "test");
    let id = prof.enter(&data);
    prof.exit(id, u64::MAX);
}

// ========================================================================
// Re-exports Tests
// ========================================================================

#[test]
fn test_reexports_available() {
    // Verify that re-exported types are accessible
    fn assert_type_exists<T>() {}
    assert_type_exists::<KernelSpanId>();
    assert_type_exists::<KernelSpanData>();
    assert_type_exists::<KernelNopProfiler>();
}

#[test]
fn test_reexport_nop_profiler() {
    let nop = KernelNopProfiler;
    assert!(!nop.enabled("any"));

    let data = KernelSpanData::new("test", "test");
    let id = nop.enter(&data);
    assert!(id.is_null());
    nop.exit(id, 0);
}

#[test]
fn test_reexport_profiler_fn() {
    // profiler() returns the global profiler (NopProfiler if none set)
    let p = profiler();
    assert!(!p.enabled("test"));
}

// ========================================================================
// init_profiling Tests
// ========================================================================

// Note: init_profiling() uses a OnceLock and set_profiler() which can
// only be set once per process. We cannot test it multiple times in the
// same test binary. We test init_profiling_with_filter instead which
// delegates to init_profiling.

#[test]
fn test_init_profiling_with_filter_delegates() {
    // init_profiling_with_filter currently ignores the filter and
    // delegates to init_profiling. We can only verify it doesn't panic.
    // The actual set_profiler might fail if already set by another test,
    // but the function should handle that gracefully.
    let _ = init_profiling_with_filter("test");
}

#[test]
fn test_init_profiling_idempotent() {
    // Calling init_profiling multiple times should not panic.
    // Second call returns Err because profiler is already set.
    let result1 = init_profiling();
    let result2 = init_profiling();
    // At least one should succeed, the other may fail
    // (depends on test ordering)
    let _ = result1;
    let _ = result2;
}

// ========================================================================
// Matches Filter Edge Cases
// ========================================================================

#[test]
fn test_matches_filter_empty_target() {
    let prof = TracingProfiler::with_filter("runner");
    assert!(!prof.matches_filter(""));
}

#[test]
fn test_matches_filter_target_equals_filter() {
    let prof = TracingProfiler::with_filter("runner");
    assert!(prof.matches_filter("runner"));
}

#[test]
fn test_matches_filter_all_enabled_empty_target() {
    let prof = TracingProfiler::with_filter("");
    assert!(prof.matches_filter(""));
}

#[test]
fn test_matches_filter_all_enabled_with_1() {
    let prof = TracingProfiler::with_filter("1");
    assert!(prof.matches_filter("anything"));
}

#[test]
fn test_matches_filter_none() {
    let prof = TracingProfiler::disabled();
    assert!(!prof.matches_filter("anything"));
    assert!(!prof.matches_filter(""));
}

// ========================================================================
// PROFILE_ENV_VAR Constant Test
// ========================================================================

#[test]
fn test_profile_env_var_constant() {
    assert_eq!(PROFILE_ENV_VAR, "REOVIM_PROFILE");
}

// The Ok(val) branch requires REOVIM_PROFILE env var to be set,
// which needs unsafe set_var in Rust 2024 (deny(unsafe_code)).
#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_profiling_filter_returns_option() {
    // Instead, we verify the function runs and returns a valid Option.
    let filter = profiling_filter();
    // Whether set or not, the result should be consistent with env var
    match std::env::var(PROFILE_ENV_VAR) {
        Ok(val) => assert_eq!(filter, Some(val)),
        Err(_) => assert!(filter.is_none()),
    }
}

#[test]
fn test_enter_with_trace_subscriber_covers_span_fields() {
    // Set up a TRACE-level subscriber so span! macro body executes
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_test_writer()
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let prof = TracingProfiler::with_filter("test");
    let data = SpanData::new("coverage_span", "test::target");
    let id = prof.enter(&data);
    assert_eq!(id, data.id);
    prof.exit(id, 42);
}
