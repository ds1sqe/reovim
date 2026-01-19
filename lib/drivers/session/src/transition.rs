//! Mode transition types.
//!
//! This module provides types for mode transitions:
//! - [`TransitionContext`] - Context passed when entering a new mode
//! - [`PopResult`] - Result returned when exiting a mode
//!
//! # Design
//!
//! These types enable mode stacking patterns:
//! - Push operator-pending mode with pending operator
//! - Pop with operator range result
//! - Pass counts and registers across mode boundaries
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::{TransitionContext, PopResult};
//!
//! // Enter operator-pending mode after pressing 'd'
//! let ctx = TransitionContext::with_operator(delete_cmd);
//! session.push_mode(op_pending_mode, ctx);
//!
//! // Motion provides range, pop back with result
//! let result = PopResult::OperatorRange {
//!     operator: delete_cmd,
//!     start,
//!     end,
//!     linewise: false,
//!     count: None,
//!     register: None,
//! };
//! session.pop_mode(Some(result));
//! ```

use reovim_kernel::api::v1::{CommandId, Position};

/// Context passed when entering a new mode.
///
/// Contains state from the previous mode that the new mode needs:
/// - Pending operator (for operator-pending mode)
/// - Count prefix (inherited across mode transitions)
/// - Register selection
#[derive(Debug, Clone, Default)]
pub struct TransitionContext {
    /// Pending operator waiting for a motion/text-object.
    ///
    /// Set when entering operator-pending mode (e.g., after pressing `d`).
    pub pending_operator: Option<CommandId>,

    /// Count prefix to pass to the new mode.
    ///
    /// Counts can be combined: `2d3w` = delete 6 words.
    pub count: Option<usize>,

    /// Register for the operation.
    pub register: Option<char>,
}

impl TransitionContext {
    /// Create an empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with a pending operator.
    #[must_use]
    pub fn with_operator(operator: CommandId) -> Self {
        Self {
            pending_operator: Some(operator),
            ..Default::default()
        }
    }

    /// Set the pending operator (builder pattern).
    #[must_use]
    pub fn operator(mut self, op: CommandId) -> Self {
        self.pending_operator = Some(op);
        self
    }

    /// Set the count (builder pattern).
    #[must_use]
    pub const fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Set the register (builder pattern).
    #[must_use]
    pub const fn register(mut self, register: char) -> Self {
        self.register = Some(register);
        self
    }
}

/// Result returned when popping from a mode.
///
/// The parent mode uses this to complete its operation:
/// - Operator-pending returns range for the operator
/// - Search returns the search pattern
/// - Command-line returns the entered command
#[derive(Debug, Clone)]
pub enum PopResult {
    /// Operator completed with a range.
    ///
    /// The parent mode (normal) should execute the pending operator
    /// on this range.
    OperatorRange {
        /// The operator command to execute.
        operator: CommandId,
        /// Start position of the range.
        start: Position,
        /// End position of the range.
        end: Position,
        /// Whether the range is linewise (full lines).
        linewise: bool,
        /// Count applied to the operator.
        count: Option<usize>,
        /// Target register for the operation.
        register: Option<char>,
    },

    /// Text object selected.
    TextObject {
        /// Start position.
        start: Position,
        /// End position.
        end: Position,
        /// Whether the selection is linewise.
        linewise: bool,
        /// Whether this is an "inner" (i) or "around" (a) text object.
        inner: bool,
    },

    /// User cancelled the operation (Escape pressed).
    Cancelled,

    /// Search pattern entered.
    SearchPattern {
        /// The search pattern.
        pattern: String,
        /// Search direction (forward = true, backward = false).
        forward: bool,
    },

    /// Command-line command entered.
    CommandLine {
        /// The entered command string.
        command: String,
    },

    /// Character input completed (for f/t/r commands).
    CharInput {
        /// The entered character.
        char: char,
    },
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_command() -> CommandId {
        CommandId::new(ModuleId::new("test"), "delete")
    }

    #[test]
    fn test_transition_context_new() {
        let ctx = TransitionContext::new();
        assert!(ctx.pending_operator.is_none());
        assert!(ctx.count.is_none());
        assert!(ctx.register.is_none());
    }

    #[test]
    fn test_transition_context_with_operator() {
        let op = test_command();
        let ctx = TransitionContext::with_operator(op.clone());
        assert_eq!(ctx.pending_operator, Some(op));
    }

    #[test]
    fn test_transition_context_builder() {
        let op = test_command();
        let ctx = TransitionContext::new()
            .operator(op.clone())
            .count(2)
            .register('a');

        assert_eq!(ctx.pending_operator, Some(op));
        assert_eq!(ctx.count, Some(2));
        assert_eq!(ctx.register, Some('a'));
    }

    #[test]
    fn test_pop_result_cancelled() {
        let result = PopResult::Cancelled;
        assert!(matches!(result, PopResult::Cancelled));
    }

    #[test]
    fn test_pop_result_operator_range() {
        let result = PopResult::OperatorRange {
            operator: test_command(),
            start: Position::new(0, 0),
            end: Position::new(0, 5),
            linewise: false,
            count: Some(2),
            register: Some('a'),
        };

        if let PopResult::OperatorRange {
            linewise,
            count,
            register,
            ..
        } = result
        {
            assert!(!linewise);
            assert_eq!(count, Some(2));
            assert_eq!(register, Some('a'));
        } else {
            panic!("expected OperatorRange");
        }
    }

    #[test]
    fn test_pop_result_search_pattern() {
        let result = PopResult::SearchPattern {
            pattern: "foo".to_string(),
            forward: true,
        };

        if let PopResult::SearchPattern { pattern, forward } = result {
            assert_eq!(pattern, "foo");
            assert!(forward);
        } else {
            panic!("expected SearchPattern");
        }
    }
}
