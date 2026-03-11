#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Logging driver for reovim.
//!
//! This driver implements the kernel's `Logger` trait using the tracing ecosystem.
//! It bridges kernel `pr_*!` macros to tracing subscribers.
//!
//! # Architecture
//!
//! Following Linux kernel design (mechanism vs policy):
//! - **Kernel provides mechanism**: `Logger` trait, `Level`, `Record`, `pr_*!` macros
//! - **This driver provides policy**: Where logs go, formatting, filtering
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         User Code                                │
//! │   pr_info!("buffer {} opened", id);                              │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Kernel (printk/)                             │
//! │   Level │ Record │ Logger trait │ Global OnceLock                │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                 Driver (drivers/log/)                            │
//! │   TracingLogger │ LogConfig │ init_logging()                     │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     tracing ecosystem                            │
//! │   tracing │ tracing-subscriber │ tracing-appender                │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use reovim_driver_log::{TracingLogger, init_logging, LogConfig};
//! use reovim_kernel::api::v1::set_logger;
//!
//! // Step 1: Initialize tracing subscriber
//! let config = LogConfig::default();
//! init_logging(&config)?;
//!
//! // Step 2: Set TracingLogger as global kernel logger
//! static LOGGER: TracingLogger = TracingLogger;
//! set_logger(&LOGGER)?;
//!
//! // Step 3: Now pr_*! macros route to tracing
//! pr_info!("editor started");
//! pr_debug!("buffer count: {}", 5);
//! ```
//!
//! # Environment Variable
//!
//! Set `REOVIM_LOG` to control log level (overrides `LogConfig.level`):
//!
//! ```bash
//! REOVIM_LOG=debug reovim myfile.txt
//! REOVIM_LOG=trace reovim --log=- myfile.txt
//! REOVIM_LOG=reovim_kernel=trace,warn reovim myfile.txt  # Fine-grained control
//! ```
//!
//! # Output Formats
//!
//! - **Plain**: `2024-01-15T10:30:00Z INFO file.rs:42 message`
//! - **JSON**: `{"timestamp":"...","level":"INFO","target":"...","message":"..."}`
//! - **Pretty**: Colorized output for terminal (development)
//!
//! # File Rotation
//!
//! When using `LogOutput::File`, log files can be rotated:
//! - `RotationPolicy::Never` - Single file
//! - `RotationPolicy::Daily` - Rotate at midnight UTC
//! - `RotationPolicy::Hourly` - Rotate every hour

mod config;
mod logger;
mod subscriber;

// Configuration types
pub use config::{LogConfig, LogFormat, LogOutput, RotationPolicy};

// Logger implementation
pub use logger::{TracingLogger, from_tracing_level, to_tracing_level};

// Subscriber setup
pub use subscriber::{LogError, init_logging};

// Re-export kernel types for convenience
pub use reovim_kernel::api::v1::{Level, Logger, set_logger};
