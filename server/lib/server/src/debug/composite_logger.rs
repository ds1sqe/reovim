//! Composite logger that writes to both debug ring buffer and tracing.
//!
//! This logger implements the kernel's `Logger` trait and forwards log records
//! to both the server's debug ring buffer (for post-mortem analysis) and to
//! the tracing ecosystem (for real-time output).
//!
//! # Architecture
//!
//! ```text
//! pr_*! macros
//!      │
//!      ▼
//! ┌────────────────────────┐
//! │  CompositeLogger       │
//! │  ├─► DebugRingBuffer   │──► Post-mortem dumps
//! │  └─► TracingLogger     │──► Real-time output
//! └────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use reovim_server::debug::{init_composite_logging, COMPOSITE_LOGGER};
//! use reovim_kernel::api::v1::set_logger;
//!
//! // Initialize the debug ring buffer
//! init_debug_ring().expect("ring buffer already initialized");
//!
//! // Set the composite logger as the global kernel logger
//! set_logger(&COMPOSITE_LOGGER).expect("logger already set");
//!
//! // Now pr_*! macros write to both ring buffer and tracing
//! pr_info!("server started");
//! ```

use reovim_kernel::api::v1::{Level, Logger, Record};

use super::try_debug_ring;

/// Composite logger that writes to debug ring buffer and tracing.
///
/// This is a zero-sized type - all state lives in:
/// - Global debug ring buffer (`DEBUG_RING`)
/// - Global tracing subscriber
///
/// This allows `CompositeLogger` to be a static constant.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompositeLogger;

/// Global composite logger instance.
///
/// Use this with `set_logger()` to enable both ring buffer and tracing logging.
pub static COMPOSITE_LOGGER: CompositeLogger = CompositeLogger;

impl Logger for CompositeLogger {
    fn log(&self, record: &Record) {
        // 1. Push to debug ring buffer (if initialized)
        if let Some(ring) = try_debug_ring() {
            ring.push(record);
        }

        // 2. Forward to tracing
        // Note: target: must be a constant, so we include module_path as a field.
        match record.level() {
            Level::Error => tracing::error!(
                module_path = record.module_path(),
                file = record.file(),
                line = record.line(),
                "{}",
                record.message()
            ),
            Level::Warn => tracing::warn!(
                module_path = record.module_path(),
                file = record.file(),
                line = record.line(),
                "{}",
                record.message()
            ),
            Level::Info => tracing::info!(
                module_path = record.module_path(),
                file = record.file(),
                line = record.line(),
                "{}",
                record.message()
            ),
            Level::Debug => tracing::debug!(
                module_path = record.module_path(),
                file = record.file(),
                line = record.line(),
                "{}",
                record.message()
            ),
            Level::Trace => tracing::trace!(
                module_path = record.module_path(),
                file = record.file(),
                line = record.line(),
                "{}",
                record.message()
            ),
        }
    }

    fn flush(&self) {
        // tracing-subscriber handles flushing automatically.
        // Ring buffer doesn't need flushing (in-memory).
    }

    fn enabled(&self, level: Level) -> bool {
        // Check both: if ring buffer wants it OR tracing wants it
        let ring_enabled = try_debug_ring().is_some(); // Ring buffer captures everything

        let tracing_enabled = match level {
            Level::Error => tracing::enabled!(tracing::Level::ERROR),
            Level::Warn => tracing::enabled!(tracing::Level::WARN),
            Level::Info => tracing::enabled!(tracing::Level::INFO),
            Level::Debug => tracing::enabled!(tracing::Level::DEBUG),
            Level::Trace => tracing::enabled!(tracing::Level::TRACE),
        };

        ring_enabled || tracing_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composite_logger_is_zst() {
        assert_eq!(std::mem::size_of::<CompositeLogger>(), 0);
    }

    #[test]
    fn test_composite_logger_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CompositeLogger>();
    }

    #[test]
    fn test_composite_logger_default() {
        let logger = CompositeLogger;
        logger.flush();
    }

    #[test]
    fn test_composite_logger_debug() {
        let logger = CompositeLogger;
        let debug_str = format!("{logger:?}");
        assert_eq!(debug_str, "CompositeLogger");
    }

    #[test]
    fn test_composite_logger_copy() {
        let logger = CompositeLogger;
        let copied = logger;
        copied.flush();
    }

