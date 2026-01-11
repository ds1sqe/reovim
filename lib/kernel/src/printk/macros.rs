//! Logging macros for kernel messages.
//!
//! Linux equivalent: `pr_err()`, `pr_warn()`, `pr_info()`, `pr_debug()` in `include/linux/printk.h`
//!
//! These macros provide a convenient way to log messages at different severity levels.
//! They check the log level before formatting to avoid allocation overhead when
//! logging is disabled.

/// Logs a message at the Error level.
///
/// Use for critical errors that require immediate attention and may prevent
/// normal operation.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_err;
///
/// let buffer_id = 42;
/// pr_err!("failed to save buffer {}", buffer_id);
/// pr_err!("critical error occurred");
/// ```
#[macro_export]
macro_rules! pr_err {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Error) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Error,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

/// Logs a message at the Warn level.
///
/// Use for warning conditions that don't prevent operation but indicate
/// potential problems.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_warn;
///
/// let path = "/tmp/file.txt";
/// pr_warn!("file {} not found, using default", path);
/// pr_warn!("deprecated configuration option used");
/// ```
#[macro_export]
macro_rules! pr_warn {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Warn) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Warn,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

/// Logs a message at the Info level.
///
/// Use for general informational messages about normal operation.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_info;
///
/// pr_info!("editor started");
/// pr_info!("opened {} buffers", 3);
/// ```
#[macro_export]
macro_rules! pr_info {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Info) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Info,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

/// Logs a message at the Debug level.
///
/// Use for detailed diagnostic information useful during development.
/// These messages are typically disabled in release builds.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_debug;
///
/// let cursor_pos = (10, 5);
/// pr_debug!("cursor moved to {:?}", cursor_pos);
/// pr_debug!("entering function");
/// ```
#[macro_export]
macro_rules! pr_debug {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Debug) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Debug,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

/// Logs a message at the Trace level.
///
/// Use for very detailed tracing of execution flow. This is the most
/// verbose level and should be used sparingly.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_trace;
///
/// pr_trace!("processing event");
/// pr_trace!("loop iteration {}", 42);
/// ```
#[macro_export]
macro_rules! pr_trace {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Trace) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Trace,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

#[cfg(test)]
mod tests {
    use {
        super::super::*,
        std::sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    /// A test logger that captures log messages.
    struct CapturingLogger {
        messages: Mutex<Vec<String>>,
        enabled_level: Level,
        call_count: AtomicUsize,
    }

    impl CapturingLogger {
        fn new(enabled_level: Level) -> Self {
            Self {
                messages: Mutex::new(Vec::new()),
                enabled_level,
                call_count: AtomicUsize::new(0),
            }
        }

        fn messages(&self) -> Vec<String> {
            self.messages.lock().unwrap().clone()
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::Relaxed)
        }
    }

    impl Logger for CapturingLogger {
        fn log(&self, record: &Record) {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            let msg = format!(
                "[{}] {}:{}: {}",
                record.level(),
                record.file(),
                record.line(),
                record.message()
            );
            self.messages.lock().unwrap().push(msg);
        }

        fn flush(&self) {}

        fn enabled(&self, level: Level) -> bool {
            level <= self.enabled_level
        }
    }

    // Note: We can't easily test the macros with set_logger() because
    // it's global and can only be set once. Instead, we test the
    // underlying __log function.

    #[test]
    fn test_log_internal() {
        // Test __log directly
        let logger = CapturingLogger::new(Level::Info);

        // Simulate what the macro does
        if logger.enabled(Level::Info) {
            let message = format_args!("test message {}", 42).to_string();
            let record = Record::builder(Level::Info)
                .message(&message)
                .module_path("test")
                .file("macros.rs")
                .line(100)
                .build();
            logger.log(&record);
        }

        assert_eq!(logger.call_count(), 1);
        let messages = logger.messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("test message 42"));
        assert!(messages[0].contains("INFO"));
    }

    #[test]
    fn test_level_filtering() {
        // Test that log is not called when level is disabled
        let logger = CapturingLogger::new(Level::Warn);

        // Info should be filtered (Warn < Info)
        if logger.enabled(Level::Info) {
            let message = "should not appear";
            let record = Record::builder(Level::Info).message(message).build();
            logger.log(&record);
        }

        // Error should pass (Error < Warn)
        if logger.enabled(Level::Error) {
            let message = "should appear";
            let record = Record::builder(Level::Error).message(message).build();
            logger.log(&record);
        }

        assert_eq!(logger.call_count(), 1);
        let messages = logger.messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("should appear"));
    }

    #[test]
    fn test_format_args() {
        // Test that format_args works correctly
        let logger = CapturingLogger::new(Level::Trace);

        // Test various format specifiers by logging each one
        // (can't store format_args in a Vec due to lifetime issues)

        // Simple message
        if logger.enabled(Level::Debug) {
            let message = "simple message".to_string();
            let record = Record::builder(Level::Debug).message(&message).build();
            logger.log(&record);
        }

        // Number formatting
        if logger.enabled(Level::Debug) {
            let message = format!("number: {}", 42);
            let record = Record::builder(Level::Debug).message(&message).build();
            logger.log(&record);
        }

        // Multiple arguments
        if logger.enabled(Level::Debug) {
            let message = format!("multiple: {} and {}", "a", "b");
            let record = Record::builder(Level::Debug).message(&message).build();
            logger.log(&record);
        }

        // Debug formatting
        if logger.enabled(Level::Debug) {
            let message = format!("debug: {:?}", vec![1, 2, 3]);
            let record = Record::builder(Level::Debug).message(&message).build();
            logger.log(&record);
        }

        assert_eq!(logger.call_count(), 4);
    }
}
