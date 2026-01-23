//! Command execution result.
//!
//! Per issue #388, `CommandResult` has only 5 variants. Vim-specific
//! behavior (operator-pending, motion types) is handled by the vim
//! resolver via `VimSessionState`, not via command results.

/// Result of command execution.
///
/// # Design (Issue #388)
///
/// This enum is intentionally minimal. Vim-specific behavior like
/// operator-pending mode and motion types are policy concerns handled
/// by the vim module's resolver, not by command results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    /// Command executed successfully.
    Success,
    /// Command failed with an error message.
    Error(String),
    /// Command requests editor to quit.
    Quit,
    /// Command requests editor to quit without saving.
    ForceQuit,
    /// Command requests client to detach (server continues running).
    ///
    /// Unlike `Quit`, the server remains active and other clients can connect.
    /// The TUI client receives a DETACH notification and disconnects gracefully.
    Detach,
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

    /// Check if the result requests quit.
    #[must_use]
    pub const fn is_quit(&self) -> bool {
        matches!(self, Self::Quit | Self::ForceQuit)
    }

    /// Check if the result requests detach.
    #[must_use]
    pub const fn is_detach(&self) -> bool {
        matches!(self, Self::Detach)
    }

    /// Create an error result with a message.
    #[must_use]
    pub fn error(msg: &str) -> Self {
        Self::Error(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_result_success() {
        let result = CommandResult::Success;
        assert!(result.is_success());
        assert!(!result.is_error());
        assert!(!result.is_quit());
    }

    #[test]
    fn test_command_result_error() {
        let result = CommandResult::error("Something went wrong");
        assert!(!result.is_success());
        assert!(result.is_error());
        assert!(!result.is_quit());
    }

    #[test]
    fn test_command_result_quit() {
        assert!(CommandResult::Quit.is_quit());
        assert!(CommandResult::ForceQuit.is_quit());
    }
}