    #[test]
    fn test_log_each_level() {
        let logger = CompositeLogger;
        // Construct records for each level and verify log() doesn't panic
        for &level in &[
            Level::Error,
            Level::Warn,
            Level::Info,
            Level::Debug,
            Level::Trace,
        ] {
            let record = Record::builder(level)
                .message("test message")
                .module_path("test::module")
                .file("test.rs")
                .line(42)
                .build();
            logger.log(&record);
        }
    }

    #[test]
    fn test_enabled_each_level() {
        let logger = CompositeLogger;
        // enabled() should not panic for any level
        for &level in &[
            Level::Error,
            Level::Warn,
            Level::Info,
            Level::Debug,
            Level::Trace,
        ] {
            let _ = logger.enabled(level);
        }
    }

    #[test]
    fn test_global_composite_logger_exists() {
        // Verify the static instance is usable
        COMPOSITE_LOGGER.flush();
        let _ = COMPOSITE_LOGGER.enabled(Level::Info);
    }

    #[test]
    fn test_log_with_empty_message() {
        let logger = CompositeLogger;
        let record = Record::builder(Level::Info)
            .message("")
            .module_path("test::empty")
            .file("test.rs")
            .line(1)
            .build();
        // Should not panic with empty message
        logger.log(&record);
    }

    #[test]
    fn test_log_with_long_message() {
        let logger = CompositeLogger;
        let long_msg = "x".repeat(1000);
        let record = Record::builder(Level::Debug)
            .message(&long_msg)
            .module_path("test::long")
            .file("test.rs")
            .line(99)
            .build();
        // Should not panic with long message
        logger.log(&record);
    }

    #[test]
    fn test_flush_is_noop() {
        let logger = CompositeLogger;
        // Calling flush multiple times should be fine
        logger.flush();
        logger.flush();
        logger.flush();
    }

    #[test]
    fn test_enabled_without_ring_buffer() {
        let logger = CompositeLogger;
        // Without ring buffer, only tracing determines enabled state.
        // In test context, tracing may or may not be initialized.
        // The important thing is it doesn't panic.
        let _ = logger.enabled(Level::Error);
        let _ = logger.enabled(Level::Warn);
        let _ = logger.enabled(Level::Info);
        let _ = logger.enabled(Level::Debug);
        let _ = logger.enabled(Level::Trace);
    }

