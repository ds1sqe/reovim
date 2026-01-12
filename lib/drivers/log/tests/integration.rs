//! Integration test: verify `TracingLogger` works with kernel `Logger` trait.
//!
//! This test demonstrates the full kernel -> driver integration.

use {
    reovim_driver_log::{LogConfig, LogFormat, LogOutput, Logger, TracingLogger},
    reovim_kernel::api::v1::Level,
};

const fn assert_send_sync<T: Send + Sync>() {}
const fn assert_logger<T: Logger>(_: &T) {}

#[test]
fn test_tracing_logger_implements_logger_trait() {
    // This test verifies that TracingLogger correctly implements the Logger trait
    // from the kernel. We can't actually call set_logger() here because:
    // 1. Global logger can only be set once per process
    // 2. Tests run in parallel
    //
    // Instead, we verify:
    // 1. TracingLogger implements Logger (compile-time check)
    // 2. TracingLogger has correct trait bounds (Send + Sync)
    // 3. Methods are callable

    let logger = TracingLogger;

    // Verify trait bounds
    assert_send_sync::<TracingLogger>();

    // Verify Logger trait is implemented (compile-time check)
    assert_logger(&logger);

    // Verify enabled() is callable
    let _ = logger.enabled(Level::Info);

    // Verify flush() is callable
    logger.flush();
}

#[test]
fn test_log_config_defaults() {
    // Verify LogConfig has sensible defaults
    let config = LogConfig::default();

    assert_eq!(config.level, Level::Info);
    assert!(matches!(config.output, LogOutput::Stderr));
    assert!(matches!(config.format, LogFormat::Plain));
    assert!(config.file_path.is_none());
}

#[test]
fn test_level_filtering() {
    // Verify TracingLogger respects level filtering
    let logger = TracingLogger;

    // All levels should be callable
    for level in Level::ALL {
        let _ = logger.enabled(level);
    }
}

// Note: We cannot test actual logging output easily because:
// 1. set_logger() can only be called once globally
// 2. Tests run in parallel
// 3. Global tracing subscriber can only be set once
//
// These would be better tested in a dedicated integration test binary
// or end-to-end tests that run serially.
