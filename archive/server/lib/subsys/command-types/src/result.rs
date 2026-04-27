//! Command execution result.
//!
//! `CommandResult` follows the Unix exit code model: commands return
//! only exit status (`Success` / `Error`). Lifecycle side effects
//! (quit, disconnect) go through `RuntimeSignal` on `SessionRuntime`.
//!
//! See issue #547 for the design rationale.

/// Result of command execution.
///
/// Like Unix exit codes: commands return status only.
/// Side effects go through `runtime.signal()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    /// Command executed successfully (exit code 0).
    Success,
    /// Command failed with an error message (non-zero exit).
    Error(String),
}

impl CommandResult {
    /// Check if the result is success.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Check if the result is an error.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Create an error result with a message.
    #[must_use]
    pub fn error(msg: &str) -> Self {
        Self::Error(msg.to_string())
    }
}

#[cfg(test)]
#[path = "result_tests.rs"]
mod tests;
