//! Logger trait and global logger storage.
//!
//! Linux equivalent: `struct console` and `register_console()` in `kernel/printk/`
//!
//! This module defines the `Logger` trait that drivers implement to provide
//! actual log output. The kernel only defines the interface (mechanism),
//! while drivers implement the policy (where and how to log).

use std::{error::Error, fmt, sync::OnceLock};

use super::{level::Level, record::Record};

/// Logger trait - the kernel mechanism for logging.
///
/// Drivers implement this trait to provide actual log output. The kernel
/// only defines the interface, not the policy (where logs go, formatting, etc.).
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` as the logger is accessed from
/// multiple threads concurrently. Implementations should be lock-free
/// or use minimal locking to avoid blocking the caller.
///
/// # Example
///
/// ```
/// use reovim_kernel::printk::{Logger, Level, Record};
///
/// struct StderrLogger;
///
/// impl Logger for StderrLogger {
///     fn log(&self, record: &Record) {
///         eprintln!("[{}] {}: {}",
///             record.level(),
///             record.file(),
///             record.message());
///     }
///
///     fn flush(&self) {
///         // stderr is unbuffered by default
///     }
///
///     fn enabled(&self, level: Level) -> bool {
///         level <= Level::Debug  // Log everything except Trace
///     }
/// }
/// ```
pub trait Logger: Send + Sync {
    /// Log a record.
    ///
    /// This method should be fast and non-blocking. If the logger
    /// buffers output, it should do so efficiently.
    fn log(&self, record: &Record);

    /// Flush any buffered output.
    ///
    /// Called to ensure all pending logs are written. May be called
    /// before program exit or when immediate output is needed.
    fn flush(&self);

    /// Check if a level is enabled for logging.
    ///
    /// This method is called before formatting the log message,
    /// allowing early exit if the level is not enabled. This avoids
    /// the cost of string formatting when logging is disabled.
    fn enabled(&self, level: Level) -> bool;
}

/// No-op logger used when no logger is set.
///
/// All operations are no-ops. `enabled()` returns `false` for all levels,
/// causing the logging macros to skip message formatting entirely.
///
/// This is the default logger if `set_logger()` is never called.
#[derive(Debug, Clone, Copy, Default)]
pub struct NopLogger;

impl Logger for NopLogger {
    #[inline]
    fn log(&self, _record: &Record) {
        // No-op
    }

    #[inline]
    fn flush(&self) {
        // No-op
    }

    #[inline]
    fn enabled(&self, _level: Level) -> bool {
        false
    }
}

/// Error returned when attempting to set the logger more than once.
///
/// The global logger can only be set once. This error is returned
/// if `set_logger()` is called after a logger has already been set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetLoggerError;

impl fmt::Display for SetLoggerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "logger already set")
    }
}

impl Error for SetLoggerError {}

// =============================================================================
// Global Logger State
// =============================================================================

/// Global logger storage.
///
/// Uses `OnceLock` for thread-safe one-time initialization.
/// This is the only global state in the printk module.
static LOGGER: OnceLock<&'static dyn Logger> = OnceLock::new();

/// Static no-op logger instance used as default.
static NOP_LOGGER: NopLogger = NopLogger;

/// Sets the global logger.
///
/// This function can only be called once. Subsequent calls will
/// return `Err(SetLoggerError)`.
///
/// # Errors
///
/// Returns `Err(SetLoggerError)` if a logger has already been set.
/// The global logger can only be set once.
///
/// # Thread Safety
///
/// This function is thread-safe. If multiple threads call `set_logger()`
/// concurrently, only one will succeed (and return `Ok`), while the
/// others will return `Err(SetLoggerError)`.
///
/// # Example
///
/// ```
/// use reovim_kernel::printk::{Logger, Level, Record, set_logger};
///
/// struct MyLogger;
///
/// impl Logger for MyLogger {
///     fn log(&self, record: &Record) {
///         eprintln!("{}", record.message());
///     }
///     fn flush(&self) {}
///     fn enabled(&self, _level: Level) -> bool { true }
/// }
///
/// static MY_LOGGER: MyLogger = MyLogger;
///
/// // First call succeeds
/// // Note: This would succeed, but we can't actually run it in doctests
/// // because other tests might have already set the logger.
/// // assert!(set_logger(&MY_LOGGER).is_ok());
/// ```
pub fn set_logger(logger: &'static dyn Logger) -> Result<(), SetLoggerError> {
    LOGGER.set(logger).map_err(|_| SetLoggerError)
}

/// Returns the global logger.
///
/// If no logger has been set via `set_logger()`, returns the no-op logger
/// which silently discards all log messages.
///
/// # Example
///
/// ```
/// use reovim_kernel::printk::{logger, Level};
///
/// // Before set_logger() is called, returns NopLogger
/// assert!(!logger().enabled(Level::Error));
/// ```
#[must_use]
pub fn logger() -> &'static dyn Logger {
    LOGGER.get().copied().unwrap_or(&NOP_LOGGER)
}

/// Internal helper for logging macros.
///
/// This function is called by the `pr_*` macros after the level check passes.
/// It formats the message and passes it to the logger.
///
/// # Note
///
/// This function is marked `#[doc(hidden)]` because it's an implementation
/// detail of the logging macros. Users should use the macros instead.
#[doc(hidden)]
pub fn __log(
    level: Level,
    module_path: &'static str,
    file: &'static str,
    line: u32,
    args: fmt::Arguments,
) {
    // Format the message (this is the allocation we want to avoid
    // when logging is disabled - hence the level check in macros)
    let message = args.to_string();

    let record = Record::builder(level)
        .message(&message)
        .module_path(module_path)
        .file(file)
        .line(line)
        .build();

    logger().log(&record);
}

/// Flushes the global logger.
///
/// Ensures all buffered log messages are written out. Useful before
/// program exit or when immediate output is required.
pub fn flush() {
    logger().flush();
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::sync::atomic::{AtomicUsize, Ordering},
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
}
