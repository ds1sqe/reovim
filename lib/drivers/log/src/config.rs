//! Logging configuration types.
//!
//! These types define the **policy** for logging: where logs go,
//! how they're formatted, and when files rotate.
//!
//! Following Linux kernel "mechanism vs policy":
//! - Kernel provides mechanism (`Logger` trait)
//! - This driver provides policy (output, format, rotation)

use std::path::PathBuf;

use reovim_kernel::api::v1::Level;

/// Configuration for the logging driver.
///
/// # Example
///
/// ```
/// use reovim_driver_log::{LogConfig, LogOutput, LogFormat};
///
/// let config = LogConfig {
///     level: reovim_driver_log::Level::Debug,
///     output: LogOutput::Stderr,
///     format: LogFormat::Pretty,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Minimum log level to emit.
    ///
    /// Messages below this level will be filtered out.
    /// Can be overridden by `REOVIM_LOG` environment variable.
    pub level: Level,

    /// Where to send log output.
    pub output: LogOutput,

    /// Log message format.
    pub format: LogFormat,

    /// File path (required when output is `File`).
    pub file_path: Option<PathBuf>,

    /// Log file rotation policy (only used when output is `File`).
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

/// Log output destination.
///
/// Determines where log messages are written.
#[derive(Debug, Clone, Default)]
pub enum LogOutput {
    /// Write to stderr (default).
    ///
    /// This is the standard destination for log output,
    /// keeping stdout free for program output.
    #[default]
    Stderr,

    /// Write to stdout.
    ///
    /// Use when logs should go to standard output
    /// (e.g., for piping to other programs).
    Stdout,

    /// Write to file.
    ///
    /// Requires `file_path` to be set in `LogConfig`.
    /// Supports rotation via `RotationPolicy`.
    File,
}

/// Log message format.
///
/// Controls how log messages are formatted in output.
#[derive(Debug, Clone, Default)]
pub enum LogFormat {
    /// Plain text format.
    ///
    /// Output: `2024-01-15T10:30:00Z INFO file.rs:42 message`
    #[default]
    Plain,

    /// JSON structured format.
    ///
    /// Each log entry is a JSON object with fields:
    /// `timestamp`, `level`, `target`, `file`, `line`, `message`
    ///
    /// Useful for log aggregation systems.
    Json,

    /// Pretty formatted with colors (for terminal).
    ///
    /// Human-readable format with ANSI colors for levels.
    /// Best for development and debugging.
    Pretty,
}

/// Log file rotation policy.
///
/// Controls when log files are rotated (new file created).
/// Only applicable when `LogOutput::File` is used.
#[derive(Debug, Clone, Default)]
pub enum RotationPolicy {
    /// Never rotate - single log file.
    #[default]
    Never,

    /// Rotate daily at midnight (UTC).
    ///
    /// Creates files like: `reovim.2024-01-15.log`
    Daily,

    /// Rotate hourly.
    ///
    /// Creates files like: `reovim.2024-01-15-10.log`
    Hourly,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.level, Level::Info);
        assert!(matches!(config.output, LogOutput::Stderr));
        assert!(matches!(config.format, LogFormat::Plain));
        assert!(config.file_path.is_none());
        assert!(matches!(config.rotation, RotationPolicy::Never));
    }

    #[test]
    fn test_log_config_custom() {
        let config = LogConfig {
            level: Level::Debug,
            output: LogOutput::File,
            format: LogFormat::Json,
            file_path: Some(PathBuf::from("/tmp/reovim.log")),
            rotation: RotationPolicy::Daily,
        };

        assert_eq!(config.level, Level::Debug);
        assert!(matches!(config.output, LogOutput::File));
        assert!(matches!(config.format, LogFormat::Json));
        assert_eq!(config.file_path, Some(PathBuf::from("/tmp/reovim.log")));
        assert!(matches!(config.rotation, RotationPolicy::Daily));
    }

    #[test]
    fn test_log_output_default() {
        let output = LogOutput::default();
        assert!(matches!(output, LogOutput::Stderr));
    }

    #[test]
    fn test_log_format_default() {
        let format = LogFormat::default();
        assert!(matches!(format, LogFormat::Plain));
    }

    #[test]
    fn test_rotation_policy_default() {
        let rotation = RotationPolicy::default();
        assert!(matches!(rotation, RotationPolicy::Never));
    }

    #[test]
    fn test_log_config_clone() {
        let config = LogConfig {
            level: Level::Warn,
            output: LogOutput::Stdout,
            format: LogFormat::Pretty,
            file_path: None,
            rotation: RotationPolicy::Hourly,
        };

        // Clone and consume original to verify Clone actually works
        let cloned = config.clone();
        drop(config);

        // Verify cloned values
        assert_eq!(cloned.level, Level::Warn);
        assert!(matches!(cloned.output, LogOutput::Stdout));
        assert!(matches!(cloned.format, LogFormat::Pretty));
        assert!(matches!(cloned.rotation, RotationPolicy::Hourly));
    }
}
