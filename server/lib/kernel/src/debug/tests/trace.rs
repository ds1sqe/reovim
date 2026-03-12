use super::super::*;

#[test]
fn test_tracepoint_enable_disable() {
    let tp = TracePoint::new("test", "test_module");
    assert!(!tp.is_enabled());

    tp.enable();
    assert!(tp.is_enabled());

    tp.disable();
    assert!(!tp.is_enabled());
}

#[test]
fn test_trace_event_creation() {
    let event = TraceEvent {
        timestamp_ns: 12345,
        tracepoint: "test_tp",
        module: "test_mod",
        message: "test message".to_string(),
    };

    assert_eq!(event.tracepoint, "test_tp");
    assert_eq!(event.module, "test_mod");
    assert_eq!(event.message, "test message");
}

#[test]
fn test_emit_trace_no_sink() {
    // Should not panic when no sink is set
    emit_trace("test", "test_mod", "test message".to_string());
}

// ========== TracePoint fields access ==========

#[test]
fn test_tracepoint_fields() {
    let tp = TracePoint::new("my_trace", "my_module");
    assert_eq!(tp.name, "my_trace");
    assert_eq!(tp.module, "my_module");
    assert!(!tp.is_enabled());
}

// ========== TraceSink trait ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_trace_sink_impl() {
    use std::sync::{Arc, Mutex};

    struct TestSink {
        events: Mutex<Vec<String>>,
    }

    impl TraceSink for TestSink {
        fn emit(&self, event: TraceEvent) {
            self.events.lock().unwrap().push(event.message);
        }
    }

    let sink = Arc::new(TestSink {
        events: Mutex::new(Vec::new()),
    });

    // Verify the trait can be implemented and used
    let event = TraceEvent {
        timestamp_ns: 0,
        tracepoint: "test_tp",
        module: "test_mod",
        message: "hello trace".to_string(),
    };
    sink.emit(event);

    let count = sink.events.lock().unwrap().len();
    assert_eq!(count, 1);
    let first = sink.events.lock().unwrap()[0].clone();
    assert_eq!(first, "hello trace");
}

// ========== set_trace_sink ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_set_trace_sink_called() {
    // We can't actually set the global sink reliably in tests
    // (it's a OnceLock and might already be set by another test).
    // But we can at least call it and verify it doesn't panic.
    struct DummySink;
    impl TraceSink for DummySink {
        fn emit(&self, _event: TraceEvent) {}
    }

    // This might succeed or fail depending on test order, but should not panic
    set_trace_sink(Box::new(DummySink));
}

// ========== TraceEvent Clone and Debug ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_trace_event_clone_and_debug() {
    let event = TraceEvent {
        timestamp_ns: 999,
        tracepoint: "tp",
        module: "mod",
        message: "msg".to_string(),
    };
    let cloned = event.clone();
    assert_eq!(cloned.timestamp_ns, 999);
    assert_eq!(cloned.tracepoint, "tp");
    assert_eq!(cloned.module, "mod");

    let debug = format!("{event:?}");
    assert!(debug.contains("TraceEvent"));
}

// === Coverage: TracePoint Send + Sync ===

#[test]
fn test_tracepoint_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TracePoint>();
}

// === Coverage: define_tracepoint macro ===

#[test]
fn test_define_tracepoint_macro() {
    crate::define_tracepoint!(TP_TEST_MACRO, "test_module");
    assert_eq!(TP_TEST_MACRO.name, "TP_TEST_MACRO");
    assert_eq!(TP_TEST_MACRO.module, "test_module");
    assert!(!TP_TEST_MACRO.is_enabled());
}

// === Coverage: trace_event macro (disabled tracepoint) ===

#[test]
fn test_trace_event_macro_disabled() {
    crate::define_tracepoint!(TP_TEST_DISABLED, "test_module");
    // Should not panic even when tracepoint is disabled
    crate::trace_event!(TP_TEST_DISABLED, "test message {}", 42);
}

// === Coverage: trace_event macro (enabled tracepoint) ===

#[test]
fn test_trace_event_macro_enabled() {
    crate::define_tracepoint!(TP_TEST_ENABLED, "test_module");
    TP_TEST_ENABLED.enable();
    // Should emit trace event (sink may or may not be set)
    crate::trace_event!(TP_TEST_ENABLED, "test message {}", 42);
    TP_TEST_ENABLED.disable();
}

// === Coverage: emit_trace with sink set ===

#[test]
fn test_emit_trace_with_sink_set() {
    // Try setting the global sink (may already be set by another test)
    struct CountingSink;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl TraceSink for CountingSink {
        fn emit(&self, _event: TraceEvent) {}
    }
    set_trace_sink(Box::new(CountingSink));

    // Now emit_trace should go through the Some(sink) path
    emit_trace("test_tp", "test_mod", "hello sink".to_string());
}
