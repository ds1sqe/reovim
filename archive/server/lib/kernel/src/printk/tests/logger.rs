use {
    super::super::*,
    std::{
        error::Error,
        sync::atomic::{AtomicUsize, Ordering},
    },
};

#[test]
fn test_nop_logger() {
    let logger = NopLogger;

    // All levels disabled
    assert!(!logger.enabled(Level::Error));
    assert!(!logger.enabled(Level::Warn));
    assert!(!logger.enabled(Level::Info));
    assert!(!logger.enabled(Level::Debug));
    assert!(!logger.enabled(Level::Trace));

    // log() and flush() are no-ops (should not panic)
    let record = Record::builder(Level::Info).message("test").build();
    logger.log(&record);
    logger.flush();
}

#[test]
fn test_set_logger_error_display() {
    let err = SetLoggerError;
    assert_eq!(format!("{err}"), "logger already set");
}

#[test]
fn test_logger_fallback() {
    // Note: We can't test set_logger() easily because it's global
    // and other tests might have already set it.
    // Instead, we just verify that logger() returns something.
    let l = logger();
    // Should not panic
    l.flush();
}

#[test]
fn test_custom_logger_trait() {
    // Test that we can implement the Logger trait

    struct CountingLogger {
        count: AtomicUsize,
    }

    impl Logger for CountingLogger {
        fn log(&self, _record: &Record) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }

        fn flush(&self) {}

        fn enabled(&self, level: Level) -> bool {
            level <= Level::Info
        }
    }

    let logger = CountingLogger {
        count: AtomicUsize::new(0),
    };

    // Test enabled()
    assert!(logger.enabled(Level::Error));
    assert!(logger.enabled(Level::Warn));
    assert!(logger.enabled(Level::Info));
    assert!(!logger.enabled(Level::Debug));
    assert!(!logger.enabled(Level::Trace));

    // Test flush()
    logger.flush();

    // Test log()
    let record = Record::builder(Level::Info).message("test").build();
    logger.log(&record);
    assert_eq!(logger.count.load(Ordering::Relaxed), 1);
}

#[test]
fn test_logger_send_sync() {
    // Verify Logger is Send + Sync (compile-time check)
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NopLogger>();
}

#[test]
fn test_logger_trait_object() {
    // Verify we can use Logger as a trait object
    let logger: &dyn Logger = &NopLogger;
    assert!(!logger.enabled(Level::Error));

    let record = Record::builder(Level::Error).message("test").build();
    logger.log(&record);
    logger.flush();
}

// ========== __log function ==========

#[test]
fn test_log_internal() {
    // __log should not panic even with default NopLogger
    __log(
        Level::Info,
        "test::module",
        "test_file.rs",
        42,
        format_args!("hello {}", "world"),
    );
}

// ========== flush function ==========

#[test]
fn test_flush() {
    // flush should not panic with NopLogger
    flush();
}

// ========== SetLoggerError as Error ==========

#[test]
fn test_set_logger_error_is_error() {
    let err = SetLoggerError;
    // Test std::error::Error impl
    let _: &dyn std::error::Error = &err;
    assert!(err.source().is_none());
}
