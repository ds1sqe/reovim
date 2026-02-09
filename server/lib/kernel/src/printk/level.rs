//! Log level definitions for kernel messages.
//!
//! Linux equivalent: `KERN_*` constants in `include/linux/kern_levels.h`
//!
//! This module defines the severity levels for kernel log messages,
//! ordered from most severe (Error) to most verbose (Trace).

use std::{fmt, str::FromStr};

/// Log level for kernel messages.
///
/// Levels are ordered by severity, with `Error` being the most severe
/// and `Trace` being the most verbose. This ordering allows filtering:
/// if a logger is configured for `Info` level, it will log `Error`,
/// `Warn`, and `Info` messages, but not `Debug` or `Trace`.
///
/// # Linux Equivalent
///
/// | Level | Linux Constant | Usage |
/// |-------|----------------|-------|
/// | Error | `KERN_ERR` | Critical errors requiring immediate attention |
/// | Warn | `KERN_WARNING` | Warning conditions |
/// | Info | `KERN_INFO` | Informational messages |
/// | Debug | `KERN_DEBUG` | Debug-level messages |
/// | Trace | (custom) | Most verbose tracing |
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
/// use std::str::FromStr;
///
/// let level = Level::Info;
/// assert!(level <= Level::Debug);  // Info is less verbose than Debug
/// assert!(level >= Level::Error);  // Info is more verbose than Error
///
/// // Convert to/from string
/// assert_eq!(Level::Error.as_str(), "ERROR");
/// assert_eq!(Level::from_str("warn"), Ok(Level::Warn));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Level {
    /// Critical errors requiring immediate attention.
    ///
    /// Use for conditions that prevent normal operation.
    /// Linux equivalent: `KERN_ERR`
    Error = 0,

    /// Warning conditions.
    ///
    /// Use for potentially harmful situations that don't prevent operation.
    /// Linux equivalent: `KERN_WARNING`
    Warn = 1,

    /// Informational messages.
    ///
    /// Use for general operational messages.
    /// Linux equivalent: `KERN_INFO`
    #[default]
    Info = 2,

    /// Debug-level messages.
    ///
    /// Use for detailed diagnostic information during development.
    /// Linux equivalent: `KERN_DEBUG`
    Debug = 3,

    /// Trace-level messages.
    ///
    /// Most verbose level, for detailed tracing of execution flow.
    Trace = 4,
}

impl Level {
    /// All log levels in order of severity.
    pub const ALL: [Self; 5] = [
        Self::Error,
        Self::Warn,
        Self::Info,
        Self::Debug,
        Self::Trace,
    ];

    /// Returns the string representation of this level.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    ///
    /// assert_eq!(Level::Error.as_str(), "ERROR");
    /// assert_eq!(Level::Warn.as_str(), "WARN");
    /// assert_eq!(Level::Info.as_str(), "INFO");
    /// assert_eq!(Level::Debug.as_str(), "DEBUG");
    /// assert_eq!(Level::Trace.as_str(), "TRACE");
    /// ```
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }

    /// Check if this level is at least as severe as the given level.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    ///
    /// assert!(Level::Error.is_at_least(Level::Info));   // Error is more severe
    /// assert!(Level::Info.is_at_least(Level::Info));    // Same level
    /// assert!(!Level::Debug.is_at_least(Level::Info));  // Debug is less severe
    /// ```
    #[must_use]
    pub const fn is_at_least(&self, other: Self) -> bool {
        (*self as u8) <= (other as u8)
    }
}

/// Error returned when parsing a level from a string fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLevelError;

impl fmt::Display for ParseLevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown log level")
    }
}

impl std::error::Error for ParseLevelError {}

impl FromStr for Level {
    type Err = ParseLevelError;

    /// Parses a level from a string (case-insensitive).
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(Level::from_str("error"), Ok(Level::Error));
    /// assert_eq!(Level::from_str("WARN"), Ok(Level::Warn));
    /// assert_eq!(Level::from_str("Info"), Ok(Level::Info));
    /// assert!(Level::from_str("unknown").is_err());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "error" | "err" => Ok(Self::Error),
            "warn" | "warning" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(ParseLevelError),
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_ordering() {
        // Error is most severe (lowest value)
        assert!(Level::Error < Level::Warn);
        assert!(Level::Warn < Level::Info);
        assert!(Level::Info < Level::Debug);
        assert!(Level::Debug < Level::Trace);
    }

    #[test]
    fn test_level_as_str() {
        assert_eq!(Level::Error.as_str(), "ERROR");
        assert_eq!(Level::Warn.as_str(), "WARN");
        assert_eq!(Level::Info.as_str(), "INFO");
        assert_eq!(Level::Debug.as_str(), "DEBUG");
        assert_eq!(Level::Trace.as_str(), "TRACE");
    }

    #[test]
    fn test_level_from_str() {
        // Exact matches
        assert_eq!(Level::from_str("error"), Ok(Level::Error));
        assert_eq!(Level::from_str("warn"), Ok(Level::Warn));
        assert_eq!(Level::from_str("info"), Ok(Level::Info));
        assert_eq!(Level::from_str("debug"), Ok(Level::Debug));
        assert_eq!(Level::from_str("trace"), Ok(Level::Trace));

        // Case insensitive
        assert_eq!(Level::from_str("ERROR"), Ok(Level::Error));
        assert_eq!(Level::from_str("Warn"), Ok(Level::Warn));
        assert_eq!(Level::from_str("INFO"), Ok(Level::Info));

        // Aliases
        assert_eq!(Level::from_str("err"), Ok(Level::Error));
        assert_eq!(Level::from_str("warning"), Ok(Level::Warn));

        // Unknown
        assert!(Level::from_str("unknown").is_err());
        assert!(Level::from_str("").is_err());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_level_display() {
        assert_eq!(format!("{}", Level::Error), "ERROR");
        assert_eq!(format!("{}", Level::Info), "INFO");
    }

    #[test]
    fn test_level_is_at_least() {
        // Error is at least as severe as everything
        assert!(Level::Error.is_at_least(Level::Error));
        assert!(Level::Error.is_at_least(Level::Warn));
        assert!(Level::Error.is_at_least(Level::Info));
        assert!(Level::Error.is_at_least(Level::Debug));
        assert!(Level::Error.is_at_least(Level::Trace));

        // Info is at least as severe as Info, Debug, Trace
        assert!(Level::Info.is_at_least(Level::Info));
        assert!(Level::Info.is_at_least(Level::Debug));
        assert!(Level::Info.is_at_least(Level::Trace));
        assert!(!Level::Info.is_at_least(Level::Error));
        assert!(!Level::Info.is_at_least(Level::Warn));

        // Trace is only at least as severe as itself
        assert!(Level::Trace.is_at_least(Level::Trace));
        assert!(!Level::Trace.is_at_least(Level::Debug));
    }

    #[test]
    fn test_level_default() {
        assert_eq!(Level::default(), Level::Info);
    }

    #[test]
    fn test_level_all() {
        assert_eq!(Level::ALL.len(), 5);
        assert_eq!(Level::ALL[0], Level::Error);
        assert_eq!(Level::ALL[4], Level::Trace);
    }

    #[test]
    fn test_parse_level_error() {
        let err = ParseLevelError;
        assert_eq!(format!("{err}"), "unknown log level");
    }
}
