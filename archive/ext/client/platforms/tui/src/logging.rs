#![cfg_attr(coverage_nightly, coverage(off))]
//! TUI platform file logging bootstrap.
//!
//! File-log initialization lives in the platform crate so the
//! standalone `apps/tui/` bin and any in-process embedded launcher
//! share one setup path. Callers pass `Some(path)` to route tracing
//! output to an append-only file (TUI mode owns the terminal, so
//! stderr would corrupt the display), or `None` to emit to stderr.

use std::{io, path::Path};

use {
    tracing_appender::rolling,
    tracing_subscriber::{EnvFilter, fmt},
};

/// Install a tracing subscriber for TUI platform use.
///
/// When `log_path` is `Some`, logs are written to that file (parent
/// directories created as needed). When `None`, logs go to stderr.
/// The filter reads `RUST_LOG` if set, otherwise defaults to `info`.
///
/// # Errors
///
/// Returns an I/O error if creating the log directory or opening the
/// log file fails, or an error if another subscriber is already
/// installed.
pub fn init(log_path: Option<&Path>) -> io::Result<()> {
    let filter = std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| EnvFilter::try_new(s).ok())
        .unwrap_or_else(|| EnvFilter::new("info"));

    if let Some(path) = log_path {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let prefix = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reovim-tui.log");

        let appender = rolling::never(dir, prefix);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);
        // The guard must outlive the process for logs to flush on drop.
        // Forgetting it is the standard pattern for bin-scope loggers.
        std::mem::forget(guard);

        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(non_blocking)
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber).map_err(io::Error::other)?;
    } else {
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(io::stderr)
            .finish();
        tracing::subscriber::set_global_default(subscriber).map_err(io::Error::other)?;
    }

    Ok(())
}
