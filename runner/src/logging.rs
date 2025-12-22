//! Logging initialization for reovim
//!
//! Configures tracing-based file logging with runtime-configurable levels.
//! Log files are written to `~/.local/share/reovim/reovim-<timestamp>.log`.

use std::path::PathBuf;

use {
    tracing_appender::non_blocking::WorkerGuard,
    tracing_subscriber::{
        EnvFilter,
        fmt::{self, time::UtcTime},
        layer::SubscriberExt,
        util::SubscriberInitExt,
    },
};

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
/// ```
#[must_use]
pub fn init() -> Option<WorkerGuard> {
    let log_dir = get_log_directory();

    // Create log directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Warning: Failed to create log directory: {e}");
        return None;
    }

    let log_filename = generate_log_filename();

    // Create file appender (non-blocking for async compatibility)
    let file_appender = tracing_appender::rolling::never(&log_dir, &log_filename);
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

    Some(guard)
}
