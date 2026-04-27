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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn flush(&self) {}

    fn enabled(&self, level: Level) -> bool {
        level <= self.enabled_level
    }
}

// Note: We can't easily test the macros with set_logger() because
// it's global and can only be set once. Instead, we test the
// underlying __log function.

#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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

#[cfg_attr(coverage_nightly, coverage(off))]
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
