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
#[path = "composite_logger_tests.rs"]
mod tests;
