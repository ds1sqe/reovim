//! Ex-command types - re-exported from driver-command.
//!
//! These types are now defined in `reovim-driver-command` (mechanism layer)
//! and re-exported here for backwards compatibility.
//!
//! # Architecture
//!
//! - **Mechanism (driver-command)**: `ExCommandHandler` trait, context, errors
//! - **Policy (this module)**: Concrete command implementations (`:w`, `:q`, etc.)

// Re-export from driver-command for backwards compatibility
pub use reovim_driver_command::{
    ExCommandContext, ExCommandError as CommandError, ExCommandHandler, ExCommandRange as Range,
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::Position};

    #[test]
    fn test_range_new() {
        let range = Range::new(Position::new(0, 0), Position::new(5, 0));
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.column, 0);
        assert_eq!(range.end.line, 5);
        assert_eq!(range.end.column, 0);
    }

    #[test]
    fn test_range_with_columns() {
        let range = Range::new(Position::new(1, 3), Position::new(4, 7));
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.column, 3);
        assert_eq!(range.end.line, 4);
        assert_eq!(range.end.column, 7);
    }

    #[test]
    fn test_range_single_line() {
        let range = Range::new(Position::new(3, 0), Position::new(3, 10));
        assert_eq!(range.start.line, range.end.line);
    }

    #[test]
    fn test_command_error_display_no_buffer() {
        let err = CommandError::NoBuffer;
        assert_eq!(err.to_string(), "no buffer");
    }

    #[test]
    fn test_command_error_display_no_window() {
        let err = CommandError::NoWindow;
        assert_eq!(err.to_string(), "no window");
    }

    #[test]
    fn test_command_error_display_invalid_arguments() {
        let err = CommandError::InvalidArguments("bad arg".into());
        let display = err.to_string();
        assert!(display.contains("invalid arguments"));
        assert!(display.contains("bad arg"));
    }

    #[test]
    fn test_command_error_display_execution_failed() {
        let err = CommandError::ExecutionFailed("disk full".into());
        let display = err.to_string();
        assert!(display.contains("execution failed"));
        assert!(display.contains("disk full"));
    }

    #[test]
    fn test_command_error_display_unknown_command() {
        let err = CommandError::UnknownCommand("foo".into());
        let display = err.to_string();
        assert!(display.contains("unknown command"));
        assert!(display.contains("foo"));
    }

    #[test]
    fn test_command_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(CommandError::NoBuffer);
        assert_eq!(err.to_string(), "no buffer");
    }

    #[test]
    fn test_command_error_debug() {
        let err = CommandError::NoBuffer;
        let debug = format!("{err:?}");
        assert!(debug.contains("NoBuffer"));
    }

    #[test]
    fn test_command_error_clone() {
        let err = CommandError::InvalidArguments("test".into());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_range_equality() {
        let a = Range::new(Position::new(1, 0), Position::new(5, 0));
        let b = Range::new(Position::new(1, 0), Position::new(5, 0));
        assert_eq!(a, b);
    }

    #[test]
    fn test_range_inequality() {
        let a = Range::new(Position::new(1, 0), Position::new(5, 0));
        let b = Range::new(Position::new(1, 0), Position::new(6, 0));
        assert_ne!(a, b);
    }
}
