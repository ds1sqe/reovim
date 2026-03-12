//! Subscriber setup utilities.
//!
//! This module handles the tracing subscriber configuration based on `LogConfig`.
//! It provides `init_logging()` to set up the tracing subscriber before
//! setting the kernel logger.
//!
//! # Usage Order
//!
//! 1. Call `init_logging(&config)` to set up tracing subscriber
//! 2. Call `set_logger(&LOGGER)` to route kernel `pr_*!` macros to tracing
//!
//! ```rust,ignore
//! use reovim_driver_log::{init_logging, LogConfig, TracingLogger};
//! use reovim_kernel::api::v1::set_logger;
//!
//! // Step 1: Set up tracing subscriber
//! init_logging(&LogConfig::default())?;
//!
//! // Step 2: Set kernel logger
//! static LOGGER: TracingLogger = TracingLogger;
//! set_logger(&LOGGER)?;
//! ```

use std::{io, path::Path};

use {
    tracing_appender::rolling,
    tracing_subscriber::{EnvFilter, fmt},
};

use {
    crate::config::{LogConfig, LogFormat, LogOutput, RotationPolicy},
    reovim_kernel::api::v1::Level,
};

/// Error type for logging initialization.
#[derive(Debug)]
pub enum LogError {
    /// File path required for `File` output but not provided.
    MissingFilePath,

    /// Invalid filter string.
    InvalidFilter(String),

    /// Failed to set global subscriber.
    SetGlobalDefault(String),

    /// I/O error during setup.
    Io(io::Error),
}

impl std::fmt::Display for LogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFilePath => write!(f, "file_path required for File output"),
            Self::InvalidFilter(s) => write!(f, "invalid filter: {s}"),
            Self::SetGlobalDefault(s) => write!(f, "failed to set global subscriber: {s}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for LogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for LogError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Initialize logging with the given configuration.
///
/// This sets up the tracing subscriber. Call this **before** `set_logger()`.
///
/// # Environment Variable
///
/// The `REOVIM_LOG` environment variable takes precedence over `config.level`.
/// Examples:
/// - `REOVIM_LOG=debug` - enable debug and above
/// - `REOVIM_LOG=warn` - enable warn and error only
/// - `REOVIM_LOG=reovim_kernel=trace` - trace for kernel crate only
///
/// # Errors
///
/// Returns error if:
/// - `LogOutput::File` is used but `file_path` is `None`
/// - Invalid filter string
/// - Subscriber setup fails
///
/// # Example
///
/// ```
/// use reovim_driver_log::{init_logging, LogConfig, LogFormat};
///
/// let config = LogConfig {
///     format: LogFormat::Pretty,
///     ..Default::default()
/// };
///
/// // In a real application (can only be called once):
/// // init_logging(&config).expect("failed to initialize logging");
/// ```
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn init_logging(config: &LogConfig) -> Result<(), LogError> {
    let filter = build_filter(config)?;

    match (&config.output, &config.format) {
        (LogOutput::Stderr, LogFormat::Plain) => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_writer(io::stderr)
                .with_file(true)
                .with_line_number(true)
                .with_target(true)
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LogError::SetGlobalDefault(e.to_string()))?;
        }
        (LogOutput::Stderr, LogFormat::Json) => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_writer(io::stderr)
                .json()
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LogError::SetGlobalDefault(e.to_string()))?;
        }
        (LogOutput::Stderr, LogFormat::Pretty) => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_writer(io::stderr)
                .pretty()
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LogError::SetGlobalDefault(e.to_string()))?;
        }
        (LogOutput::Stdout, format) => {
            init_stdout(filter, format)?;
        }
        (LogOutput::File, format) => {
            let path = config.file_path.as_ref().ok_or(LogError::MissingFilePath)?;
            init_file(filter, path, &config.rotation, format)?;
        }
    }

    Ok(())
}

/// Build the `EnvFilter` from config, respecting `REOVIM_LOG` env var.
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_filter(config: &LogConfig) -> Result<EnvFilter, LogError> {
    // REOVIM_LOG env var takes precedence
    if let Ok(filter) = EnvFilter::try_from_env("REOVIM_LOG") {
        return Ok(filter);
    }

    // Fall back to config level
    let level_str = level_to_filter_str(config.level);

    EnvFilter::try_new(level_str).map_err(|e| LogError::InvalidFilter(e.to_string()))
}

/// Convert kernel `Level` to filter string.
const fn level_to_filter_str(level: Level) -> &'static str {
    match level {
        Level::Error => "error",
        Level::Warn => "warn",
        Level::Info => "info",
        Level::Debug => "debug",
        Level::Trace => "trace",
    }
}

/// Initialize stdout subscriber.
#[cfg_attr(coverage_nightly, coverage(off))]
fn init_stdout(filter: EnvFilter, format: &LogFormat) -> Result<(), LogError> {
    match format {
        LogFormat::Plain => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_writer(io::stdout)
                .with_file(true)
                .with_line_number(true)
                .with_target(true)
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LogError::SetGlobalDefault(e.to_string()))
        }
        LogFormat::Json => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_writer(io::stdout)
                .json()
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LogError::SetGlobalDefault(e.to_string()))
        }
        LogFormat::Pretty => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_writer(io::stdout)
                .pretty()
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LogError::SetGlobalDefault(e.to_string()))
        }
    }
}

/// Initialize file subscriber with rotation.
#[cfg_attr(coverage_nightly, coverage(off))]
fn init_file(
    filter: EnvFilter,
    path: &Path,
    rotation: &RotationPolicy,
    format: &LogFormat,
) -> Result<(), LogError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let prefix = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("reovim");

    let appender = match rotation {
        RotationPolicy::Never => rolling::never(dir, prefix),
        RotationPolicy::Daily => rolling::daily(dir, prefix),
        RotationPolicy::Hourly => rolling::hourly(dir, prefix),
    };

    // Non-blocking writer to avoid blocking the main thread on I/O.
    // The guard is leaked to keep the writer alive for the program lifetime.
    // This is intentional - logging should work until program exit.
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);
    std::mem::forget(guard);

    match format {
        LogFormat::Json => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_writer(non_blocking)
                .json()
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LogError::SetGlobalDefault(e.to_string()))
        }
        LogFormat::Plain | LogFormat::Pretty => {
            // Pretty doesn't make sense for files (no colors), use plain
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_writer(non_blocking)
                .with_file(true)
                .with_line_number(true)
                .with_target(true)
                .with_ansi(false) // No ANSI colors in file output
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LogError::SetGlobalDefault(e.to_string()))
        }
    }
}

#[cfg(test)]
#[path = "subscriber_tests.rs"]
mod tests;
