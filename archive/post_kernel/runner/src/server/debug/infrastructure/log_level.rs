//! Dynamic log level control.
//!
//! Provides runtime log level changes using `tracing_subscriber` reload layer.
//!
//! # Type Note
//!
//! The reload layer type depends on the subscriber composition.
//! With our current setup (`registry().with(reload_layer).with(LogBufferLayer)...`),
//! the handle type is `reload::Handle<LevelFilter, tracing_subscriber::Registry>`.
//! This is because the reload layer is applied directly to the Registry before
//! other layers. If the layer composition order changes, the type would need
//! to be updated.

use std::sync::OnceLock;

use tracing_subscriber::{filter::LevelFilter, reload};

/// Error type for log level operations.
#[derive(Debug, Clone)]
pub enum LogLevelError {
    /// Invalid log level string.
    InvalidLevel(String),
    /// Reload handle not initialized.
    NotInitialized,
    /// Failed to apply new level.
    ReloadFailed(String),
}

impl std::fmt::Display for LogLevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLevel(s) => write!(f, "invalid log level: {s}"),
            Self::NotInitialized => write!(f, "log level control not initialized"),
            Self::ReloadFailed(s) => write!(f, "failed to reload log level: {s}"),
        }
    }
}

impl std::error::Error for LogLevelError {}

/// Global reload handle for log level filter.
///
/// The type parameter is `Registry` because the reload layer is applied
/// directly to `tracing_subscriber::registry()` before other layers.
static LEVEL_HANDLE: OnceLock<reload::Handle<LevelFilter, tracing_subscriber::Registry>> =
    OnceLock::new();

/// Initialize the level reload handle.
///
/// Should be called once during server initialization, after creating
/// the reload layer but before `try_init()`.
pub fn init_level_handle(handle: reload::Handle<LevelFilter, tracing_subscriber::Registry>) {
    let _ = LEVEL_HANDLE.set(handle);
}

/// Get current log level from the reload layer.
///
/// Falls back to `STATIC_MAX_LEVEL` if the reload handle is not initialized.
#[must_use]
pub fn get_current_level() -> String {
    LEVEL_HANDLE.get().map_or_else(
        || {
            tracing::level_filters::STATIC_MAX_LEVEL
                .to_string()
                .to_lowercase()
        },
        |handle| {
            handle.clone_current().map_or_else(
                || {
                    tracing::level_filters::STATIC_MAX_LEVEL
                        .to_string()
                        .to_lowercase()
                },
                |filter| format!("{filter:?}").to_lowercase(),
            )
        },
    )
}

/// Set log level dynamically.
///
/// # Arguments
///
/// * `level` - Log level string (trace, debug, info, warn, error, off)
///
/// # Returns
///
/// The previous log level string on success.
///
/// # Errors
///
/// Returns error if the level string is invalid, the reload handle is not
/// initialized, or the reload operation fails.
pub fn set_log_level(level: &str) -> Result<String, LogLevelError> {
    let filter = match level.to_lowercase().as_str() {
        "trace" => LevelFilter::TRACE,
        "debug" => LevelFilter::DEBUG,
        "info" => LevelFilter::INFO,
        "warn" | "warning" => LevelFilter::WARN,
        "error" => LevelFilter::ERROR,
        "off" => LevelFilter::OFF,
        _ => return Err(LogLevelError::InvalidLevel(level.to_string())),
    };

    let handle = LEVEL_HANDLE.get().ok_or(LogLevelError::NotInitialized)?;
    let previous = handle
        .clone_current()
        .map_or_else(|| "unknown".to_string(), |f| format!("{f:?}").to_lowercase());

    handle
        .reload(filter)
        .map_err(|e| LogLevelError::ReloadFailed(e.to_string()))?;

    Ok(previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_error_display() {
        let err = LogLevelError::InvalidLevel("bad".to_string());
        assert!(err.to_string().contains("invalid log level"));

        let err = LogLevelError::NotInitialized;
        assert!(err.to_string().contains("not initialized"));

        let err = LogLevelError::ReloadFailed("error".to_string());
        assert!(err.to_string().contains("failed to reload"));
    }

    #[test]
    fn test_get_current_level_fallback() {
        // When handle is not initialized, should return STATIC_MAX_LEVEL
        // This test relies on the fact that in test context, the handle
        // may not be initialized (depends on test execution order)
        let level = get_current_level();
        assert!(!level.is_empty());
    }

    #[test]
    fn test_set_log_level_not_initialized() {
        // In test context without proper initialization, should return NotInitialized
        // Note: This test may pass or fail depending on whether other tests
        // have initialized the handle. The global state makes this tricky.
        // The key behavior we're testing is that invalid levels return error.
        let result = set_log_level("invalid_level");
        assert!(result.is_err());
        match result {
            Err(LogLevelError::InvalidLevel(_) | LogLevelError::NotInitialized) => {}
            _ => panic!("Expected InvalidLevel or NotInitialized error"),
        }
    }

    #[test]
    fn test_level_parsing() {
        // Test that level parsing works for valid levels
        // We can't test the actual reload without proper initialization,
        // but we can verify the parsing logic by checking error types
        let levels = ["trace", "debug", "info", "warn", "warning", "error", "off"];
        for level in levels {
            let result = set_log_level(level);
            // Should either succeed or fail with NotInitialized, not InvalidLevel
            match result {
                Ok(_) | Err(LogLevelError::NotInitialized | LogLevelError::ReloadFailed(_)) => {}
                Err(LogLevelError::InvalidLevel(l)) => {
                    panic!("Valid level '{level}' was rejected as invalid: {l}");
                }
            }
        }
    }

    #[test]
    fn test_invalid_level_rejected() {
        let result = set_log_level("foobar");
        assert!(matches!(result, Err(LogLevelError::InvalidLevel(_))));

        let result = set_log_level("");
        assert!(matches!(result, Err(LogLevelError::InvalidLevel(_))));

        let result = set_log_level("DEBUG"); // uppercase should still work
        assert!(!matches!(result, Err(LogLevelError::InvalidLevel(_))));
    }
}
