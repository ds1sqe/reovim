#![cfg_attr(coverage_nightly, coverage(off))]
//! Application logging bootstrap.
//!
//! Sets up the `tracing` subscriber for the reovim binary at startup.
//! The kernel `Logger` trait is wired separately via
//! `reovim_server::debug::COMPOSITE_LOGGER` in `init_debug_infrastructure`.

use std::{
    io,
    path::{Path, PathBuf},
};

use {
    reovim_kernel::api::v1::Level,
    tracing_appender::rolling,
    tracing_subscriber::{EnvFilter, fmt},
};

#[derive(Debug, Clone)]
pub struct LogConfig {
    pub level: Level,
    pub output: LogOutput,
    pub format: LogFormat,
    pub file_path: Option<PathBuf>,
    pub rotation: RotationPolicy,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Level::Info,
            output: LogOutput::Stderr,
            format: LogFormat::Plain,
            file_path: None,
            rotation: RotationPolicy::Never,
        }
    }
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Stdout/File variants reserved for future log routing
pub enum LogOutput {
    #[default]
    Stderr,
    Stdout,
    File,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Json/Pretty variants reserved for future log format selection
pub enum LogFormat {
    #[default]
    Plain,
    Json,
    Pretty,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Daily/Hourly variants reserved for future log rotation
pub enum RotationPolicy {
    #[default]
    Never,
    Daily,
    Hourly,
}

#[derive(Debug)]
pub enum LogError {
    MissingFilePath,
    InvalidFilter(String),
    SetGlobalDefault(String),
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

/// Install the tracing subscriber described by `config`.
///
/// # Errors
///
/// Returns a [`LogError`] if the env-filter fails to parse or the
/// configured log file cannot be opened.
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

fn build_filter(config: &LogConfig) -> Result<EnvFilter, LogError> {
    if let Ok(filter) = EnvFilter::try_from_env("REOVIM_LOG") {
        return Ok(filter);
    }
    let level_str = level_to_filter_str(config.level);
    EnvFilter::try_new(level_str).map_err(|e| LogError::InvalidFilter(e.to_string()))
}

const fn level_to_filter_str(level: Level) -> &'static str {
    match level {
        Level::Error => "error",
        Level::Warn => "warn",
        Level::Info => "info",
        Level::Debug => "debug",
        Level::Trace => "trace",
    }
}

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
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_writer(non_blocking)
                .with_file(true)
                .with_line_number(true)
                .with_target(true)
                .with_ansi(false)
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LogError::SetGlobalDefault(e.to_string()))
        }
    }
}
