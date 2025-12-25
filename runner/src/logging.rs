//! Logging initialization for reovim
//!
//! Configures tracing-based logging with runtime-configurable levels and targets.
//!
//! ## Log Targets
//!
//! - **Default**: Timestamped files in `~/.local/share/reovim/reovim-<timestamp>.log`
//! - **Custom file**: `--log=/path/to/file.log`
//! - **Stderr**: `--log=-` or `--log=stderr`
//! - **Disabled**: `--log=none` or `--log=off`

use std::{io, path::PathBuf};

use {
    tracing_appender::non_blocking::WorkerGuard,
    tracing_subscriber::{
        EnvFilter,
        fmt::{self, time::UtcTime},
        layer::SubscriberExt,
        util::SubscriberInitExt,
    },
};

/// Target for log output
#[derive(Debug, Clone)]
pub enum LogTarget {
    /// Default: timestamped file in XDG data directory
    Default,
    /// Custom file path
    File(PathBuf),
    /// Log to stderr
    Stderr,
    /// Logging disabled
    Disabled,
}

/// Parse a log target from a CLI argument
///
/// # Special values
/// - `None` -> Default (timestamped file in ~/.local/share/reovim/)
/// - `"-"` or `"stderr"` -> Stderr
/// - `"none"` or `"off"` -> Disabled
/// - Any other string -> File path
#[must_use]
pub fn parse_log_target(arg: Option<&str>) -> LogTarget {
    match arg {
        None => LogTarget::Default,
        Some("-" | "stderr") => LogTarget::Stderr,
        Some("none" | "off") => LogTarget::Disabled,
        Some(path) => {
            let path = PathBuf::from(path);
            // Resolve relative paths against CWD
            if path.is_relative() {
                LogTarget::File(
                    std::env::current_dir()
                        .map(|cwd| cwd.join(&path))
                        .unwrap_or(path),
                )
            } else {
                LogTarget::File(path)
            }
        }
    }
}

/// Default log level when `REOVIM_LOG` is not set
const DEFAULT_LOG_LEVEL: &str = "info";

/// Environment variable for controlling log level
const LOG_LEVEL_ENV: &str = "REOVIM_LOG";

/// Get the XDG data directory for reovim logs
fn get_log_directory() -> PathBuf {
    // Follow XDG Base Directory spec
    // $XDG_DATA_HOME defaults to $HOME/.local/share
    std::env::var("XDG_DATA_HOME")
        .map_or_else(
            |_| {
                let home = std::env::var("HOME").expect("HOME environment variable not set");
                PathBuf::from(home).join(".local").join("share")
            },
            PathBuf::from,
        )
        .join("reovim")
}

/// Generate a timestamped log filename
fn generate_log_filename() -> String {
    let now = chrono::Local::now();
    format!("reovim-{}.log", now.format("%Y-%m-%d-%H-%M-%S"))
}

/// Initialize the logging system
///
/// Returns a guard that must be held for the duration of the program
/// to ensure logs are flushed before exit.
///
/// # Log Levels
///
/// Set via `REOVIM_LOG` environment variable:
/// - `error` - Only errors
/// - `warn` - Warnings and above
/// - `info` - Informational messages and above (default)
/// - `debug` - Debug messages and above
/// - `trace` - All messages including trace-level
///
/// # Example
///
/// ```bash
/// REOVIM_LOG=debug reovim myfile.txt
/// REOVIM_LOG=trace reovim --log=- myfile.txt  # Debug to stderr
/// ```
#[must_use]
pub fn init(target: &LogTarget) -> Option<WorkerGuard> {
    match target {
        LogTarget::Disabled => None,
        LogTarget::Stderr => Some(init_stderr()),
        LogTarget::Default => init_default_file(),
        LogTarget::File(path) => init_custom_file(path),
    }
}

/// Initialize logging to stderr
fn init_stderr() -> WorkerGuard {
    let filter = EnvFilter::try_from_env(LOG_LEVEL_ENV)
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_LEVEL));

    // Use non_blocking for consistency with file-based logging
    let (non_blocking, guard) = tracing_appender::non_blocking(io::stderr());

    let fmt_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(true) // Enable ANSI colors for stderr
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    guard
}

/// Initialize logging to default timestamped file
fn init_default_file() -> Option<WorkerGuard> {
    let log_dir = get_log_directory();

    // Create log directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Warning: Failed to create log directory: {e}");
        return None;
    }

    let log_filename = generate_log_filename();
    Some(init_file_internal(&log_dir, &log_filename))
}

/// Initialize logging to custom file path
fn init_custom_file(path: &std::path::Path) -> Option<WorkerGuard> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("Warning: Failed to create log directory {}: {e}", parent.display());
        return None;
    }

    // Extract directory and filename
    let log_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let log_filename = path
        .file_name()
        .map_or_else(|| "reovim.log".to_string(), |s| s.to_string_lossy().to_string());

    Some(init_file_internal(log_dir, &log_filename))
}

/// Internal helper for file-based logging
fn init_file_internal(log_dir: &std::path::Path, log_filename: &str) -> WorkerGuard {
    // Create file appender (non-blocking for async compatibility)
    let file_appender = tracing_appender::rolling::never(log_dir, log_filename);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Build env filter from REOVIM_LOG or default
    let filter = EnvFilter::try_from_env(LOG_LEVEL_ENV)
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_LEVEL));

    // Configure subscriber with file output
    let fmt_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false) // No ANSI colors in log file
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    guard
}
