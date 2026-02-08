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
        assert!(!result.is_detach());
    }

    #[test]
    fn test_command_result_error() {
        let result = CommandResult::error("Something went wrong");
        assert!(!result.is_success());
        assert!(result.is_error());
        assert!(!result.is_quit());
        assert!(!result.is_detach());
    }

    #[test]
    fn test_command_result_error_direct_construction() {
        let result = CommandResult::Error("direct error".to_string());
        assert!(result.is_error());
        assert!(!result.is_success());
    }

    #[test]
    fn test_command_result_quit() {
        let quit = CommandResult::Quit;
        assert!(quit.is_quit());
        assert!(!quit.is_success());
        assert!(!quit.is_error());
        assert!(!quit.is_detach());
    }

    #[test]
    fn test_command_result_force_quit() {
        let fq = CommandResult::ForceQuit;
        assert!(fq.is_quit());
        assert!(!fq.is_success());
        assert!(!fq.is_error());
        assert!(!fq.is_detach());
    }

    #[test]
    fn test_command_result_detach() {
        let detach = CommandResult::Detach;
        assert!(detach.is_detach());
        assert!(!detach.is_success());
        assert!(!detach.is_error());
        assert!(!detach.is_quit());
    }

    #[test]
    fn test_command_result_error_factory() {
        let result = CommandResult::error("test message");
        assert_eq!(result, CommandResult::Error("test message".to_string()));
    }

    #[test]
    fn test_command_result_error_factory_empty_message() {
        let result = CommandResult::error("");
        assert_eq!(result, CommandResult::Error(String::new()));
        assert!(result.is_error());
    }

    #[test]
    fn test_command_result_equality() {
        assert_eq!(CommandResult::Success, CommandResult::Success);
        assert_eq!(CommandResult::Quit, CommandResult::Quit);
        assert_eq!(CommandResult::ForceQuit, CommandResult::ForceQuit);
        assert_eq!(CommandResult::Detach, CommandResult::Detach);
        assert_eq!(CommandResult::Error("x".to_string()), CommandResult::Error("x".to_string()));

        assert_ne!(CommandResult::Success, CommandResult::Quit);
        assert_ne!(CommandResult::Quit, CommandResult::ForceQuit);
        assert_ne!(CommandResult::Success, CommandResult::Detach);
        assert_ne!(CommandResult::Error("a".to_string()), CommandResult::Error("b".to_string()));
    }

    #[test]
    fn test_command_result_clone() {
        let results = [
            CommandResult::Success,
            CommandResult::Error("err".to_string()),
            CommandResult::Quit,
            CommandResult::ForceQuit,
            CommandResult::Detach,
        ];
        for result in &results {
            let cloned = result.clone();
            assert_eq!(result, &cloned);
        }
    }

    #[test]
    fn test_command_result_debug() {
        let debug_str = format!("{:?}", CommandResult::Success);
        assert_eq!(debug_str, "Success");

        let debug_str = format!("{:?}", CommandResult::Quit);
        assert_eq!(debug_str, "Quit");

        let debug_str = format!("{:?}", CommandResult::ForceQuit);
        assert_eq!(debug_str, "ForceQuit");

        let debug_str = format!("{:?}", CommandResult::Detach);
        assert_eq!(debug_str, "Detach");

        let debug_str = format!("{:?}", CommandResult::Error("msg".to_string()));
        assert!(debug_str.contains("Error"));
        assert!(debug_str.contains("msg"));
    }
}
