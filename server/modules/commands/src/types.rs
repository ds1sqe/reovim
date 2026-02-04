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
        assert_eq!(range.end.line, 5);
    }

    #[test]
    fn test_command_error_display() {
        let err = CommandError::NoBuffer;
        assert_eq!(err.to_string(), "no buffer");

        let err = CommandError::UnknownCommand("foo".into());
        assert!(err.to_string().contains("unknown command"));
    }
}
