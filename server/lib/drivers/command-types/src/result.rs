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
mod tests {
    use super::*;

    #[test]
    fn test_command_result_success() {
        let result = CommandResult::Success;
        assert!(result.is_success());
        assert!(!result.is_error());
    }

    #[test]
    fn test_command_result_error() {
        let result = CommandResult::error("Something went wrong");
        assert!(!result.is_success());
        assert!(result.is_error());
    }

    #[test]
    fn test_command_result_error_direct_construction() {
        let result = CommandResult::Error("direct error".to_string());
        assert!(result.is_error());
        assert!(!result.is_success());
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
        assert_eq!(CommandResult::Error("x".to_string()), CommandResult::Error("x".to_string()));

        assert_ne!(CommandResult::Success, CommandResult::Error("e".to_string()));
        assert_ne!(CommandResult::Error("a".to_string()), CommandResult::Error("b".to_string()));
    }

    #[test]
    fn test_command_result_clone() {
        let results = [
            CommandResult::Success,
            CommandResult::Error("err".to_string()),
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

        let debug_str = format!("{:?}", CommandResult::Error("msg".to_string()));
        assert!(debug_str.contains("Error"));
        assert!(debug_str.contains("msg"));
    }
}
