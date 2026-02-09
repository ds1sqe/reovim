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
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn test_log_error_display() {
        assert_eq!(LogError::MissingFilePath.to_string(), "file_path required for File output");

        assert_eq!(LogError::InvalidFilter("bad".into()).to_string(), "invalid filter: bad");

        assert_eq!(
            LogError::SetGlobalDefault("already set".into()).to_string(),
            "failed to set global subscriber: already set"
        );
    }

    #[test]
    fn test_level_to_filter_str() {
        assert_eq!(level_to_filter_str(Level::Error), "error");
        assert_eq!(level_to_filter_str(Level::Warn), "warn");
        assert_eq!(level_to_filter_str(Level::Info), "info");
        assert_eq!(level_to_filter_str(Level::Debug), "debug");
        assert_eq!(level_to_filter_str(Level::Trace), "trace");
    }

    #[test]
    fn test_log_error_debug() {
        let err = LogError::MissingFilePath;
        let debug_str = format!("{err:?}");
        assert!(debug_str.contains("MissingFilePath"));
    }

    #[test]
    fn test_log_error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let log_err = LogError::from(io_err);
        assert!(matches!(log_err, LogError::Io(_)));
    }

    #[test]
    fn test_log_error_source() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let log_err = LogError::Io(io_err);
        assert!(log_err.source().is_some());

        let missing_err = LogError::MissingFilePath;
        assert!(missing_err.source().is_none());
    }

    #[test]
    fn test_log_error_io_display() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let log_err = LogError::Io(io_err);
        let display = log_err.to_string();
        assert!(display.contains("I/O error"));
        assert!(display.contains("access denied"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_log_error_source_all_variants() {
        // MissingFilePath has no source
        let err = LogError::MissingFilePath;
        assert!(err.source().is_none());

        // InvalidFilter has no source
        let err = LogError::InvalidFilter("bad filter".into());
        assert!(err.source().is_none());

        // SetGlobalDefault has no source
        let err = LogError::SetGlobalDefault("already set".into());
        assert!(err.source().is_none());

        // Io wraps an io::Error which IS the source
        let io_err = io::Error::new(io::ErrorKind::NotFound, "gone");
        let err = LogError::Io(io_err);
        assert!(err.source().is_some());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_log_error_from_io_preserves_error_kind() {
        let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken");
        let log_err = LogError::from(io_err);
        match log_err {
            LogError::Io(ref e) => assert_eq!(e.kind(), io::ErrorKind::BrokenPipe),
            _ => panic!("Expected LogError::Io"),
        }
    }

    #[test]
    fn test_log_error_debug_all_variants() {
        let variants: Vec<LogError> = vec![
            LogError::MissingFilePath,
            LogError::InvalidFilter("test".into()),
            LogError::SetGlobalDefault("test".into()),
            LogError::Io(io::Error::other("test")),
        ];
        for variant in &variants {
            let debug_str = format!("{variant:?}");
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_log_error_display_missing_file_path() {
        let err = LogError::MissingFilePath;
        assert_eq!(err.to_string(), "file_path required for File output");
    }

    #[test]
    fn test_log_error_display_invalid_filter() {
        let err = LogError::InvalidFilter("not a valid directive".into());
        assert_eq!(err.to_string(), "invalid filter: not a valid directive");
    }

    #[test]
    fn test_log_error_display_set_global_default() {
        let err = LogError::SetGlobalDefault("a]ready set".into());
        assert_eq!(err.to_string(), "failed to set global subscriber: a]ready set");
    }

    #[test]
    fn test_level_to_filter_str_exhaustive() {
        // Verify every level maps to a distinct filter string
        let levels = Level::ALL;
        let strs: Vec<&str> = levels.iter().map(|l| level_to_filter_str(*l)).collect();

        // All should be non-empty
        for s in &strs {
            assert!(!s.is_empty());
        }

        // All should be distinct
        for (i, a) in strs.iter().enumerate() {
            for (j, b) in strs.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "Filter strings at indices {i} and {j} should differ");
                }
            }
        }
    }

    #[test]
    fn test_build_filter_with_default_config() {
        // build_filter should succeed with a default config
        let config = LogConfig::default();
        let filter = build_filter(&config);
        assert!(filter.is_ok());
    }

    #[test]
    fn test_build_filter_with_all_levels() {
        for level in Level::ALL {
            let config = LogConfig {
                level,
                ..Default::default()
            };
            let filter = build_filter(&config);
            assert!(filter.is_ok(), "build_filter should succeed for level {level:?}");
        }
    }

    #[test]
    fn test_log_error_is_error_trait() {
        fn assert_error<T: std::error::Error>() {}
        assert_error::<LogError>();
    }

    // Note: We can't test init_logging() via set_global_default because
    // it can only be set once per process. Instead, we test the building
    // logic by using tracing::subscriber::with_default for thread-local
    // subscriber testing.

    #[test]
    fn test_init_logging_stderr_plain_builds_subscriber() {
        // Verify the subscriber construction path for Stderr+Plain
        let config = LogConfig {
            output: LogOutput::Stderr,
            format: LogFormat::Plain,
            level: Level::Info,
            ..Default::default()
        };
        let filter = build_filter(&config).unwrap();
        // Build the subscriber the same way init_logging does
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(io::stderr)
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .finish();
        // Use with_default to verify the subscriber works
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("test log from stderr plain");
        });
    }

    #[test]
    fn test_init_logging_stderr_json_builds_subscriber() {
        let config = LogConfig {
            output: LogOutput::Stderr,
            format: LogFormat::Json,
            level: Level::Debug,
            ..Default::default()
        };
        let filter = build_filter(&config).unwrap();
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(io::stderr)
            .json()
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!("test log from stderr json");
        });
    }

    #[test]
    fn test_init_logging_stderr_pretty_builds_subscriber() {
        let config = LogConfig {
            output: LogOutput::Stderr,
            format: LogFormat::Pretty,
            level: Level::Warn,
            ..Default::default()
        };
        let filter = build_filter(&config).unwrap();
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(io::stderr)
            .pretty()
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!("test log from stderr pretty");
        });
    }

    #[test]
    fn test_init_stdout_plain_builds_subscriber() {
        let config = LogConfig {
            output: LogOutput::Stdout,
            format: LogFormat::Plain,
            level: Level::Info,
            ..Default::default()
        };
        let filter = build_filter(&config).unwrap();
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(io::stdout)
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("test log from stdout plain");
        });
    }

    #[test]
    fn test_init_stdout_json_builds_subscriber() {
        let config = LogConfig {
            output: LogOutput::Stdout,
            format: LogFormat::Json,
            level: Level::Info,
            ..Default::default()
        };
        let filter = build_filter(&config).unwrap();
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(io::stdout)
            .json()
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("test log from stdout json");
        });
    }

    #[test]
    fn test_init_stdout_pretty_builds_subscriber() {
        let config = LogConfig {
            output: LogOutput::Stdout,
            format: LogFormat::Pretty,
            level: Level::Info,
            ..Default::default()
        };
        let filter = build_filter(&config).unwrap();
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(io::stdout)
            .pretty()
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("test log from stdout pretty");
        });
    }

    #[test]
    fn test_init_file_json_builds_subscriber() {
        let temp_dir = std::env::temp_dir().join("reovim_test_log_file_json");
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_path = temp_dir.join("test.log");

        let config = LogConfig {
            output: LogOutput::File,
            format: LogFormat::Json,
            level: Level::Info,
            file_path: Some(log_path.clone()),
            rotation: RotationPolicy::Never,
        };
        let filter = build_filter(&config).unwrap();

        let dir = log_path.parent().unwrap_or_else(|| Path::new("."));
        let prefix = log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reovim");
        let appender = rolling::never(dir, prefix);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);

        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(non_blocking)
            .json()
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("test log to file json");
        });
        drop(guard);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_init_file_plain_builds_subscriber() {
        let temp_dir = std::env::temp_dir().join("reovim_test_log_file_plain");
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_path = temp_dir.join("test.log");

        let config = LogConfig {
            output: LogOutput::File,
            format: LogFormat::Plain,
            level: Level::Info,
            file_path: Some(log_path.clone()),
            rotation: RotationPolicy::Never,
        };
        let filter = build_filter(&config).unwrap();

        let dir = log_path.parent().unwrap_or_else(|| Path::new("."));
        let prefix = log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reovim");
        let appender = rolling::never(dir, prefix);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);

        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(non_blocking)
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("test log to file plain");
        });
        drop(guard);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_init_file_pretty_builds_as_plain() {
        // Pretty format for files falls through to Plain (no ANSI in files)
        let temp_dir = std::env::temp_dir().join("reovim_test_log_file_pretty");
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_path = temp_dir.join("test.log");

        let config = LogConfig {
            output: LogOutput::File,
            format: LogFormat::Pretty,
            level: Level::Info,
            file_path: Some(log_path.clone()),
            rotation: RotationPolicy::Never,
        };
        let filter = build_filter(&config).unwrap();

        let dir = log_path.parent().unwrap_or_else(|| Path::new("."));
        let prefix = log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reovim");
        let appender = rolling::never(dir, prefix);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);

        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(non_blocking)
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("test log to file pretty-as-plain");
        });
        drop(guard);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_init_file_rotation_daily() {
        let temp_dir = std::env::temp_dir().join("reovim_test_log_daily");
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_path = temp_dir.join("daily.log");

        let dir = log_path.parent().unwrap_or_else(|| Path::new("."));
        let prefix = log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reovim");
        let appender = rolling::daily(dir, prefix);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);
        drop(non_blocking);
        drop(guard);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_init_file_rotation_hourly() {
        let temp_dir = std::env::temp_dir().join("reovim_test_log_hourly");
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_path = temp_dir.join("hourly.log");

        let dir = log_path.parent().unwrap_or_else(|| Path::new("."));
        let prefix = log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reovim");
        let appender = rolling::hourly(dir, prefix);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);
        drop(non_blocking);
        drop(guard);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_init_logging_file_missing_path_error() {
        let config = LogConfig {
            output: LogOutput::File,
            format: LogFormat::Plain,
            level: Level::Info,
            file_path: None,
            rotation: RotationPolicy::Never,
        };
        let result = init_logging(&config);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LogError::MissingFilePath));
    }

    #[test]
    fn test_init_file_path_without_parent() {
        // Test the unwrap_or_else fallback for dir (no parent path)
        let path = Path::new("justfilename.log");
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        assert_eq!(dir, Path::new(""));
    }

    #[test]
    fn test_init_file_path_without_filename() {
        // Test the unwrap_or fallback for prefix (no file name)
        let path = Path::new("/");
        let prefix = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reovim");
        assert_eq!(prefix, "reovim");
    }
}
