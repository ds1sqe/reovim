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
//!
//! ## LSP Log
//!
//! Dedicated LSP JSON-RPC message logging:
//! - `--lsp-log=default` - Timestamped file `lsp-<timestamp>.log`
//! - `--lsp-log=/path/to/lsp.log` - Custom file path
//! - Filter: Only logs with target `reovim_lsp` are captured

use std::{io, path::PathBuf};

use {
    tracing_appender::non_blocking::WorkerGuard,
    tracing_subscriber::{
        EnvFilter, Layer,
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

/// Target for LSP-specific log output
#[derive(Debug, Clone)]
pub enum LspLogTarget {
    /// LSP logging disabled (default)
    Disabled,
    /// Default: timestamped file `lsp-<timestamp>.log` in XDG data directory
    Default,
    /// Custom file path
    File(PathBuf),
}

/// Container for log guards (must be held for duration of program)
///
/// These guards are intentionally held but never read - dropping them would
/// cause the non-blocking writers to flush and shut down.
#[allow(dead_code)]
pub struct LogGuards {
    /// Main log guard
    main: Option<WorkerGuard>,
    /// LSP log guard
    lsp: Option<WorkerGuard>,
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

/// Parse an LSP log target from a CLI argument
///
/// # Special values
/// - `None` -> Disabled (no LSP logging)
/// - `"default"` -> Default (timestamped file in ~/.local/share/reovim/)
/// - `"none"` or `"off"` -> Disabled
/// - Any other string -> File path
#[must_use]
pub fn parse_lsp_log_target(arg: Option<&str>) -> LspLogTarget {
    match arg {
        None | Some("none" | "off") => LspLogTarget::Disabled,
        Some("default") => LspLogTarget::Default,
        Some(path) => {
            let path = PathBuf::from(path);
            // Resolve relative paths against CWD
            if path.is_relative() {
                LspLogTarget::File(
                    std::env::current_dir()
                        .map(|cwd| cwd.join(&path))
                        .unwrap_or(path),
                )
            } else {
                LspLogTarget::File(path)
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

/// Generate a timestamped LSP log filename
fn generate_lsp_log_filename() -> String {
    let now = chrono::Local::now();
    format!("lsp-{}.log", now.format("%Y-%m-%d-%H-%M-%S"))
}

/// Initialize the logging system (without LSP logging)
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
/// Convenience function for initializing without LSP logging.
#[must_use]
#[allow(dead_code)]
pub fn init(target: &LogTarget) -> LogGuards {
    init_with_lsp(target, &LspLogTarget::Disabled)
}

/// Initialize the logging system with optional LSP logging
///
/// Returns guards that must be held for the duration of the program
/// to ensure logs are flushed before exit.
#[must_use]
pub fn init_with_lsp(target: &LogTarget, lsp_target: &LspLogTarget) -> LogGuards {
    let main_filter = EnvFilter::try_from_env(LOG_LEVEL_ENV)
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_LEVEL));

    // Handle each combination of main + LSP logging
    match (target, lsp_target) {
        (LogTarget::Disabled, LspLogTarget::Disabled) => {
            // No logging at all
            tracing_subscriber::registry().init();
            LogGuards {
                main: None,
                lsp: None,
            }
        }
        (LogTarget::Disabled, lsp) => {
            // Only LSP logging
            let lsp_guard = init_lsp_only(lsp);
            LogGuards {
                main: None,
                lsp: lsp_guard,
            }
        }
        (main, LspLogTarget::Disabled) => {
            // Only main logging (original behavior)
            let main_guard = init_main_only(main, main_filter);
            LogGuards {
                main: main_guard,
                lsp: None,
            }
        }
        (main, lsp) => {
            // Both main and LSP logging
            init_both_layers(main, lsp, main_filter)
        }
    }
}

/// Initialize main logging only (original single-layer behavior)
fn init_main_only(target: &LogTarget, filter: EnvFilter) -> Option<WorkerGuard> {
    match target {
        LogTarget::Disabled => None,
        LogTarget::Stderr => {
            let (non_blocking, guard) = tracing_appender::non_blocking(io::stderr());
            let layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(true)
                .with_timer(UtcTime::rfc_3339())
                .with_target(true)
                .with_file(true)
                .with_line_number(true);
            tracing_subscriber::registry()
                .with(filter)
                .with(layer)
                .init();
            Some(guard)
        }
        LogTarget::Default => {
            let log_dir = get_log_directory();
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                eprintln!("Warning: Failed to create log directory: {e}");
                return None;
            }
            let log_filename = generate_log_filename();
            Some(init_file_layer(&log_dir, &log_filename, filter))
        }
        LogTarget::File(path) => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                eprintln!("Warning: Failed to create log directory {}: {e}", parent.display());
                return None;
            }
            let log_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let log_filename = path
                .file_name()
                .map_or_else(|| "reovim.log".to_string(), |s| s.to_string_lossy().to_string());
            Some(init_file_layer(log_dir, &log_filename, filter))
        }
    }
}

/// Initialize a file-based log layer
fn init_file_layer(
    log_dir: &std::path::Path,
    log_filename: &str,
    filter: EnvFilter,
) -> WorkerGuard {
    let file_appender = tracing_appender::rolling::never(log_dir, log_filename);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_file(true)
        .with_line_number(true);
    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .init();
    guard
}

/// Initialize LSP-only logging (when main logging is disabled)
fn init_lsp_only(target: &LspLogTarget) -> Option<WorkerGuard> {
    let (log_dir, log_filename) = match target {
        LspLogTarget::Disabled => return None,
        LspLogTarget::Default => {
            let log_dir = get_log_directory();
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                eprintln!("Warning: Failed to create LSP log directory: {e}");
                return None;
            }
            (log_dir, generate_lsp_log_filename())
        }
        LspLogTarget::File(path) => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                eprintln!("Warning: Failed to create LSP log directory {}: {e}", parent.display());
                return None;
            }
            let log_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let log_filename = path
                .file_name()
                .map_or_else(|| "lsp.log".to_string(), |s| s.to_string_lossy().to_string());
            (log_dir.to_path_buf(), log_filename)
        }
    };

    let file_appender = tracing_appender::rolling::never(&log_dir, &log_filename);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Filter to only include reovim_lsp target at trace level
    let lsp_filter = EnvFilter::new("reovim_lsp=trace");

    let layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_timer(UtcTime::rfc_3339())
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_filter(lsp_filter);

    tracing_subscriber::registry().with(layer).init();
    Some(guard)
}