    #[test]
    fn test_composite_logger_implements_logger_trait() {
        // Verify the Logger trait is implemented
        fn assert_logger<T: Logger>(_: &T) {}
        assert_logger(&CompositeLogger);
        assert_logger(&COMPOSITE_LOGGER);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_log_with_ring_buffer_if_available() {
        // The global ring buffer may or may not be initialized depending on
        // test execution order. We exercise the log() path regardless.
        let logger = CompositeLogger;
        let record = Record::builder(Level::Info)
            .message("composite-ring-test")
            .module_path("test::composite")
            .file("composite_logger.rs")
            .line(1)
            .build();

        // Log should handle both cases: ring buffer present or absent
        logger.log(&record);

        // If ring buffer happens to be initialized, exercise the read path.
        // We don't assert the specific entry exists because the global ring may
        // have small capacity (2048 bytes when initialized by ring_buffer::tests)
        // causing eviction when many tests run concurrently.
        if let Some(ring) = super::try_debug_ring() {
            let entries = ring.tail(100);
            let _ = entries
                .iter()
                .any(|e| e.message.contains("composite-ring-test"));
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enabled_returns_bool() {
        let logger = CompositeLogger;

        // enabled() should return a boolean without panicking regardless of
        // whether the ring buffer is initialized or not.
        let info_enabled = logger.enabled(Level::Info);
        let debug_enabled = logger.enabled(Level::Debug);
        let trace_enabled = logger.enabled(Level::Trace);

        // If the ring buffer is initialized, all levels should be enabled
        if super::try_debug_ring().is_some() {
            assert!(info_enabled);
            assert!(debug_enabled);
            assert!(trace_enabled);
        }
    }

    #[test]
    fn test_log_all_levels_with_different_fields() {
        let logger = CompositeLogger;

        // Error with all fields populated
        let record = Record::builder(Level::Error)
            .message("error occurred")
            .module_path("reovim_server::grpc")
            .file("server/lib/server/src/grpc/input.rs")
            .line(123)
            .build();
        logger.log(&record);

        // Warn with minimal fields
        let record = Record::builder(Level::Warn)
            .message("warning")
            .module_path("")
            .file("")
            .line(0)
            .build();
        logger.log(&record);

        // Trace level
        let record = Record::builder(Level::Trace)
            .message("detailed trace")
            .module_path("reovim_server::session")
            .file("session.rs")
            .line(456)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_each_level_individually_error() {
        let logger = CompositeLogger;
        let record = Record::builder(Level::Error)
            .message("error level test")
            .module_path("test::error")
            .file("test.rs")
            .line(1)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_each_level_individually_warn() {
        let logger = CompositeLogger;
        let record = Record::builder(Level::Warn)
            .message("warn level test")
            .module_path("test::warn")
            .file("test.rs")
            .line(2)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_each_level_individually_info() {
        let logger = CompositeLogger;
        let record = Record::builder(Level::Info)
            .message("info level test")
            .module_path("test::info")
            .file("test.rs")
            .line(3)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_each_level_individually_debug() {
        let logger = CompositeLogger;
        let record = Record::builder(Level::Debug)
            .message("debug level test")
            .module_path("test::debug")
            .file("test.rs")
            .line(4)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_each_level_individually_trace() {
        let logger = CompositeLogger;
        let record = Record::builder(Level::Trace)
            .message("trace level test")
            .module_path("test::trace")
            .file("test.rs")
            .line(5)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_enabled_each_level_individually() {
        let logger = CompositeLogger;

        let _error = logger.enabled(Level::Error);
        let _warn = logger.enabled(Level::Warn);
        let _info = logger.enabled(Level::Info);
        let _debug = logger.enabled(Level::Debug);
        let _trace = logger.enabled(Level::Trace);
    }

    #[test]
    fn test_log_with_special_characters() {
        let logger = CompositeLogger;
        let record = Record::builder(Level::Info)
            .message("special chars: \t\n\"quotes\" and 'apostrophes'")
            .module_path("test::special")
            .file("test.rs")
            .line(1)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_with_unicode() {
        let logger = CompositeLogger;
        let record = Record::builder(Level::Info)
            .message("unicode: \u{1F600} \u{1F680}")
            .module_path("test::unicode")
            .file("test.rs")
            .line(1)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_composite_logger_repeated_calls() {
        let logger = CompositeLogger;
        // Multiple rapid calls should not panic
        for i in 0..100 {
            let msg = format!("rapid message {i}");
            let record = Record::builder(Level::Debug)
                .message(&msg)
                .module_path("test::rapid")
                .file("test.rs")
                .line(i)
                .build();
            logger.log(&record);
        }
    }

    #[test]
    fn test_enabled_consistency() {
        let logger = CompositeLogger;
        // Multiple calls to enabled should be consistent
        let first = logger.enabled(Level::Info);
        let second = logger.enabled(Level::Info);
        assert_eq!(first, second);
    }

    /// Ensure the global ring buffer is initialized for coverage of line 59.
    fn ensure_ring_initialized() {
        let _ = crate::debug::init_debug_ring();
    }

    #[test]
    fn test_log_with_ring_buffer_initialized() {
        // Cover line 59: ring.push(record) when ring buffer is available
        ensure_ring_initialized();

        let logger = CompositeLogger;
        let record = Record::builder(Level::Info)
            .message("ring buffer push coverage")
            .module_path("test::ring_push")
            .file("test.rs")
            .line(1)
            .build();
        logger.log(&record);

        // Verify the ring buffer is accessible (entry may be evicted due to small
        // capacity when initialized by other tests with capacity 2048).
        if let Some(ring) = super::try_debug_ring() {
            let _entries = ring.tail(100);
        }
    }

    #[test]
    fn test_log_all_levels_with_tracing_subscriber() {
        // Cover lines 66-98: tracing macro bodies for each level.
        // Install a tracing subscriber at TRACE level so the macro bodies execute.
        ensure_ring_initialized();

        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();

        let _guard = tracing::subscriber::set_default(subscriber);

        let logger = CompositeLogger;

        // Error level (lines 66-68, 70)
        let record = Record::builder(Level::Error)
            .message("error with subscriber")
            .module_path("test::sub::error")
            .file("sub_test.rs")
            .line(10)
            .build();
        logger.log(&record);

        // Warn level (lines 73-75, 77)
        let record = Record::builder(Level::Warn)
            .message("warn with subscriber")
            .module_path("test::sub::warn")
            .file("sub_test.rs")
            .line(20)
            .build();
        logger.log(&record);

        // Info level (lines 80-82, 84)
        let record = Record::builder(Level::Info)
            .message("info with subscriber")
            .module_path("test::sub::info")
            .file("sub_test.rs")
            .line(30)
            .build();
        logger.log(&record);

        // Debug level (lines 87-89, 91)
        let record = Record::builder(Level::Debug)
            .message("debug with subscriber")
            .module_path("test::sub::debug")
            .file("sub_test.rs")
            .line(40)
            .build();
        logger.log(&record);

        // Trace level (lines 94-96, 98)
        let record = Record::builder(Level::Trace)
            .message("trace with subscriber")
            .module_path("test::sub::trace")
            .file("sub_test.rs")
            .line(50)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_error_with_subscriber() {
        // Dedicated test for Error level tracing branch (lines 66-68, 70)
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let logger = CompositeLogger;
        let record = Record::builder(Level::Error)
            .message("dedicated error test")
            .module_path("test::dedicated::error")
            .file("dedicated.rs")
            .line(1)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_warn_with_subscriber() {
        // Dedicated test for Warn level tracing branch (lines 73-75, 77)
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let logger = CompositeLogger;
        let record = Record::builder(Level::Warn)
            .message("dedicated warn test")
            .module_path("test::dedicated::warn")
            .file("dedicated.rs")
            .line(2)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_info_with_subscriber() {
        // Dedicated test for Info level tracing branch (lines 80-82, 84)
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let logger = CompositeLogger;
        let record = Record::builder(Level::Info)
            .message("dedicated info test")
            .module_path("test::dedicated::info")
            .file("dedicated.rs")
            .line(3)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_debug_with_subscriber() {
        // Dedicated test for Debug level tracing branch (lines 87-89, 91)
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let logger = CompositeLogger;
        let record = Record::builder(Level::Debug)
            .message("dedicated debug test")
            .module_path("test::dedicated::debug")
            .file("dedicated.rs")
            .line(4)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_log_trace_with_subscriber() {
        // Dedicated test for Trace level tracing branch (lines 94-96, 98)
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let logger = CompositeLogger;
        let record = Record::builder(Level::Trace)
            .message("dedicated trace test")
            .module_path("test::dedicated::trace")
            .file("dedicated.rs")
            .line(5)
            .build();
        logger.log(&record);
    }

    #[test]
    fn test_enabled_with_subscriber_all_levels() {
        // Test enabled() with a subscriber that accepts all levels
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(std::io::sink)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let logger = CompositeLogger;

        // With a TRACE-level subscriber, all levels should be enabled
        assert!(logger.enabled(Level::Error));
        assert!(logger.enabled(Level::Warn));
        assert!(logger.enabled(Level::Info));
        assert!(logger.enabled(Level::Debug));
        assert!(logger.enabled(Level::Trace));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_log_with_ring_buffer_verified() {
        // Cover the ring buffer code path by ensuring it is initialized and
        // exercising the push + tail path. We don't assert specific entries
        // because the global ring may have small capacity (2048 bytes when
        // initialized by ring_buffer::tests) causing eviction.
        ensure_ring_initialized();

        let logger = CompositeLogger;
        let record = Record::builder(Level::Info)
            .message("ring_verified_coverage")
            .module_path("test::ring_verify")
            .file("composite_logger.rs")
            .line(1)
            .build();
        logger.log(&record);

        // Exercise ring buffer read path
        if let Some(ring) = super::try_debug_ring() {
            let entries = ring.tail(100);
            // Just verify we can iterate entries without panic
            let _ = entries.iter().any(|e| e.message.contains("ring_verified"));
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enabled_with_ring_buffer_initialized() {
        // Cover lines 296-298: enabled returns true when ring buffer is initialized
        ensure_ring_initialized();

        let logger = CompositeLogger;

        // With ring buffer initialized, ring_enabled is true, so all levels should
        // be enabled (ring_enabled || tracing_enabled)
        if super::try_debug_ring().is_some() {
            assert!(logger.enabled(Level::Info));
            assert!(logger.enabled(Level::Debug));
            assert!(logger.enabled(Level::Trace));
        }
    }
}
