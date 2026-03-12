//! `TracingLogger` - bridges kernel `Logger` trait to tracing.
//!
//! This is the core of the driver: it receives `Record` from kernel
//! and emits tracing events to whatever subscriber is configured.
//!
//! # Architecture
//!
//! ```text
//! Kernel                           Driver
//! ┌─────────────┐                  ┌─────────────────┐
//! │ pr_info!()  │ ──Record──────▶ │ TracingLogger   │
//! │             │                  │   .log()        │
//! └─────────────┘                  └────────┬────────┘
//!                                           │
//!                                           ▼
//!                                  ┌─────────────────┐
//!                                  │ tracing::info!  │
//!                                  │ (subscriber)    │
//!                                  └─────────────────┘
//! ```

use reovim_kernel::api::v1::{Level, Logger, Record};

/// Logger implementation that forwards to tracing.
///
/// This is a zero-sized type (ZST) - all state lives in the global
/// tracing subscriber. This allows `TracingLogger` to be a static.
///
/// # Example
///
/// ```
/// use reovim_driver_log::TracingLogger;
/// use reovim_kernel::api::v1::set_logger;
///
/// static LOGGER: TracingLogger = TracingLogger;
///
/// // In main(), after init_logging():
/// // set_logger(&LOGGER).expect("logger already set");
/// ```
///
/// # Thread Safety
///
/// `TracingLogger` is `Send + Sync` as required by the `Logger` trait.
/// Since it's a ZST with no state, it's inherently thread-safe.
#[derive(Debug, Clone, Copy, Default)]
pub struct TracingLogger;

#[cfg_attr(coverage_nightly, coverage(off))]
impl Logger for TracingLogger {
    fn log(&self, record: &Record) {
        // Emit tracing event with all metadata from kernel Record.
        // Note: `target:` in tracing macros must be a compile-time constant,
        // so we include module_path as a regular field instead.
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
        // For file output, tracing-appender flushes on drop.
        // This is a no-op for TracingLogger.
    }

    fn enabled(&self, level: Level) -> bool {
        // This is called BEFORE message formatting - must be fast.
        // We check if the tracing subscriber would accept this level.
        match level {
            Level::Error => tracing::enabled!(tracing::Level::ERROR),
            Level::Warn => tracing::enabled!(tracing::Level::WARN),
            Level::Info => tracing::enabled!(tracing::Level::INFO),
            Level::Debug => tracing::enabled!(tracing::Level::DEBUG),
            Level::Trace => tracing::enabled!(tracing::Level::TRACE),
        }
    }
}

/// Convert kernel `Level` to tracing `Level`.
///
/// The mapping is 1:1 - kernel levels were designed to match tracing levels.
///
/// | Kernel | tracing |
/// |--------|---------|
/// | Error  | ERROR   |
/// | Warn   | WARN    |
/// | Info   | INFO    |
/// | Debug  | DEBUG   |
/// | Trace  | TRACE   |
#[must_use]
pub const fn to_tracing_level(level: Level) -> tracing::Level {
    match level {
        Level::Error => tracing::Level::ERROR,
        Level::Warn => tracing::Level::WARN,
        Level::Info => tracing::Level::INFO,
        Level::Debug => tracing::Level::DEBUG,
        Level::Trace => tracing::Level::TRACE,
    }
}

/// Convert tracing `Level` to kernel `Level`.
///
/// Inverse of [`to_tracing_level`].
#[must_use]
pub const fn from_tracing_level(level: tracing::Level) -> Level {
    match level {
        tracing::Level::ERROR => Level::Error,
        tracing::Level::WARN => Level::Warn,
        tracing::Level::INFO => Level::Info,
        tracing::Level::DEBUG => Level::Debug,
        tracing::Level::TRACE => Level::Trace,
    }
}

#[cfg(test)]
#[path = "logger_tests.rs"]
mod tests;
