use super::*;

#[test]
fn test_level_mapping_to_tracing() {
    assert_eq!(to_tracing_level(Level::Error), tracing::Level::ERROR);
    assert_eq!(to_tracing_level(Level::Warn), tracing::Level::WARN);
    assert_eq!(to_tracing_level(Level::Info), tracing::Level::INFO);
    assert_eq!(to_tracing_level(Level::Debug), tracing::Level::DEBUG);
    assert_eq!(to_tracing_level(Level::Trace), tracing::Level::TRACE);
}

#[test]
fn test_level_mapping_from_tracing() {
    assert_eq!(from_tracing_level(tracing::Level::ERROR), Level::Error);
    assert_eq!(from_tracing_level(tracing::Level::WARN), Level::Warn);
    assert_eq!(from_tracing_level(tracing::Level::INFO), Level::Info);
    assert_eq!(from_tracing_level(tracing::Level::DEBUG), Level::Debug);
    assert_eq!(from_tracing_level(tracing::Level::TRACE), Level::Trace);
}

#[test]
fn test_level_roundtrip() {
    for level in Level::ALL {
        let tracing_level = to_tracing_level(level);
        let back = from_tracing_level(tracing_level);
        assert_eq!(level, back);
    }
}

#[test]
fn test_tracing_logger_is_zst() {
    assert_eq!(std::mem::size_of::<TracingLogger>(), 0);
}

#[test]
fn test_tracing_logger_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TracingLogger>();
}

#[test]
fn test_tracing_logger_default() {
    let logger = TracingLogger;
    // Should compile and not panic
    logger.flush();
}

#[test]
fn test_tracing_logger_copy() {
    let logger = TracingLogger;
    let copied = logger;
    // ZST is Copy, so both are valid
    copied.flush();
}

#[test]
fn test_tracing_logger_debug() {
    let logger = TracingLogger;
    let debug_str = format!("{logger:?}");
    assert_eq!(debug_str, "TracingLogger");
}

#[test]
fn test_tracing_logger_clone() {
    let logger = TracingLogger;
    let cloned = logger;
    let debug_str = format!("{cloned:?}");
    assert_eq!(debug_str, "TracingLogger");
}

#[test]
fn test_tracing_logger_log_all_levels() {
    // Test that logging at all levels does not panic.
    // Without a subscriber, events are simply dropped.
    let logger = TracingLogger;
    for level in Level::ALL {
        let record = Record::builder(level)
            .message("test message")
            .module_path("test::module")
            .file("test.rs")
            .line(1)
            .build();
        logger.log(&record);
    }
}

#[test]
fn test_tracing_logger_log_empty_message() {
    let logger = TracingLogger;
    let record = Record::builder(Level::Info)
        .message("")
        .module_path("")
        .file("")
        .line(0)
        .build();
    // Should not panic with empty fields
    logger.log(&record);
}

#[test]
fn test_tracing_logger_enabled_no_panic() {
    // Without a subscriber, enabled() should not panic
    let logger = TracingLogger;
    for level in Level::ALL {
        // Just verify it returns a boolean without panicking
        let _ = logger.enabled(level);
    }
}

#[test]
fn test_tracing_logger_flush_no_op() {
    let logger = TracingLogger;
    // flush() is a no-op, should never panic
    logger.flush();
    logger.flush();
}

#[test]
fn test_to_tracing_level_and_back() {
    // Verify that every kernel level converts to a distinct tracing level
    let levels = Level::ALL;
    let tracing_levels: Vec<_> = levels.iter().map(|l| to_tracing_level(*l)).collect();

    // All tracing levels should be distinct
    for (i, a) in tracing_levels.iter().enumerate() {
        for (j, b) in tracing_levels.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "Levels at indices {i} and {j} should differ");
            }
        }
    }
}

#[test]
fn test_tracing_logger_implements_logger_trait() {
    fn assert_logger<T: Logger>() {}
    assert_logger::<TracingLogger>();
}

#[test]
fn test_tracing_logger_log_with_metadata() {
    let logger = TracingLogger;
    let record = Record::builder(Level::Warn)
        .message("warning message with detail")
        .module_path("reovim_kernel::mm::buffer")
        .file("buffer.rs")
        .line(999)
        .build();
    // Should not panic, just emits to whatever subscriber is set
    logger.log(&record);
}

#[test]
fn test_tracing_logger_log_all_levels_with_subscriber() {
    use tracing_subscriber::fmt;

    // Create a subscriber that captures at TRACE level
    let subscriber = fmt::Subscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(std::io::sink)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let logger = TracingLogger;
        for level in Level::ALL {
            let record = Record::builder(level)
                .message("test message for coverage")
                .module_path("test::coverage")
                .file("test.rs")
                .line(42)
                .build();
            logger.log(&record);
        }
    });
}

#[test]
fn test_tracing_logger_enabled_with_subscriber() {
    use tracing_subscriber::fmt;

    // Create a subscriber that enables all levels
    let subscriber = fmt::Subscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(std::io::sink)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let logger = TracingLogger;
        // With an active subscriber at TRACE level, all levels should be enabled
        for level in Level::ALL {
            assert!(
                logger.enabled(level),
                "Level {level:?} should be enabled with TRACE subscriber"
            );
        }
    });
}

#[test]
fn test_tracing_logger_enabled_with_error_only_subscriber() {
    use tracing_subscriber::fmt;

    // Create a subscriber that only enables ERROR level
    let subscriber = fmt::Subscriber::builder()
        .with_max_level(tracing::Level::ERROR)
        .with_writer(std::io::sink)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let logger = TracingLogger;
        assert!(logger.enabled(Level::Error));
        // Lower levels should NOT be enabled
        assert!(!logger.enabled(Level::Warn));
        assert!(!logger.enabled(Level::Info));
        assert!(!logger.enabled(Level::Debug));
        assert!(!logger.enabled(Level::Trace));
    });
}
