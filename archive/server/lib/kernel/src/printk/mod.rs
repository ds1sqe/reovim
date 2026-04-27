//! Kernel logging subsystem.
//!
//! Linux equivalent: `kernel/printk/`
//!
//! This module provides the kernel's logging mechanism following Linux's printk design.
//! The kernel defines the logging interface (`Logger` trait), while drivers implement
//! the actual output policy (where logs go, formatting, buffering, etc.).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         User Code                                │
//! │                                                                  │
//! │   pr_err!("failed: {}", err);                                    │
//! │   pr_info!("buffer {} opened", id);                              │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Kernel (printk/)                             │
//! │                                                                  │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
//! │  │    Level     │    │    Record    │    │   Logger     │       │
//! │  │  (severity)  │    │  (message +  │    │  (trait)     │       │
//! │  │              │    │   metadata)  │    │              │       │
//! │  └──────────────┘    └──────────────┘    └──────────────┘       │
//! │                                                 │                │
//! │                              ┌──────────────────┘                │
//! │                              │                                   │
//! │                   ┌──────────┴──────────┐                        │
//! │                   │  Global Logger      │                        │
//! │                   │  (OnceLock)         │                        │
//! │                   └──────────┬──────────┘                        │
//! │                              │                                   │
//! └──────────────────────────────┼───────────────────────────────────┘
//!                                │
//!              ┌─────────────────┴─────────────────┐
//!              │                                   │
//!              ▼                                   ▼
//! ┌────────────────────────┐         ┌────────────────────────┐
//! │   NopLogger (default)  │         │   Driver Logger        │
//! │   - No output          │         │   (e.g., TracingLogger)│
//! │   - Zero overhead      │         │   - Actual output      │
//! └────────────────────────┘         └────────────────────────┘
//! ```
//!
//! # Design Philosophy
//!
//! Following Linux kernel "mechanism, not policy":
//!
//! - **Kernel provides mechanism**: `Logger` trait, `Level` enum, `Record` struct
//! - **Drivers provide policy**: Where logs go, formatting, filtering, buffering
//! - **Zero external dependencies**: Pure Rust, std only
//! - **Performance first**: Level check before formatting to avoid allocations
//!
//! # Usage
//!
//! ## Basic Logging
//!
//! ```
//! use reovim_kernel::{pr_err, pr_warn, pr_info, pr_debug, pr_trace};
//!
//! // Log at different levels
//! pr_err!("critical error: buffer not found");
//! pr_warn!("deprecated API used");
//! pr_info!("editor started with {} buffers", 3);
//! pr_debug!("cursor at position {:?}", (10, 5));
//! pr_trace!("entering function");
//! ```
//!
//! ## Setting a Logger (Driver Layer)
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! struct StderrLogger;
//!
//! impl Logger for StderrLogger {
//!     fn log(&self, record: &Record) {
//!         eprintln!("[{}] {}:{} - {}",
//!             record.level(),
//!             record.file(),
//!             record.line(),
//!             record.message());
//!     }
//!
//!     fn flush(&self) {}
//!
//!     fn enabled(&self, level: Level) -> bool {
//!         level <= Level::Info  // Log Error, Warn, Info
//!     }
//! }
//!
//! // Set once at startup (typically in server/lib/server/)
//! // static LOGGER: StderrLogger = StderrLogger;
//! // set_logger(&LOGGER).expect("logger already set");
//! ```
//!
//! # Performance
//!
//! The logging macros are designed for minimal overhead when logging is disabled:
//!
//! ```text
//! pr_info!("message: {}", expensive_computation());
//!          │
//!          ▼
//! if logger().enabled(Level::Info) {  ◄── Fast check (no allocation)
//!     // Only format if enabled
//!     __log(..., format_args!(...));   ◄── Allocation happens here
//! }
//! ```
//!
//! When the level is disabled, `enabled()` returns `false` and the format
//! string is never evaluated, avoiding the cost of string formatting.
//!
//! # Thread Safety
//!
//! All logging operations are thread-safe:
//!
//! - `set_logger()` uses `OnceLock` for safe one-time initialization
//! - `Logger` implementations must be `Send + Sync`
//! - Multiple threads can log concurrently without blocking

mod level;
mod logger;
mod macros;
mod record;

// Re-export level types
pub use level::{Level, ParseLevelError};

// Re-export record types
pub use record::{Record, RecordBuilder};

// Re-export logger types and functions
pub use logger::{__log, Logger, NopLogger, SetLoggerError, flush, logger, set_logger};

#[cfg(test)]
mod tests;