/// Initialize both main and LSP logging layers
#[allow(clippy::too_many_lines)]
fn init_both_layers(
    main_target: &LogTarget,
    lsp_target: &LspLogTarget,
    main_filter: EnvFilter,
) -> LogGuards {
    // Get main layer components
    let (main_guard, main_nb) = match main_target {
        LogTarget::Disabled => (None, None),
        LogTarget::Stderr => {
            let (nb, guard) = tracing_appender::non_blocking(io::stderr());
            (Some(guard), Some((nb, true))) // true = ansi
        }
        LogTarget::Default => {
            let log_dir = get_log_directory();
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                eprintln!("Warning: Failed to create log directory: {e}");
                (None, None)
            } else {
                let log_filename = generate_log_filename();
                let file_appender = tracing_appender::rolling::never(&log_dir, &log_filename);
                let (nb, guard) = tracing_appender::non_blocking(file_appender);
                (Some(guard), Some((nb, false))) // false = no ansi
            }
        }
        LogTarget::File(path) => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                eprintln!("Warning: Failed to create log directory {}: {e}", parent.display());
                (None, None)
            } else {
                let log_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
                let log_filename = path
                    .file_name()
                    .map_or_else(|| "reovim.log".to_string(), |s| s.to_string_lossy().to_string());
                let file_appender = tracing_appender::rolling::never(log_dir, &log_filename);
                let (nb, guard) = tracing_appender::non_blocking(file_appender);
                (Some(guard), Some((nb, false)))
            }
        }
    };

    // Get LSP layer components
    let (lsp_guard, lsp_nb) = match lsp_target {
        LspLogTarget::Disabled => (None, None),
        LspLogTarget::Default => {
            let log_dir = get_log_directory();
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                eprintln!("Warning: Failed to create LSP log directory: {e}");
                (None, None)
            } else {
                let log_filename = generate_lsp_log_filename();
                let file_appender = tracing_appender::rolling::never(&log_dir, &log_filename);
                let (nb, guard) = tracing_appender::non_blocking(file_appender);
                (Some(guard), Some(nb))
            }
        }
        LspLogTarget::File(path) => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                eprintln!("Warning: Failed to create LSP log directory {}: {e}", parent.display());
                (None, None)
            } else {
                let log_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
                let log_filename = path
                    .file_name()
                    .map_or_else(|| "lsp.log".to_string(), |s| s.to_string_lossy().to_string());
                let file_appender = tracing_appender::rolling::never(log_dir, &log_filename);
                let (nb, guard) = tracing_appender::non_blocking(file_appender);
                (Some(guard), Some(nb))
            }
        }
    };

    // Build the subscriber with both layers
    let registry = tracing_subscriber::registry();

    match (main_nb, lsp_nb) {
        (Some((main_nb, ansi)), Some(lsp_nb)) => {
            // Apply main_filter to main_layer only (not at registry level)
            // so LSP trace events can reach the lsp_layer
            let main_layer = fmt::layer()
                .with_writer(main_nb)
                .with_ansi(ansi)
                .with_timer(UtcTime::rfc_3339())
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_filter(main_filter);

            let lsp_filter = EnvFilter::new("reovim_lsp=trace");
            let lsp_layer = fmt::layer()
                .with_writer(lsp_nb)
                .with_ansi(false)
                .with_timer(UtcTime::rfc_3339())
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_filter(lsp_filter);

            registry.with(main_layer).with(lsp_layer).init();
        }
        (Some((main_nb, ansi)), None) => {
            let main_layer = fmt::layer()
                .with_writer(main_nb)
                .with_ansi(ansi)
                .with_timer(UtcTime::rfc_3339())
                .with_target(true)
                .with_file(true)
                .with_line_number(true);

            registry.with(main_filter).with(main_layer).init();
        }
        (None, Some(lsp_nb)) => {
            let lsp_filter = EnvFilter::new("reovim_lsp=trace");
            let lsp_layer = fmt::layer()
                .with_writer(lsp_nb)
                .with_ansi(false)
                .with_timer(UtcTime::rfc_3339())
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_filter(lsp_filter);

            registry.with(lsp_layer).init();
        }
        (None, None) => {
            registry.init();
        }
    }

    LogGuards {
        main: main_guard,
        lsp: lsp_guard,
    }
}
